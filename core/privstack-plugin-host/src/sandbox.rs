//! Individual plugin sandbox — one Wasmtime instance per plugin.
//!
//! Each `PluginSandbox` owns a `wasmtime::Store` with:
//! - Memory isolation (configurable ceiling)
//! - CPU fuel budgets (prevents infinite loops)
//! - Entity-type scoping (plugin can only CRUD its declared types)
//! - Permission-gated host function access
//!
//! The sandbox compiles and instantiates a .wasm component, wiring up
//! all host imports and detecting optional capability exports.

use crate::bindings::PluginWorld;
use crate::error::PluginHostError;
use crate::permissions::{Permission, PermissionSet};
use crate::wit_types::*;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{debug, info, warn};
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, ResourceLimiter, Store};
use wasmtime::component::ResourceTable;
use wasmtime_wasi::p2::{IoView, WasiCtx, WasiCtxBuilder, WasiView};

/// Resource usage metrics for a plugin sandbox.
/// Used for monitoring and display in the UI.
#[derive(Debug, Clone, Serialize)]
pub struct PluginResourceMetrics {
    /// Memory currently used by the plugin in bytes.
    pub memory_used_bytes: usize,
    /// Maximum memory allowed for this plugin in bytes.
    pub memory_limit_bytes: usize,
    /// Memory usage as a ratio (0.0 to 1.0).
    pub memory_usage_ratio: f64,
    /// Fuel consumed in the last plugin call.
    pub fuel_consumed_last_call: u64,
    /// Fuel budget per call.
    pub fuel_budget_per_call: u64,
    /// Average fuel consumed over the last 1000 calls.
    pub fuel_average_last_1000: u64,
    /// Peak fuel consumed across all tracked calls.
    pub fuel_peak: u64,
    /// Number of calls in the fuel history (max 1000).
    pub fuel_history_count: usize,
    /// Number of entities owned by this plugin.
    pub entity_count: usize,
    /// Estimated disk usage in bytes for plugin entities.
    pub disk_usage_bytes: usize,
}

/// Resource limits for a plugin sandbox.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum memory in bytes (default: 64MB for first-party, 32MB for third-party).
    pub max_memory_bytes: usize,
    /// CPU fuel budget per invocation (prevents infinite loops).
    pub fuel_per_call: u64,
    /// Timeout per invocation in milliseconds.
    pub call_timeout_ms: u64,
    /// Shutdown deadline in milliseconds for `dispose()`.
    pub shutdown_deadline_ms: u64,
}

impl ResourceLimits {
    pub fn first_party() -> Self {
        Self {
            max_memory_bytes: 64 * 1024 * 1024, // 64MB
            fuel_per_call: 1_000_000_000,         // ~1 billion instructions
            call_timeout_ms: 5_000,
            shutdown_deadline_ms: 2_000,
        }
    }

    pub fn third_party() -> Self {
        Self {
            max_memory_bytes: 32 * 1024 * 1024, // 32MB
            fuel_per_call: 500_000_000,
            call_timeout_ms: 3_000,
            shutdown_deadline_ms: 2_000,
        }
    }
}

/// A resource limiter that tracks actual memory usage.
/// Wraps memory limit enforcement with allocation tracking.
pub struct TrackingLimiter {
    /// Maximum memory allowed in bytes.
    max_memory: usize,
    /// Current memory allocated (tracked via grow callbacks).
    current_memory: AtomicUsize,
    /// Maximum tables allowed.
    max_tables: u32,
    /// Maximum table elements allowed.
    max_table_elements: u32,
    /// Maximum instances allowed.
    max_instances: u32,
    /// Maximum memories allowed.
    max_memories: u32,
}

impl TrackingLimiter {
    pub fn new(max_memory: usize) -> Self {
        Self {
            max_memory,
            current_memory: AtomicUsize::new(0),
            max_tables: 100,
            max_table_elements: 20_000,
            max_instances: 50,
            max_memories: 50,
        }
    }

    /// Get the current memory usage in bytes.
    pub fn current_memory_bytes(&self) -> usize {
        self.current_memory.load(Ordering::Relaxed)
    }
}

impl ResourceLimiter for TrackingLimiter {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        // Track the new memory size
        self.current_memory.store(desired, Ordering::Relaxed);

        // Allow growth if within our limit
        if desired <= self.max_memory {
            Ok(true)
        } else {
            debug!(
                current = current,
                desired = desired,
                max = self.max_memory,
                "Memory growth denied - would exceed limit"
            );
            Ok(false)
        }
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        Ok(desired <= self.max_table_elements as usize)
    }

    fn instances(&self) -> usize {
        self.max_instances as usize
    }

    fn tables(&self) -> usize {
        self.max_tables as usize
    }

    fn memories(&self) -> usize {
        self.max_memories as usize
    }
}

/// State stored in each plugin's `wasmtime::Store`.
/// Contains references to host services the plugin can access.
/// Also implements the host import traits (see host_impl.rs).
pub struct PluginState {
    pub plugin_id: String,
    pub permissions: PermissionSet,
    /// Entity types this plugin declared — used for scoping enforcement.
    pub declared_entity_types: HashSet<String>,
    /// The entity store handle for CRUD operations.
    pub entity_store: Arc<privstack_storage::EntityStore>,
    /// The event store handle for sync event recording.
    pub event_store: Arc<privstack_storage::EventStore>,
    /// Plugin-scoped settings stored as key-value pairs.
    pub settings: HashMap<String, String>,
    /// Cached entity schemas from this plugin.
    pub schemas: Vec<WitEntitySchema>,
    /// Cached view state JSON.
    pub view_state: Option<String>,
    /// Pending state-change notification flag.
    pub state_dirty: bool,
    /// Pending navigation request (set by navigation host import).
    pub pending_navigation: Option<String>,
    /// Wasmtime resource limiter with memory tracking.
    pub limiter: TrackingLimiter,
    /// WASI context for wasm32-wasip1 imports.
    pub wasi_ctx: WasiCtx,
    /// Resource table required by WasiView.
    pub resource_table: ResourceTable,
}

impl IoView for PluginState {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.resource_table
    }
}

impl WasiView for PluginState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi_ctx
    }
}

impl PluginState {
    /// Check if a plugin is allowed to operate on a given entity type.
    pub fn check_entity_type_access(&self, entity_type: &str) -> Result<(), PluginHostError> {
        if self.declared_entity_types.contains(entity_type) {
            Ok(())
        } else {
            Err(PluginHostError::EntityTypeNotDeclared {
                plugin_id: self.plugin_id.clone(),
                entity_type: entity_type.to_string(),
            })
        }
    }

    /// Check if a permission is granted.
    pub fn check_permission(&self, permission: Permission) -> Result<(), PluginHostError> {
        if self.permissions.is_granted(permission) {
            Ok(())
        } else {
            Err(PluginHostError::PermissionDenied {
                plugin_id: self.plugin_id.clone(),
                permission: permission.interface_name().to_string(),
            })
        }
    }
}

/// Wasmtime runtime state — only present for real .wasm plugins.
struct WasmRuntime {
    _engine: Engine,
    store: Store<PluginState>,
    bindings: PluginWorld,
}

/// A sandboxed plugin instance, either metadata-only or backed by a real Wasmtime component.
pub struct PluginSandbox {
    pub metadata: WitPluginMetadata,
    pub resource_limits: ResourceLimits,
    /// Whether the plugin exports `linkable-item-provider`.
    pub has_linkable_item_provider: bool,
    /// Cached link_type from the plugin's linkable-item-provider export.
    pub cached_link_type: Option<String>,
    /// Whether the plugin exports `deep-link-target`.
    pub has_deep_link_target: bool,
    /// Whether the plugin exports `timer`.
    pub has_timer: bool,
    /// Whether the plugin exports `shutdown-aware`.
    pub has_shutdown_aware: bool,
    /// Wasmtime runtime — None for metadata-only sandboxes.
    runtime: Option<WasmRuntime>,
    /// Plugin state — used directly for metadata-only sandboxes.
    /// For Wasm sandboxes, state lives inside `runtime.store`.
    standalone_state: Option<PluginState>,
    /// Fuel consumed in the last plugin call (for metrics tracking).
    last_fuel_consumed: u64,
}

impl PluginSandbox {
    /// Create a new sandbox from metadata + schemas (no .wasm file).
    /// Used for testing and for the metadata-only loading path.
    pub fn new(
        metadata: WitPluginMetadata,
        schemas: Vec<WitEntitySchema>,
        permissions: PermissionSet,
        resource_limits: ResourceLimits,
        entity_store: Arc<privstack_storage::EntityStore>,
        event_store: Arc<privstack_storage::EventStore>,
    ) -> Result<Self, PluginHostError> {
        let declared_entity_types: HashSet<String> =
            schemas.iter().map(|s| s.entity_type.clone()).collect();

        info!(
            plugin_id = %metadata.id,
            entity_types = ?declared_entity_types,
            "Creating plugin sandbox (metadata-only)"
        );

        // Validate schemas convert to core format
        for schema in &schemas {
            schema
                .to_core_schema()
                .map_err(|e| PluginHostError::InvalidSchema(format!("{}: {}", metadata.id, e)))?;
        }

        let limiter = TrackingLimiter::new(resource_limits.max_memory_bytes);

        let state = PluginState {
            plugin_id: metadata.id.clone(),
            permissions,
            declared_entity_types,
            entity_store,
            event_store,
            settings: HashMap::new(),
            schemas: schemas.clone(),
            view_state: None,
            state_dirty: false,
            pending_navigation: None,
            limiter,
            wasi_ctx: WasiCtxBuilder::new().build(),
            resource_table: ResourceTable::new(),
        };

        Ok(Self {
            metadata,
            resource_limits,
            has_linkable_item_provider: false,
            cached_link_type: None,
            has_deep_link_target: false,
            has_timer: false,
            has_shutdown_aware: false,
            runtime: None,
            standalone_state: Some(state),
            last_fuel_consumed: 0,
        })
    }

    /// Create a sandbox from a compiled .wasm component file.
    /// This is the real runtime path: compiles the component, wires imports,
    /// calls `get_metadata()` + `get_entity_schemas()`, detects capabilities.
    pub fn from_wasm(
        wasm_path: &Path,
        permissions: PermissionSet,
        resource_limits: ResourceLimits,
        entity_store: Arc<privstack_storage::EntityStore>,
        event_store: Arc<privstack_storage::EventStore>,
    ) -> Result<Self, PluginHostError> {
        info!(path = %wasm_path.display(), "Loading Wasm component");

        let mut config = wasmtime::Config::new();
        config.wasm_component_model(true);
        config.consume_fuel(true);

        let engine = Engine::new(&config)
            .map_err(|e| PluginHostError::Compilation(e))?;

        // Load and compile the component
        let wasm_bytes = std::fs::read(wasm_path).map_err(|e| {
            PluginHostError::InitializationFailed(format!(
                "failed to read {}: {}",
                wasm_path.display(),
                e
            ))
        })?;

        let component = Component::new(&engine, &wasm_bytes)
            .map_err(|e| PluginHostError::Compilation(e))?;

        // Create linker with all host imports
        let mut linker = Linker::new(&engine);
        PluginWorld::add_to_linker(&mut linker, |state: &mut PluginState| state)
            .map_err(|e| PluginHostError::Compilation(e))?;

        // Link WASI preview 2 (required for wasm32-wasip1 compiled components)
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
            .map_err(|e| PluginHostError::Compilation(e))?;

        let limiter = TrackingLimiter::new(resource_limits.max_memory_bytes);

        // Build a minimal WASI context (no filesystem, no network, no env)
        let wasi_ctx = WasiCtxBuilder::new().build();

        // Bootstrap with empty state — we'll fill in entity types after get_metadata
        let state = PluginState {
            plugin_id: String::new(),
            permissions,
            declared_entity_types: HashSet::new(),
            entity_store,
            event_store,
            settings: HashMap::new(),
            schemas: Vec::new(),
            view_state: None,
            state_dirty: false,
            pending_navigation: None,
            limiter,
            wasi_ctx,
            resource_table: ResourceTable::new(),
        };

        let mut store = Store::new(&engine, state);
        store.set_fuel(resource_limits.fuel_per_call).ok();
        store.limiter(|s| &mut s.limiter);

        // Instantiate
        let bindings = PluginWorld::instantiate(&mut store, &component, &linker)
            .map_err(|e| PluginHostError::Compilation(e))?;

        // Call get_metadata() to discover plugin identity
        store.set_fuel(resource_limits.fuel_per_call).ok();
        let wit_metadata = bindings
            .privstack_plugin_plugin()
            .call_get_metadata(&mut store)
            .map_err(|e| {
                PluginHostError::PluginCrashed {
                    plugin_id: "unknown".into(),
                    message: format!("get_metadata() failed: {}", e),
                }
            })?;

        let metadata = convert_wit_metadata(&wit_metadata);
        store.data_mut().plugin_id = metadata.id.clone();

        // Call get_entity_schemas()
        store.set_fuel(resource_limits.fuel_per_call).ok();
        let wit_schemas = bindings
            .privstack_plugin_plugin()
            .call_get_entity_schemas(&mut store)
            .map_err(|e| {
                PluginHostError::PluginCrashed {
                    plugin_id: metadata.id.clone(),
                    message: format!("get_entity_schemas() failed: {}", e),
                }
            })?;

        let schemas = convert_wit_schemas(&wit_schemas);
        let declared_entity_types: HashSet<String> =
            schemas.iter().map(|s| s.entity_type.clone()).collect();

        // Validate schemas
        for schema in &schemas {
            schema
                .to_core_schema()
                .map_err(|e| PluginHostError::InvalidSchema(format!("{}: {}", metadata.id, e)))?;
        }

        store.data_mut().declared_entity_types = declared_entity_types;
        store.data_mut().schemas = schemas;

        // All exports are required by the WIT world definition.
        // Plugins that don't need a capability provide stub implementations.
        let has_linkable_item_provider = true;
        let has_deep_link_target = true;
        let has_timer = true;
        let has_shutdown_aware = true;

        info!(
            plugin_id = %metadata.id,
            linkable = has_linkable_item_provider,
            deep_link = has_deep_link_target,
            timer = has_timer,
            shutdown_aware = has_shutdown_aware,
            "Wasm component loaded"
        );

        let mut sandbox = Self {
            metadata,
            resource_limits,
            has_linkable_item_provider,
            cached_link_type: None,
            has_deep_link_target,
            has_timer,
            has_shutdown_aware,
            runtime: Some(WasmRuntime {
                _engine: engine,
                store,
                bindings,
            }),
            standalone_state: None,
            last_fuel_consumed: 0,
        };

        // Cache link_type at load time to avoid reentrant calls later
        if has_linkable_item_provider {
            sandbox.cached_link_type = sandbox.call_link_type().ok();
        }

        Ok(sandbox)
    }

    /// Create a sandbox from a .wasm file using a shared engine and compiled component cache.
    ///
    /// Caches the compiled component as `plugin.cwasm` alongside the `.wasm` file.
    /// Uses SHA-256 of the `.wasm` bytes to detect staleness. On cache hit, deserializes
    /// the pre-compiled component (skipping compilation entirely). On miss, compiles and
    /// writes the cache for next time.
    pub fn from_wasm_cached(
        wasm_path: &Path,
        engine: &Engine,
        permissions: PermissionSet,
        resource_limits: ResourceLimits,
        entity_store: Arc<privstack_storage::EntityStore>,
        event_store: Arc<privstack_storage::EventStore>,
    ) -> Result<Self, PluginHostError> {
        info!(path = %wasm_path.display(), "Loading Wasm component (cached)");

        let wasm_bytes = std::fs::read(wasm_path).map_err(|e| {
            PluginHostError::InitializationFailed(format!(
                "failed to read {}: {}",
                wasm_path.display(),
                e
            ))
        })?;

        // SHA-256 hash of wasm bytes for cache invalidation
        let wasm_hash = hex::encode(Sha256::digest(&wasm_bytes));

        let cwasm_path = wasm_path.with_extension("cwasm");
        let hash_path = wasm_path.with_extension("cwasm.sha256");

        let component = if cwasm_path.exists() && hash_path.exists() {
            let stored_hash = std::fs::read_to_string(&hash_path).unwrap_or_default();
            if stored_hash.trim() == wasm_hash {
                info!(path = %cwasm_path.display(), "Loading cached compiled component");
                let cwasm_bytes = std::fs::read(&cwasm_path).map_err(|e| {
                    PluginHostError::InitializationFailed(format!(
                        "failed to read cached component {}: {}",
                        cwasm_path.display(),
                        e
                    ))
                })?;
                // SAFETY: The cwasm was produced by the same engine configuration
                // and the sha256 hash guarantees it matches the source .wasm bytes.
                unsafe {
                    Component::deserialize(engine, &cwasm_bytes)
                        .map_err(|e| PluginHostError::Compilation(e))?
                }
            } else {
                info!(path = %wasm_path.display(), "Cache hash mismatch, recompiling");
                Self::compile_and_cache(engine, &wasm_bytes, &wasm_hash, &cwasm_path, &hash_path, wasm_path)?
            }
        } else {
            info!(path = %wasm_path.display(), "No cached component found, compiling");
            Self::compile_and_cache(engine, &wasm_bytes, &wasm_hash, &cwasm_path, &hash_path, wasm_path)?
        };

        Self::instantiate_component(engine, component, permissions, resource_limits, entity_store, event_store)
    }

    /// Compile a wasm component and write the cache files.
    fn compile_and_cache(
        engine: &Engine,
        wasm_bytes: &[u8],
        wasm_hash: &str,
        cwasm_path: &Path,
        hash_path: &Path,
        wasm_path: &Path,
    ) -> Result<Component, PluginHostError> {
        info!(path = %wasm_path.display(), size_bytes = wasm_bytes.len(), "Compiling Wasm component");
        let compile_start = std::time::Instant::now();
        let component = Component::new(engine, wasm_bytes)
            .map_err(|e| PluginHostError::Compilation(e))?;
        info!(path = %wasm_path.display(), elapsed_ms = compile_start.elapsed().as_millis(), "Compilation complete");

        // Best-effort cache write — don't fail the load if caching fails
        info!(path = %cwasm_path.display(), "Caching compiled component");
        match component.serialize() {
            Ok(serialized) => {
                if let Err(e) = std::fs::write(cwasm_path, &serialized) {
                    warn!(path = %cwasm_path.display(), error = %e, "Failed to write compiled component cache");
                } else if let Err(e) = std::fs::write(hash_path, wasm_hash) {
                    warn!(path = %hash_path.display(), error = %e, "Failed to write component cache hash");
                } else {
                    info!(path = %cwasm_path.display(), "Component cached successfully");
                }
            }
            Err(e) => {
                warn!(path = %wasm_path.display(), error = %e, "Failed to serialize compiled component");
            }
        }

        Ok(component)
    }

    /// Shared instantiation logic for both `from_wasm` and `from_wasm_cached`.
    fn instantiate_component(
        engine: &Engine,
        component: Component,
        permissions: PermissionSet,
        resource_limits: ResourceLimits,
        entity_store: Arc<privstack_storage::EntityStore>,
        event_store: Arc<privstack_storage::EventStore>,
    ) -> Result<Self, PluginHostError> {
        let mut linker = Linker::new(engine);
        PluginWorld::add_to_linker(&mut linker, |state: &mut PluginState| state)
            .map_err(|e| PluginHostError::Compilation(e))?;
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
            .map_err(|e| PluginHostError::Compilation(e))?;

        let limiter = TrackingLimiter::new(resource_limits.max_memory_bytes);

        let wasi_ctx = WasiCtxBuilder::new().build();

        let state = PluginState {
            plugin_id: String::new(),
            permissions,
            declared_entity_types: HashSet::new(),
            entity_store,
            event_store,
            settings: HashMap::new(),
            schemas: Vec::new(),
            view_state: None,
            state_dirty: false,
            pending_navigation: None,
            limiter,
            wasi_ctx,
            resource_table: ResourceTable::new(),
        };

        let mut store = Store::new(engine, state);
        store.set_fuel(resource_limits.fuel_per_call).ok();
        store.limiter(|s| &mut s.limiter);

        let bindings = PluginWorld::instantiate(&mut store, &component, &linker)
            .map_err(|e| PluginHostError::Compilation(e))?;

        // Call get_metadata()
        store.set_fuel(resource_limits.fuel_per_call).ok();
        let wit_metadata = bindings
            .privstack_plugin_plugin()
            .call_get_metadata(&mut store)
            .map_err(|e| {
                PluginHostError::PluginCrashed {
                    plugin_id: "unknown".into(),
                    message: format!("get_metadata() failed: {}", e),
                }
            })?;

        let metadata = convert_wit_metadata(&wit_metadata);
        store.data_mut().plugin_id = metadata.id.clone();

        // Call get_entity_schemas()
        store.set_fuel(resource_limits.fuel_per_call).ok();
        let wit_schemas = bindings
            .privstack_plugin_plugin()
            .call_get_entity_schemas(&mut store)
            .map_err(|e| {
                PluginHostError::PluginCrashed {
                    plugin_id: metadata.id.clone(),
                    message: format!("get_entity_schemas() failed: {}", e),
                }
            })?;

        let schemas = convert_wit_schemas(&wit_schemas);
        let declared_entity_types: HashSet<String> =
            schemas.iter().map(|s| s.entity_type.clone()).collect();

        for schema in &schemas {
            schema
                .to_core_schema()
                .map_err(|e| PluginHostError::InvalidSchema(format!("{}: {}", metadata.id, e)))?;
        }

        store.data_mut().declared_entity_types = declared_entity_types;
        store.data_mut().schemas = schemas;

        let has_linkable_item_provider = true;
        let has_deep_link_target = true;
        let has_timer = true;
        let has_shutdown_aware = true;

        info!(
            plugin_id = %metadata.id,
            linkable = has_linkable_item_provider,
            deep_link = has_deep_link_target,
            timer = has_timer,
            shutdown_aware = has_shutdown_aware,
            "Wasm component loaded"
        );

        let mut sandbox = Self {
            metadata,
            resource_limits,
            has_linkable_item_provider,
            cached_link_type: None,
            has_deep_link_target,
            has_timer,
            has_shutdown_aware,
            runtime: Some(WasmRuntime {
                _engine: engine.clone(),
                store,
                bindings,
            }),
            standalone_state: None,
            last_fuel_consumed: 0,
        };

        if has_linkable_item_provider {
            sandbox.cached_link_type = sandbox.call_link_type().ok();
        }

        Ok(sandbox)
    }

    /// Returns `true` when backed by a real Wasm component (not metadata-only).
    pub fn has_runtime(&self) -> bool {
        self.runtime.is_some()
    }

    // ================================================================
    // State accessors
    // ================================================================

    fn runtime_mut(&mut self) -> Result<&mut WasmRuntime, PluginHostError> {
        self.runtime.as_mut().ok_or_else(|| PluginHostError::PluginCrashed {
            plugin_id: self.metadata.id.clone(),
            message: "no Wasm runtime (metadata-only sandbox)".into(),
        })
    }

    fn state_ref(&self) -> &PluginState {
        if let Some(rt) = &self.runtime {
            rt.store.data()
        } else {
            self.standalone_state.as_ref().expect("sandbox must have either runtime or standalone_state")
        }
    }

    fn state_mut_ref(&mut self) -> &mut PluginState {
        if let Some(rt) = &mut self.runtime {
            rt.store.data_mut()
        } else {
            self.standalone_state.as_mut().expect("sandbox must have either runtime or standalone_state")
        }
    }

    /// Public accessor for plugin state (for permission checks etc.)
    pub fn state(&self) -> &PluginState {
        self.state_ref()
    }

    // ================================================================
    // Resource metrics
    // ================================================================

    /// Get resource usage metrics for this plugin sandbox.
    pub fn get_resource_metrics(&self) -> PluginResourceMetrics {
        let memory_limit_bytes = self.resource_limits.max_memory_bytes;

        // Get actual memory usage from the tracking limiter
        let memory_used_bytes = self.state_ref().limiter.current_memory_bytes();

        let memory_usage_ratio = if memory_limit_bytes > 0 {
            (memory_used_bytes as f64) / (memory_limit_bytes as f64)
        } else {
            0.0
        };

        // Entity count and disk usage - query from entity store
        let state = self.state_ref();
        let mut entity_count = 0usize;
        let mut disk_usage_bytes = 0usize;

        for schema in &state.schemas {
            if let Ok(count) = state.entity_store.count_entities(&schema.entity_type, true) {
                entity_count += count;
            }
            if let Ok(bytes) = state.entity_store.estimate_storage_bytes(&schema.entity_type) {
                disk_usage_bytes += bytes;
            }
        }

        // Get fuel metrics from database (average, peak, count)
        let (fuel_average_last_1000, fuel_peak, fuel_history_count) = state
            .entity_store
            .get_fuel_metrics(&self.metadata.id)
            .unwrap_or((0, 0, 0));

        PluginResourceMetrics {
            memory_used_bytes,
            memory_limit_bytes,
            memory_usage_ratio,
            fuel_consumed_last_call: self.last_fuel_consumed,
            fuel_budget_per_call: self.resource_limits.fuel_per_call,
            fuel_average_last_1000,
            fuel_peak,
            fuel_history_count,
            entity_count,
            disk_usage_bytes,
        }
    }

    /// Track fuel consumption after a plugin call.
    /// Persists to database for historical metrics (average, peak, count).
    fn track_fuel_consumption(&mut self) {
        if let Some(rt) = &self.runtime {
            match rt.store.get_fuel() {
                Ok(remaining) => {
                    let budget = self.resource_limits.fuel_per_call;
                    self.last_fuel_consumed = budget.saturating_sub(remaining);

                    // Persist to database (maintains rolling window of 1000 entries)
                    let plugin_id = self.metadata.id.clone();
                    let fuel = self.last_fuel_consumed;
                    if let Err(e) = self.state_ref().entity_store.record_fuel_consumption(&plugin_id, fuel) {
                        warn!(
                            plugin_id = %plugin_id,
                            error = %e,
                            "Failed to record fuel consumption to database"
                        );
                    }

                    debug!(
                        plugin_id = %self.metadata.id,
                        budget = budget,
                        remaining = remaining,
                        consumed = self.last_fuel_consumed,
                        "Fuel consumption tracked"
                    );
                }
                Err(e) => {
                    warn!(
                        plugin_id = %self.metadata.id,
                        error = %e,
                        "Failed to get fuel remaining"
                    );
                    self.last_fuel_consumed = 0;
                }
            }
        }
    }

    // ================================================================
    // Plugin lifecycle calls
    // ================================================================

    /// Call the plugin's `initialize()` export.
    pub fn call_initialize(&mut self) -> Result<bool, PluginHostError> {
        let fuel = self.resource_limits.fuel_per_call;
        let pid = self.metadata.id.clone();
        let rt = self.runtime_mut()?;
        rt.store.set_fuel(fuel).ok();
        let result = rt
            .bindings
            .privstack_plugin_plugin()
            .call_initialize(&mut rt.store);
        self.track_fuel_consumption();
        result.map_err(|e| PluginHostError::PluginCrashed {
            plugin_id: pid,
            message: format!("initialize() failed: {}", e),
        })
    }

    /// Call the plugin's `activate()` export.
    pub fn call_activate(&mut self) -> Result<(), PluginHostError> {
        let fuel = self.resource_limits.fuel_per_call;
        let pid = self.metadata.id.clone();
        let rt = self.runtime_mut()?;
        rt.store.set_fuel(fuel).ok();
        let result = rt
            .bindings
            .privstack_plugin_plugin()
            .call_activate(&mut rt.store);
        self.track_fuel_consumption();
        result.map_err(|e| PluginHostError::PluginCrashed {
            plugin_id: pid,
            message: format!("activate() failed: {}", e),
        })
    }

    /// Call the plugin's `deactivate()` export.
    pub fn call_deactivate(&mut self) -> Result<(), PluginHostError> {
        let fuel = self.resource_limits.fuel_per_call;
        let pid = self.metadata.id.clone();
        let rt = self.runtime_mut()?;
        rt.store.set_fuel(fuel).ok();
        let result = rt
            .bindings
            .privstack_plugin_plugin()
            .call_deactivate(&mut rt.store);
        self.track_fuel_consumption();
        result.map_err(|e| PluginHostError::PluginCrashed {
            plugin_id: pid,
            message: format!("deactivate() failed: {}", e),
        })
    }

    /// Call the plugin's `on_navigated_to()` export.
    pub fn call_on_navigated_to(&mut self) -> Result<(), PluginHostError> {
        let fuel = self.resource_limits.fuel_per_call;
        let pid = self.metadata.id.clone();
        let rt = self.runtime_mut()?;
        rt.store.set_fuel(fuel).ok();
        let result = rt
            .bindings
            .privstack_plugin_plugin()
            .call_on_navigated_to(&mut rt.store);
        self.track_fuel_consumption();
        result.map_err(|e| PluginHostError::PluginCrashed {
            plugin_id: pid,
            message: format!("on_navigated_to() failed: {}", e),
        })
    }

    /// Call the plugin's `on_navigated_from()` export.
    pub fn call_on_navigated_from(&mut self) -> Result<(), PluginHostError> {
        let fuel = self.resource_limits.fuel_per_call;
        let pid = self.metadata.id.clone();
        let rt = self.runtime_mut()?;
        rt.store.set_fuel(fuel).ok();
        let result = rt
            .bindings
            .privstack_plugin_plugin()
            .call_on_navigated_from(&mut rt.store);
        self.track_fuel_consumption();
        result.map_err(|e| PluginHostError::PluginCrashed {
            plugin_id: pid,
            message: format!("on_navigated_from() failed: {}", e),
        })
    }

    /// Call the plugin's `dispose()` export.
    pub fn call_dispose(&mut self) -> Result<(), PluginHostError> {
        let fuel = self.resource_limits.fuel_per_call;
        let pid = self.metadata.id.clone();
        let rt = self.runtime_mut()?;
        rt.store.set_fuel(fuel).ok();
        let result = rt
            .bindings
            .privstack_plugin_plugin()
            .call_dispose(&mut rt.store);
        self.track_fuel_consumption();
        result.map_err(|e| PluginHostError::PluginCrashed {
            plugin_id: pid,
            message: format!("dispose() failed: {}", e),
        })
    }

    /// Call the plugin's `get_view_state()` export.
    pub fn call_get_view_state(&mut self) -> Result<String, PluginHostError> {
        let fuel = self.resource_limits.fuel_per_call;
        let pid = self.metadata.id.clone();
        let rt = self.runtime_mut()?;
        rt.store.set_fuel(fuel).ok();
        let result = rt
            .bindings
            .privstack_plugin_plugin()
            .call_get_view_state(&mut rt.store);
        self.track_fuel_consumption();
        let state = result.map_err(|e| PluginHostError::PluginCrashed {
            plugin_id: pid,
            message: format!("get_view_state() failed: {}", e),
        })?;
        let rt = self.runtime.as_mut().expect("runtime exists");
        rt.store.data_mut().view_state = Some(state.clone());
        rt.store.data_mut().state_dirty = false;
        Ok(state)
    }

    /// Call the plugin's `get_view_data()` export (template-data-provider capability).
    /// Returns raw JSON data model for host-side template evaluation.
    pub fn call_get_view_data(&mut self) -> Result<String, PluginHostError> {
        let fuel = self.resource_limits.fuel_per_call;
        let pid = self.metadata.id.clone();
        let rt = self.runtime_mut()?;
        rt.store.set_fuel(fuel).ok();
        let result = rt
            .bindings
            .privstack_plugin_template_data_provider()
            .call_get_view_data(&mut rt.store);
        self.track_fuel_consumption();
        result.map_err(|e| PluginHostError::PluginCrashed {
            plugin_id: pid,
            message: format!("get_view_data() failed: {}", e),
        })
    }

    /// Call the plugin's `handle_command()` export.
    pub fn call_handle_command(
        &mut self,
        name: &str,
        args: &str,
    ) -> Result<String, PluginHostError> {
        let fuel = self.resource_limits.fuel_per_call;
        let pid = self.metadata.id.clone();
        let name_owned = name.to_string();
        let rt = self.runtime_mut()?;
        rt.store.set_fuel(fuel).ok();
        let result = rt
            .bindings
            .privstack_plugin_plugin()
            .call_handle_command(&mut rt.store, name, args);
        self.track_fuel_consumption();
        result.map_err(|e| PluginHostError::PluginCrashed {
            plugin_id: pid,
            message: format!("handle_command('{}') failed: {}", name_owned, e),
        })
    }

    /// Call the plugin's `get_navigation_item()` export.
    pub fn call_get_navigation_item(&mut self) -> Result<Option<WitNavigationItem>, PluginHostError> {
        let fuel = self.resource_limits.fuel_per_call;
        let pid = self.metadata.id.clone();
        let rt = self.runtime_mut()?;
        rt.store.set_fuel(fuel).ok();
        let result = rt
            .bindings
            .privstack_plugin_plugin()
            .call_get_navigation_item(&mut rt.store);
        self.track_fuel_consumption();
        let result = result.map_err(|e| PluginHostError::PluginCrashed {
            plugin_id: pid,
            message: format!("get_navigation_item() failed: {}", e),
        })?;
        Ok(result.map(|n| convert_wit_nav_item(&n)))
    }

    /// Call the plugin's `get_commands()` export.
    pub fn call_get_commands(&mut self) -> Result<Vec<WitCommandDefinition>, PluginHostError> {
        let fuel = self.resource_limits.fuel_per_call;
        let pid = self.metadata.id.clone();
        let rt = self.runtime_mut()?;
        rt.store.set_fuel(fuel).ok();
        let result = rt
            .bindings
            .privstack_plugin_plugin()
            .call_get_commands(&mut rt.store);
        self.track_fuel_consumption();
        let cmds = result.map_err(|e| PluginHostError::PluginCrashed {
            plugin_id: pid,
            message: format!("get_commands() failed: {}", e),
        })?;
        Ok(cmds.into_iter().map(|c| convert_wit_command(&c)).collect())
    }

    // ================================================================
    // Capability exports (optional)
    // ================================================================

    /// Search linkable items if the plugin exports the capability.
    pub fn call_search_linkable_items(
        &mut self,
        query: &str,
        max_results: u32,
    ) -> Result<Vec<WitLinkableItem>, PluginHostError> {
        let fuel = self.resource_limits.fuel_per_call;
        let pid = self.metadata.id.clone();
        let rt = self.runtime_mut()?;
        rt.store.set_fuel(fuel).ok();
        let result = rt
            .bindings
            .privstack_plugin_linkable_item_provider()
            .call_search_items(&mut rt.store, query, max_results);
        self.track_fuel_consumption();
        let items = result.map_err(|e| PluginHostError::PluginCrashed {
            plugin_id: pid,
            message: format!("linkable search failed: {}", e),
        })?;
        Ok(items.into_iter().map(|i| convert_wit_linkable_item(&i)).collect())
    }

    /// Get the plugin's self-reported link type from the linkable-item-provider export.
    pub fn call_link_type(&mut self) -> Result<String, PluginHostError> {
        let fuel = self.resource_limits.fuel_per_call;
        let pid = self.metadata.id.clone();
        let rt = self.runtime_mut()?;
        rt.store.set_fuel(fuel).ok();
        let result = rt
            .bindings
            .privstack_plugin_linkable_item_provider()
            .call_link_type(&mut rt.store);
        self.track_fuel_consumption();
        result.map_err(|e| PluginHostError::PluginCrashed {
            plugin_id: pid,
            message: format!("link_type() failed: {}", e),
        })
    }

    /// Navigate to a specific item via the deep-link-target export.
    pub fn call_navigate_to_item(
        &mut self,
        item_id: &str,
    ) -> Result<(), PluginHostError> {
        let fuel = self.resource_limits.fuel_per_call;
        let pid = self.metadata.id.clone();
        let rt = self.runtime_mut()?;
        rt.store.set_fuel(fuel).ok();
        let result = rt
            .bindings
            .privstack_plugin_deep_link_target()
            .call_navigate_to_item(&mut rt.store, item_id);
        self.track_fuel_consumption();
        result.map_err(|e| PluginHostError::PluginCrashed {
            plugin_id: pid,
            message: format!("navigate_to_item failed: {}", e),
        })
    }

    // ================================================================
    // SDK message routing (host-side, for backward compat with FFI path)
    // ================================================================

    /// Handle an SDK message from the plugin. Enforces entity-type scoping.
    pub fn handle_sdk_send(&self, message: &WitSdkMessage) -> WitSdkResponse {
        if let Err(e) = self.state_ref().check_entity_type_access(&message.entity_type) {
            warn!(
                plugin_id = %self.metadata.id,
                entity_type = %message.entity_type,
                "Entity type access denied"
            );
            return WitSdkResponse::err(403, e.to_string());
        }

        if let Err(e) = serde_json::to_string(message) {
            return WitSdkResponse::err(500, format!("serialization error: {e}"));
        }

        debug!(
            plugin_id = %self.metadata.id,
            action = ?message.action,
            entity_type = %message.entity_type,
            "SDK send"
        );

        WitSdkResponse::ok(None)
    }

    /// Handle a settings get request.
    pub fn handle_settings_get(&self, key: &str, default: &str) -> String {
        self.state_ref()
            .settings
            .get(key)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    /// Handle a settings set request.
    pub fn handle_settings_set(&mut self, key: &str, value: &str) {
        self.state_mut_ref()
            .settings
            .insert(key.to_string(), value.to_string());
    }

    /// Handle a settings remove request.
    pub fn handle_settings_remove(&mut self, key: &str) {
        self.state_mut_ref().settings.remove(key);
    }

    /// Check if a vault operation is permitted.
    pub fn check_vault_access(&self) -> Result<(), PluginHostError> {
        self.state_ref().check_permission(Permission::Vault)
    }

    /// Check if linking operations are permitted.
    pub fn check_linking_access(&self) -> Result<(), PluginHostError> {
        self.state_ref().check_permission(Permission::Linking)
    }

    /// Check if dialog operations are permitted.
    pub fn check_dialog_access(&self) -> Result<(), PluginHostError> {
        self.state_ref().check_permission(Permission::Dialogs)
    }

    /// Returns the plugin's declared entity types.
    pub fn declared_entity_types(&self) -> &HashSet<String> {
        &self.state_ref().declared_entity_types
    }

    /// Replaces the permission set at runtime (e.g. after user toggles in Settings).
    pub fn update_permissions(&mut self, permissions: PermissionSet) {
        self.state_mut_ref().permissions = permissions;
    }

    /// Returns the plugin ID.
    pub fn plugin_id(&self) -> &str {
        &self.metadata.id
    }

    /// Returns whether the view state is dirty.
    pub fn is_state_dirty(&self) -> bool {
        self.state_ref().state_dirty
    }

    /// Returns a mutable reference to the plugin state.
    pub fn state_mut(&mut self) -> &mut PluginState {
        self.state_mut_ref()
    }
}

// ============================================================
// WIT type conversions: generated bindings <-> our WitPluginMetadata etc.
// ============================================================

fn convert_wit_metadata(
    m: &crate::bindings::privstack::plugin::types::PluginMetadata,
) -> WitPluginMetadata {
    WitPluginMetadata {
        id: m.id.clone(),
        name: m.name.clone(),
        description: m.description.clone(),
        version: m.version.clone(),
        author: m.author.clone(),
        icon: m.icon.clone(),
        navigation_order: m.navigation_order,
        category: match m.category {
            crate::bindings::privstack::plugin::types::PluginCategory::Productivity => {
                WitPluginCategory::Productivity
            }
            crate::bindings::privstack::plugin::types::PluginCategory::Security => {
                WitPluginCategory::Security
            }
            crate::bindings::privstack::plugin::types::PluginCategory::Communication => {
                WitPluginCategory::Communication
            }
            crate::bindings::privstack::plugin::types::PluginCategory::Information => {
                WitPluginCategory::Information
            }
            crate::bindings::privstack::plugin::types::PluginCategory::Utility => {
                WitPluginCategory::Utility
            }
            crate::bindings::privstack::plugin::types::PluginCategory::Extension => {
                WitPluginCategory::Extension
            }
        },
        can_disable: m.can_disable,
        is_experimental: m.is_experimental,
    }
}

fn convert_wit_schemas(
    schemas: &[crate::bindings::privstack::plugin::types::EntitySchema],
) -> Vec<WitEntitySchema> {
    schemas
        .iter()
        .map(|s| WitEntitySchema {
            entity_type: s.entity_type.clone(),
            indexed_fields: s
                .indexed_fields
                .iter()
                .map(|f| WitIndexedField {
                    field_path: f.field_path.clone(),
                    field_type: match f.field_type {
                        crate::bindings::privstack::plugin::types::FieldType::Text => {
                            WitFieldType::Text
                        }
                        crate::bindings::privstack::plugin::types::FieldType::Tag => {
                            WitFieldType::Tag
                        }
                        crate::bindings::privstack::plugin::types::FieldType::DateTime => {
                            WitFieldType::DateTime
                        }
                        crate::bindings::privstack::plugin::types::FieldType::Number => {
                            WitFieldType::Number
                        }
                        crate::bindings::privstack::plugin::types::FieldType::Boolean => {
                            WitFieldType::Boolean
                        }
                        crate::bindings::privstack::plugin::types::FieldType::Vector => {
                            WitFieldType::Vector
                        }
                        crate::bindings::privstack::plugin::types::FieldType::Counter => {
                            WitFieldType::Counter
                        }
                        crate::bindings::privstack::plugin::types::FieldType::Relation => {
                            WitFieldType::Relation
                        }
                        crate::bindings::privstack::plugin::types::FieldType::Decimal => {
                            WitFieldType::Decimal
                        }
                        crate::bindings::privstack::plugin::types::FieldType::Json => {
                            WitFieldType::Json
                        }
                        crate::bindings::privstack::plugin::types::FieldType::Enumeration => {
                            WitFieldType::Enumeration
                        }
                        crate::bindings::privstack::plugin::types::FieldType::GeoPoint => {
                            WitFieldType::GeoPoint
                        }
                        crate::bindings::privstack::plugin::types::FieldType::Duration => {
                            WitFieldType::Duration
                        }
                    },
                    searchable: f.searchable,
                    vector_dim: f.vector_dim,
                    enum_options: f.enum_options.clone(),
                })
                .collect(),
            merge_strategy: match s.merge_strategy {
                crate::bindings::privstack::plugin::types::MergeStrategy::LwwDocument => {
                    WitMergeStrategy::LwwDocument
                }
                crate::bindings::privstack::plugin::types::MergeStrategy::LwwPerField => {
                    WitMergeStrategy::LwwPerField
                }
                crate::bindings::privstack::plugin::types::MergeStrategy::Custom => {
                    WitMergeStrategy::Custom
                }
            },
        })
        .collect()
}

fn convert_wit_nav_item(
    n: &crate::bindings::privstack::plugin::types::NavigationItem,
) -> WitNavigationItem {
    WitNavigationItem {
        id: n.id.clone(),
        display_name: n.display_name.clone(),
        subtitle: n.subtitle.clone(),
        icon: n.icon.clone(),
        tooltip: n.tooltip.clone(),
        order: n.order,
        show_badge: n.show_badge,
        badge_count: n.badge_count,
        shortcut_hint: n.shortcut_hint.clone(),
    }
}

fn convert_wit_command(
    c: &crate::bindings::privstack::plugin::types::CommandDefinition,
) -> WitCommandDefinition {
    WitCommandDefinition {
        name: c.name.clone(),
        description: c.description.clone(),
        keywords: c.keywords.clone(),
        category: c.category.clone(),
        icon: c.icon.clone(),
    }
}

fn convert_wit_linkable_item(
    i: &crate::bindings::privstack::plugin::types::LinkableItem,
) -> WitLinkableItem {
    WitLinkableItem {
        id: i.id.clone(),
        link_type: i.link_type.clone(),
        title: i.title.clone(),
        subtitle: i.subtitle.clone(),
        icon: i.icon.clone(),
        modified_at: i.modified_at,
        plugin_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::PermissionSet;

    fn test_stores() -> (Arc<privstack_storage::EntityStore>, Arc<privstack_storage::EventStore>) {
        let es = privstack_storage::EntityStore::open_in_memory().unwrap();
        let ev = privstack_storage::EventStore::open_in_memory().unwrap();
        (Arc::new(es), Arc::new(ev))
    }

    fn test_metadata() -> WitPluginMetadata {
        WitPluginMetadata {
            id: "test.plugin".into(),
            name: "Test Plugin".into(),
            description: "A test plugin".into(),
            version: "0.1.0".into(),
            author: "Test".into(),
            icon: None,
            navigation_order: 100,
            category: WitPluginCategory::Utility,
            can_disable: true,
            is_experimental: false,
        }
    }

    fn test_schemas() -> Vec<WitEntitySchema> {
        vec![WitEntitySchema {
            entity_type: "test_item".into(),
            indexed_fields: vec![WitIndexedField {
                field_path: "/title".into(),
                field_type: WitFieldType::Text,
                searchable: true,
                vector_dim: None,
                enum_options: None,
            }],
            merge_strategy: WitMergeStrategy::LwwPerField,
        }]
    }

    #[test]
    fn sandbox_creation() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert_eq!(sandbox.plugin_id(), "test.plugin");
        assert!(sandbox.declared_entity_types().contains("test_item"));
    }

    #[test]
    fn entity_type_scoping_enforced() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        // Allowed entity type
        let msg = WitSdkMessage {
            action: WitSdkAction::Create,
            entity_type: "test_item".into(),
            entity_id: None,
            payload: Some("{}".into()),
            parameters: vec![],
            source: None,
        };
        let resp = sandbox.handle_sdk_send(&msg);
        assert!(resp.success);

        // Disallowed entity type
        let bad_msg = WitSdkMessage {
            entity_type: "other_type".into(),
            ..msg
        };
        let resp = sandbox.handle_sdk_send(&bad_msg);
        assert!(!resp.success);
        assert_eq!(resp.error_code, Some(403));
    }

    #[test]
    fn permission_checks() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(), // No vault permission
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert!(sandbox.check_vault_access().is_err());
        assert!(sandbox.check_linking_access().is_err());
    }

    #[test]
    fn settings_crud() {
        let (es, ev) = test_stores();
        let mut sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert_eq!(sandbox.handle_settings_get("key", "default"), "default");
        sandbox.handle_settings_set("key", "value");
        assert_eq!(sandbox.handle_settings_get("key", "default"), "value");
        sandbox.handle_settings_remove("key");
        assert_eq!(sandbox.handle_settings_get("key", "default"), "default");
    }

    #[test]
    fn vector_field_without_dim_accepted() {
        let (es, ev) = test_stores();
        let schemas = vec![WitEntitySchema {
            entity_type: "bad".into(),
            indexed_fields: vec![WitIndexedField {
                field_path: "/embedding".into(),
                field_type: WitFieldType::Vector,
                searchable: false,
                vector_dim: None,
                enum_options: None,
            }],
            merge_strategy: WitMergeStrategy::LwwDocument,
        }];

        let result = PluginSandbox::new(
            test_metadata(),
            schemas,
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        );
        assert!(result.is_ok());
    }

    // ================================================================
    // ResourceLimits tests
    // ================================================================

    #[test]
    fn first_party_limits_values() {
        let limits = ResourceLimits::first_party();
        assert_eq!(limits.max_memory_bytes, 64 * 1024 * 1024);
        assert_eq!(limits.fuel_per_call, 1_000_000_000);
        assert_eq!(limits.call_timeout_ms, 5_000);
        assert_eq!(limits.shutdown_deadline_ms, 2_000);
    }

    #[test]
    fn third_party_limits_values() {
        let limits = ResourceLimits::third_party();
        assert_eq!(limits.max_memory_bytes, 32 * 1024 * 1024);
        assert_eq!(limits.fuel_per_call, 500_000_000);
        assert_eq!(limits.call_timeout_ms, 3_000);
        assert_eq!(limits.shutdown_deadline_ms, 2_000);
    }

    #[test]
    fn third_party_limits_stricter_than_first_party() {
        let fp = ResourceLimits::first_party();
        let tp = ResourceLimits::third_party();
        assert!(tp.max_memory_bytes < fp.max_memory_bytes);
        assert!(tp.fuel_per_call < fp.fuel_per_call);
        assert!(tp.call_timeout_ms <= fp.call_timeout_ms);
    }

    // ================================================================
    // Entity type access checks
    // ================================================================

    #[test]
    fn check_entity_type_access_allowed() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert!(sandbox.state().check_entity_type_access("test_item").is_ok());
    }

    #[test]
    fn check_entity_type_access_denied() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        let result = sandbox.state().check_entity_type_access("forbidden_type");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("forbidden_type"));
        assert!(err.to_string().contains("test.plugin"));
    }

    // ================================================================
    // Permission check tests
    // ================================================================

    #[test]
    fn check_permission_granted() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::all_granted(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert!(sandbox.state().check_permission(Permission::Vault).is_ok());
        assert!(sandbox.state().check_permission(Permission::Network).is_ok());
        assert!(sandbox.state().check_permission(Permission::Linking).is_ok());
    }

    #[test]
    fn check_permission_denied_returns_error() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        let err = sandbox.state().check_permission(Permission::Network).unwrap_err();
        assert!(err.to_string().contains("network"));
        assert!(err.to_string().contains("test.plugin"));
    }

    #[test]
    fn check_dialog_access() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert!(sandbox.check_dialog_access().is_err());

        // With all granted
        let (es2, ev2) = test_stores();
        let sandbox2 = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::all_granted(),
            ResourceLimits::first_party(),
            es2,
            ev2,
        )
        .unwrap();
        assert!(sandbox2.check_dialog_access().is_ok());
    }

    // ================================================================
    // Settings CRUD (extended)
    // ================================================================

    #[test]
    fn settings_overwrite_existing_key() {
        let (es, ev) = test_stores();
        let mut sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        sandbox.handle_settings_set("k", "v1");
        assert_eq!(sandbox.handle_settings_get("k", ""), "v1");
        sandbox.handle_settings_set("k", "v2");
        assert_eq!(sandbox.handle_settings_get("k", ""), "v2");
    }

    #[test]
    fn settings_remove_nonexistent_key_is_noop() {
        let (es, ev) = test_stores();
        let mut sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        // Should not panic
        sandbox.handle_settings_remove("nonexistent");
    }

    #[test]
    fn settings_multiple_keys() {
        let (es, ev) = test_stores();
        let mut sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        sandbox.handle_settings_set("a", "1");
        sandbox.handle_settings_set("b", "2");
        sandbox.handle_settings_set("c", "3");
        assert_eq!(sandbox.handle_settings_get("a", ""), "1");
        assert_eq!(sandbox.handle_settings_get("b", ""), "2");
        assert_eq!(sandbox.handle_settings_get("c", ""), "3");
    }

    // ================================================================
    // update_permissions
    // ================================================================

    #[test]
    fn update_permissions_changes_access() {
        let (es, ev) = test_stores();
        let mut sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert!(sandbox.check_vault_access().is_err());

        sandbox.update_permissions(PermissionSet::all_granted());
        assert!(sandbox.check_vault_access().is_ok());
        assert!(sandbox.check_linking_access().is_ok());
    }

    // ================================================================
    // is_state_dirty
    // ================================================================

    #[test]
    fn state_dirty_initially_false() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert!(!sandbox.is_state_dirty());
    }

    #[test]
    fn state_dirty_set_via_state_mut() {
        let (es, ev) = test_stores();
        let mut sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        sandbox.state_mut().state_dirty = true;
        assert!(sandbox.is_state_dirty());
    }

    // ================================================================
    // Accessors
    // ================================================================

    #[test]
    fn plugin_id_accessor() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert_eq!(sandbox.plugin_id(), "test.plugin");
    }

    #[test]
    fn declared_entity_types_accessor() {
        let (es, ev) = test_stores();
        let schemas = vec![
            WitEntitySchema {
                entity_type: "type_a".into(),
                indexed_fields: vec![WitIndexedField {
                    field_path: "/name".into(),
                    field_type: WitFieldType::Text,
                    searchable: true,
                    vector_dim: None,
                    enum_options: None,
                }],
                merge_strategy: WitMergeStrategy::LwwPerField,
            },
            WitEntitySchema {
                entity_type: "type_b".into(),
                indexed_fields: vec![WitIndexedField {
                    field_path: "/title".into(),
                    field_type: WitFieldType::Text,
                    searchable: true,
                    vector_dim: None,
                    enum_options: None,
                }],
                merge_strategy: WitMergeStrategy::LwwDocument,
            },
        ];

        let sandbox = PluginSandbox::new(
            test_metadata(),
            schemas,
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        let types = sandbox.declared_entity_types();
        assert_eq!(types.len(), 2);
        assert!(types.contains("type_a"));
        assert!(types.contains("type_b"));
    }

    #[test]
    fn metadata_only_sandbox_has_no_runtime_capabilities() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert!(!sandbox.has_linkable_item_provider);
        assert!(!sandbox.has_deep_link_target);
        assert!(!sandbox.has_timer);
        assert!(!sandbox.has_shutdown_aware);
    }

    #[test]
    fn metadata_only_sandbox_runtime_calls_fail() {
        let (es, ev) = test_stores();
        let mut sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        // All Wasm export calls should fail on metadata-only sandboxes
        assert!(sandbox.call_initialize().is_err());
        assert!(sandbox.call_activate().is_err());
        assert!(sandbox.call_deactivate().is_err());
        assert!(sandbox.call_dispose().is_err());
        assert!(sandbox.call_on_navigated_to().is_err());
        assert!(sandbox.call_on_navigated_from().is_err());
        assert!(sandbox.call_get_view_state().is_err());
        assert!(sandbox.call_get_view_data().is_err());
        assert!(sandbox.call_handle_command("test", "{}").is_err());
        assert!(sandbox.call_get_navigation_item().is_err());
        assert!(sandbox.call_get_commands().is_err());
        assert!(sandbox.call_search_linkable_items("query", 10).is_err());
    }

    #[test]
    fn handle_sdk_send_with_valid_entity_type() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        let msg = WitSdkMessage {
            action: WitSdkAction::List,
            entity_type: "test_item".into(),
            entity_id: None,
            payload: None,
            parameters: vec![],
            source: None,
        };
        let resp = sandbox.handle_sdk_send(&msg);
        assert!(resp.success);
    }

    #[test]
    fn pending_navigation_initially_none() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert!(sandbox.state().pending_navigation.is_none());
    }

    #[test]
    fn view_state_initially_none() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert!(sandbox.state().view_state.is_none());
    }

    #[test]
    fn schemas_stored_in_state() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert_eq!(sandbox.state().schemas.len(), 1);
        assert_eq!(sandbox.state().schemas[0].entity_type, "test_item");
    }

    // ================================================================
    // ResourceLimits derived trait coverage
    // ================================================================

    #[test]
    fn resource_limits_debug_impl() {
        let limits = ResourceLimits::first_party();
        let debug_str = format!("{:?}", limits);
        assert!(debug_str.contains("ResourceLimits"));
        assert!(debug_str.contains("max_memory_bytes"));
    }

    #[test]
    fn resource_limits_clone_impl() {
        let limits = ResourceLimits::first_party();
        let cloned = limits.clone();
        assert_eq!(cloned.max_memory_bytes, limits.max_memory_bytes);
        assert_eq!(cloned.fuel_per_call, limits.fuel_per_call);
        assert_eq!(cloned.call_timeout_ms, limits.call_timeout_ms);
        assert_eq!(cloned.shutdown_deadline_ms, limits.shutdown_deadline_ms);
    }

    // ================================================================
    // from_wasm error path (file does not exist)
    // ================================================================

    #[test]
    fn from_wasm_nonexistent_file_returns_error() {
        let (es, ev) = test_stores();
        let result = PluginSandbox::from_wasm(
            Path::new("/nonexistent/path/plugin.wasm"),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        );
        match result {
            Err(e) => assert!(e.to_string().contains("failed to read")),
            Ok(_) => panic!("expected error for nonexistent file"),
        }
    }

    // ================================================================
    // Empty schemas
    // ================================================================

    #[test]
    fn sandbox_with_empty_schemas() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            vec![],
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert!(sandbox.declared_entity_types().is_empty());
        assert_eq!(sandbox.state().schemas.len(), 0);
    }

    // ================================================================
    // Metadata with icon
    // ================================================================

    #[test]
    fn sandbox_metadata_with_icon() {
        let (es, ev) = test_stores();
        let mut meta = test_metadata();
        meta.icon = Some("icon-notes".into());
        meta.is_experimental = true;
        meta.can_disable = false;

        let sandbox = PluginSandbox::new(
            meta,
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert_eq!(sandbox.metadata.icon.as_deref(), Some("icon-notes"));
        assert!(sandbox.metadata.is_experimental);
        assert!(!sandbox.metadata.can_disable);
    }

    // ================================================================
    // handle_sdk_send with all action variants
    // ================================================================

    #[test]
    fn handle_sdk_send_with_read_action() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        let msg = WitSdkMessage {
            action: WitSdkAction::Read,
            entity_type: "test_item".into(),
            entity_id: Some("some-id".into()),
            payload: None,
            parameters: vec![],
            source: Some("test".into()),
        };
        let resp = sandbox.handle_sdk_send(&msg);
        assert!(resp.success);
    }

    #[test]
    fn handle_sdk_send_with_update_action() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        let msg = WitSdkMessage {
            action: WitSdkAction::Update,
            entity_type: "test_item".into(),
            entity_id: Some("id".into()),
            payload: Some(r#"{"title":"updated"}"#.into()),
            parameters: vec![],
            source: None,
        };
        let resp = sandbox.handle_sdk_send(&msg);
        assert!(resp.success);
    }

    #[test]
    fn handle_sdk_send_with_delete_action() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        let msg = WitSdkMessage {
            action: WitSdkAction::Delete,
            entity_type: "test_item".into(),
            entity_id: Some("id".into()),
            payload: None,
            parameters: vec![],
            source: None,
        };
        let resp = sandbox.handle_sdk_send(&msg);
        assert!(resp.success);
    }

    #[test]
    fn handle_sdk_send_with_query_action() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        let msg = WitSdkMessage {
            action: WitSdkAction::Query,
            entity_type: "test_item".into(),
            entity_id: None,
            payload: Some(r#"{"filter":"active"}"#.into()),
            parameters: vec![("limit".into(), "10".into())],
            source: None,
        };
        let resp = sandbox.handle_sdk_send(&msg);
        assert!(resp.success);
    }

    #[test]
    fn handle_sdk_send_with_trash_action() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        let msg = WitSdkMessage {
            action: WitSdkAction::Trash,
            entity_type: "test_item".into(),
            entity_id: Some("id".into()),
            payload: None,
            parameters: vec![],
            source: None,
        };
        let resp = sandbox.handle_sdk_send(&msg);
        assert!(resp.success);
    }

    #[test]
    fn handle_sdk_send_with_restore_action() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        let msg = WitSdkMessage {
            action: WitSdkAction::Restore,
            entity_type: "test_item".into(),
            entity_id: Some("id".into()),
            payload: None,
            parameters: vec![],
            source: None,
        };
        let resp = sandbox.handle_sdk_send(&msg);
        assert!(resp.success);
    }

    #[test]
    fn handle_sdk_send_with_link_action() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        let msg = WitSdkMessage {
            action: WitSdkAction::Link,
            entity_type: "test_item".into(),
            entity_id: Some("id".into()),
            payload: None,
            parameters: vec![],
            source: None,
        };
        let resp = sandbox.handle_sdk_send(&msg);
        assert!(resp.success);
    }

    #[test]
    fn handle_sdk_send_with_semantic_search_action() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        let msg = WitSdkMessage {
            action: WitSdkAction::SemanticSearch,
            entity_type: "test_item".into(),
            entity_id: None,
            payload: Some("search query".into()),
            parameters: vec![],
            source: None,
        };
        let resp = sandbox.handle_sdk_send(&msg);
        assert!(resp.success);
    }

    // ================================================================
    // State manipulation via state_mut
    // ================================================================

    #[test]
    fn set_pending_navigation_via_state_mut() {
        let (es, ev) = test_stores();
        let mut sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        sandbox.state_mut().pending_navigation = Some("target.plugin".into());
        assert_eq!(
            sandbox.state().pending_navigation.as_deref(),
            Some("target.plugin")
        );
    }

    #[test]
    fn set_view_state_via_state_mut() {
        let (es, ev) = test_stores();
        let mut sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        sandbox.state_mut().view_state = Some(r#"{"count":42}"#.into());
        assert_eq!(
            sandbox.state().view_state.as_deref(),
            Some(r#"{"count":42}"#)
        );
    }

    // ================================================================
    // PluginState direct field access
    // ================================================================

    #[test]
    fn plugin_state_plugin_id_matches() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert_eq!(sandbox.state().plugin_id, "test.plugin");
    }

    #[test]
    fn plugin_state_entity_store_accessible() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        // Verify entity_store is accessible (Arc reference)
        let _store_ref = &sandbox.state().entity_store;
    }

    #[test]
    fn plugin_state_event_store_accessible() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        let _store_ref = &sandbox.state().event_store;
    }

    // ================================================================
    // runtime_mut error message content
    // ================================================================

    #[test]
    fn runtime_mut_error_contains_plugin_id() {
        let (es, ev) = test_stores();
        let mut sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        let err = sandbox.call_initialize().unwrap_err();
        assert!(err.to_string().contains("test.plugin"));
        assert!(err.to_string().contains("metadata-only"));
    }

    // ================================================================
    // All permission check helpers with granted permissions
    // ================================================================

    #[test]
    fn check_vault_access_granted() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::all_granted(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert!(sandbox.check_vault_access().is_ok());
    }

    #[test]
    fn check_linking_access_granted() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::all_granted(),
            ResourceLimits::first_party(),
            es,
            ev,
        )
        .unwrap();

        assert!(sandbox.check_linking_access().is_ok());
    }

    // ================================================================
    // All WitPluginCategory variants in metadata
    // ================================================================

    #[test]
    fn sandbox_with_productivity_category() {
        let (es, ev) = test_stores();
        let mut meta = test_metadata();
        meta.category = WitPluginCategory::Productivity;
        let sandbox = PluginSandbox::new(
            meta, test_schemas(), PermissionSet::default_first_party(),
            ResourceLimits::first_party(), es, ev,
        ).unwrap();
        assert_eq!(sandbox.plugin_id(), "test.plugin");
    }

    #[test]
    fn sandbox_with_security_category() {
        let (es, ev) = test_stores();
        let mut meta = test_metadata();
        meta.category = WitPluginCategory::Security;
        let sandbox = PluginSandbox::new(
            meta, test_schemas(), PermissionSet::default_first_party(),
            ResourceLimits::first_party(), es, ev,
        ).unwrap();
        assert_eq!(sandbox.plugin_id(), "test.plugin");
    }

    #[test]
    fn sandbox_with_communication_category() {
        let (es, ev) = test_stores();
        let mut meta = test_metadata();
        meta.category = WitPluginCategory::Communication;
        let sandbox = PluginSandbox::new(
            meta, test_schemas(), PermissionSet::default_first_party(),
            ResourceLimits::first_party(), es, ev,
        ).unwrap();
        assert_eq!(sandbox.plugin_id(), "test.plugin");
    }

    #[test]
    fn sandbox_with_information_category() {
        let (es, ev) = test_stores();
        let mut meta = test_metadata();
        meta.category = WitPluginCategory::Information;
        let sandbox = PluginSandbox::new(
            meta, test_schemas(), PermissionSet::default_first_party(),
            ResourceLimits::first_party(), es, ev,
        ).unwrap();
        assert_eq!(sandbox.plugin_id(), "test.plugin");
    }

    #[test]
    fn sandbox_with_extension_category() {
        let (es, ev) = test_stores();
        let mut meta = test_metadata();
        meta.category = WitPluginCategory::Extension;
        let sandbox = PluginSandbox::new(
            meta, test_schemas(), PermissionSet::default_first_party(),
            ResourceLimits::first_party(), es, ev,
        ).unwrap();
        assert_eq!(sandbox.plugin_id(), "test.plugin");
    }

    // ================================================================
    // All WitFieldType variants in schemas
    // ================================================================

    fn make_schema_with_field_type(field_type: WitFieldType) -> Vec<WitEntitySchema> {
        vec![WitEntitySchema {
            entity_type: "ft_test".into(),
            indexed_fields: vec![WitIndexedField {
                field_path: "/field".into(),
                field_type,
                searchable: false,
                vector_dim: if matches!(field_type, WitFieldType::Vector) { Some(128) } else { None },
                enum_options: if matches!(field_type, WitFieldType::Enumeration) {
                    Some(vec!["a".into(), "b".into()])
                } else {
                    None
                },
            }],
            merge_strategy: WitMergeStrategy::LwwPerField,
        }]
    }

    #[test]
    fn schema_with_tag_field_type() {
        let (es, ev) = test_stores();
        let result = PluginSandbox::new(
            test_metadata(), make_schema_with_field_type(WitFieldType::Tag),
            PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn schema_with_datetime_field_type() {
        let (es, ev) = test_stores();
        let result = PluginSandbox::new(
            test_metadata(), make_schema_with_field_type(WitFieldType::DateTime),
            PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn schema_with_number_field_type() {
        let (es, ev) = test_stores();
        let result = PluginSandbox::new(
            test_metadata(), make_schema_with_field_type(WitFieldType::Number),
            PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn schema_with_boolean_field_type() {
        let (es, ev) = test_stores();
        let result = PluginSandbox::new(
            test_metadata(), make_schema_with_field_type(WitFieldType::Boolean),
            PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn schema_with_counter_field_type() {
        let (es, ev) = test_stores();
        let result = PluginSandbox::new(
            test_metadata(), make_schema_with_field_type(WitFieldType::Counter),
            PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn schema_with_relation_field_type() {
        let (es, ev) = test_stores();
        let result = PluginSandbox::new(
            test_metadata(), make_schema_with_field_type(WitFieldType::Relation),
            PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn schema_with_decimal_field_type() {
        let (es, ev) = test_stores();
        let result = PluginSandbox::new(
            test_metadata(), make_schema_with_field_type(WitFieldType::Decimal),
            PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn schema_with_json_field_type() {
        let (es, ev) = test_stores();
        let result = PluginSandbox::new(
            test_metadata(), make_schema_with_field_type(WitFieldType::Json),
            PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn schema_with_enumeration_field_type() {
        let (es, ev) = test_stores();
        let result = PluginSandbox::new(
            test_metadata(), make_schema_with_field_type(WitFieldType::Enumeration),
            PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn schema_with_geopoint_field_type() {
        let (es, ev) = test_stores();
        let result = PluginSandbox::new(
            test_metadata(), make_schema_with_field_type(WitFieldType::GeoPoint),
            PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn schema_with_duration_field_type() {
        let (es, ev) = test_stores();
        let result = PluginSandbox::new(
            test_metadata(), make_schema_with_field_type(WitFieldType::Duration),
            PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn schema_with_vector_field_type_and_dim() {
        let (es, ev) = test_stores();
        let result = PluginSandbox::new(
            test_metadata(), make_schema_with_field_type(WitFieldType::Vector),
            PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
        );
        assert!(result.is_ok());
    }

    // ================================================================
    // WitMergeStrategy::Custom in schema
    // ================================================================

    #[test]
    fn schema_with_custom_merge_strategy() {
        let (es, ev) = test_stores();
        let schemas = vec![WitEntitySchema {
            entity_type: "custom_merge".into(),
            indexed_fields: vec![WitIndexedField {
                field_path: "/data".into(),
                field_type: WitFieldType::Text,
                searchable: true,
                vector_dim: None,
                enum_options: None,
            }],
            merge_strategy: WitMergeStrategy::Custom,
        }];
        let result = PluginSandbox::new(
            test_metadata(), schemas,
            PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
        );
        assert!(result.is_ok());
    }

    // ================================================================
    // Third-party resource limits used in sandbox
    // ================================================================

    #[test]
    fn sandbox_with_third_party_limits() {
        let (es, ev) = test_stores();
        let sandbox = PluginSandbox::new(
            test_metadata(),
            test_schemas(),
            PermissionSet::default_third_party(),
            ResourceLimits::third_party(),
            es,
            ev,
        )
        .unwrap();

        assert_eq!(sandbox.resource_limits.max_memory_bytes, 32 * 1024 * 1024);
        assert_eq!(sandbox.plugin_id(), "test.plugin");
    }

    // ================================================================
    // Multiple entity types interaction
    // ================================================================

    #[test]
    fn handle_sdk_send_checks_correct_entity_type_in_multi_schema() {
        let (es, ev) = test_stores();
        let schemas = vec![
            WitEntitySchema {
                entity_type: "note".into(),
                indexed_fields: vec![WitIndexedField {
                    field_path: "/title".into(),
                    field_type: WitFieldType::Text,
                    searchable: true,
                    vector_dim: None,
                    enum_options: None,
                }],
                merge_strategy: WitMergeStrategy::LwwPerField,
            },
            WitEntitySchema {
                entity_type: "task".into(),
                indexed_fields: vec![WitIndexedField {
                    field_path: "/name".into(),
                    field_type: WitFieldType::Text,
                    searchable: true,
                    vector_dim: None,
                    enum_options: None,
                }],
                merge_strategy: WitMergeStrategy::LwwPerField,
            },
        ];

        let sandbox = PluginSandbox::new(
            test_metadata(), schemas,
            PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
        ).unwrap();

        // Both entity types should be allowed
        let msg_note = WitSdkMessage {
            action: WitSdkAction::Create,
            entity_type: "note".into(),
            entity_id: None,
            payload: Some("{}".into()),
            parameters: vec![],
            source: None,
        };
        assert!(sandbox.handle_sdk_send(&msg_note).success);

        let msg_task = WitSdkMessage {
            action: WitSdkAction::Create,
            entity_type: "task".into(),
            entity_id: None,
            payload: Some("{}".into()),
            parameters: vec![],
            source: None,
        };
        assert!(sandbox.handle_sdk_send(&msg_task).success);

        // Other type should be denied
        let msg_other = WitSdkMessage {
            action: WitSdkAction::Create,
            entity_type: "calendar".into(),
            entity_id: None,
            payload: Some("{}".into()),
            parameters: vec![],
            source: None,
        };
        assert!(!sandbox.handle_sdk_send(&msg_other).success);
    }
}
