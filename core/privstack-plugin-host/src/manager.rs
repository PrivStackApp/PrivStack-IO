//! Central plugin lifecycle manager.
//!
//! Owns all active `PluginSandbox` instances, enforces policy, and
//! provides query/routing across plugins (e.g. linkable-item search,
//! command palette aggregation).

use crate::error::PluginHostError;
use crate::permissions::PermissionSet;
use crate::policy::PolicyEngine;
use crate::sandbox::{PluginSandbox, ResourceLimits};
use crate::wit_types::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use tracing::{info, warn};
use wasmtime::Engine;

/// Manages the lifecycle of all loaded plugins.
/// Creates a shared Wasmtime engine configured for plugin sandboxing.
fn create_shared_engine() -> Result<Engine, PluginHostError> {
    let mut config = wasmtime::Config::new();
    config.wasm_component_model(true);
    config.consume_fuel(true);
    Engine::new(&config).map_err(PluginHostError::Compilation)
}

pub struct PluginHostManager {
    plugins: HashMap<String, PluginSandbox>,
    policy_engine: PolicyEngine,
    entity_store: Arc<privstack_storage::EntityStore>,
    event_store: Arc<privstack_storage::EventStore>,
    /// Shared Wasmtime engine for all plugins — lazily created on first WASM load.
    engine: OnceLock<Engine>,
}

impl PluginHostManager {
    /// Returns a reference to the shared Wasmtime engine, creating it on first access.
    fn engine(&self) -> &Engine {
        self.engine.get_or_init(|| {
            create_shared_engine().expect("failed to create Wasmtime engine")
        })
    }

    pub fn new(
        entity_store: Arc<privstack_storage::EntityStore>,
        event_store: Arc<privstack_storage::EventStore>,
    ) -> Self {
        Self {
            plugins: HashMap::new(),
            policy_engine: PolicyEngine::load(),
            entity_store,
            event_store,
            engine: OnceLock::new(),
        }
    }

    /// Creates a manager with a default unrestricted policy (no filesystem access).
    #[cfg(test)]
    pub(crate) fn new_for_test(
        entity_store: Arc<privstack_storage::EntityStore>,
        event_store: Arc<privstack_storage::EventStore>,
    ) -> Self {
        Self::with_policy(entity_store, event_store, PolicyEngine::with_config(crate::policy::PolicyConfig::default()))
    }

    pub fn with_policy(
        entity_store: Arc<privstack_storage::EntityStore>,
        event_store: Arc<privstack_storage::EventStore>,
        policy_engine: PolicyEngine,
    ) -> Self {
        Self {
            plugins: HashMap::new(),
            policy_engine,
            entity_store,
            event_store,
            engine: OnceLock::new(),
        }
    }

    // ================================================================
    // Loading / Unloading
    // ================================================================

    /// Loads a plugin from pre-parsed metadata (metadata-only path).
    /// Used for sidecar JSON loading and testing.
    pub fn load_plugin(
        &mut self,
        metadata: WitPluginMetadata,
        schemas: Vec<WitEntitySchema>,
        permissions: PermissionSet,
        resource_limits: ResourceLimits,
    ) -> Result<(), PluginHostError> {
        let plugin_id = metadata.id.clone();

        // Policy check
        if !self.policy_engine.is_plugin_allowed(&plugin_id, None) {
            return Err(PluginHostError::PolicyDenied(format!(
                "plugin '{}' blocked by policy",
                plugin_id
            )));
        }

        if self.plugins.contains_key(&plugin_id) {
            return Err(PluginHostError::PluginAlreadyLoaded(plugin_id));
        }

        let sandbox = PluginSandbox::new(
            metadata,
            schemas,
            permissions,
            resource_limits,
            Arc::clone(&self.entity_store),
            Arc::clone(&self.event_store),
        )?;

        info!(plugin_id = %plugin_id, "Plugin loaded (metadata-only)");
        self.plugins.insert(plugin_id, sandbox);
        Ok(())
    }

    /// Loads a plugin from a compiled .wasm component file.
    /// Instantiates the component, calls get_metadata/get_entity_schemas exports,
    /// and detects capability interfaces.
    pub fn load_plugin_from_wasm(
        &mut self,
        wasm_path: &Path,
        permissions: PermissionSet,
        resource_limits: ResourceLimits,
    ) -> Result<String, PluginHostError> {
        let sandbox = PluginSandbox::from_wasm_cached(
            wasm_path,
            self.engine(),
            permissions,
            resource_limits,
            Arc::clone(&self.entity_store),
            Arc::clone(&self.event_store),
        )?;

        let plugin_id = sandbox.metadata.id.clone();

        // Policy check
        if !self.policy_engine.is_plugin_allowed(&plugin_id, None) {
            return Err(PluginHostError::PolicyDenied(format!(
                "plugin '{}' blocked by policy",
                plugin_id
            )));
        }

        if self.plugins.contains_key(&plugin_id) {
            return Err(PluginHostError::PluginAlreadyLoaded(plugin_id));
        }

        info!(plugin_id = %plugin_id, "Plugin loaded from Wasm component");
        self.plugins.insert(plugin_id.clone(), sandbox);
        Ok(plugin_id)
    }

    /// Compiles multiple wasm plugins in parallel, then registers them sequentially.
    /// Returns a Vec with one Result per input entry — either the plugin ID or an error.
    pub fn load_plugins_from_wasm_parallel(
        &mut self,
        entries: Vec<(PathBuf, PermissionSet, ResourceLimits)>,
    ) -> Vec<Result<String, PluginHostError>> {
        let engine = self.engine();
        let entity_store = &self.entity_store;
        let event_store = &self.event_store;

        // Parallel compilation via scoped threads (Engine is Clone+Send+Sync)
        let sandboxes: Vec<Result<PluginSandbox, PluginHostError>> =
            std::thread::scope(|s| {
                let handles: Vec<_> = entries
                    .iter()
                    .map(|(path, perms, limits)| {
                        let engine = engine.clone();
                        let es = Arc::clone(entity_store);
                        let ev = Arc::clone(event_store);
                        let perms = perms.clone();
                        let limits = limits.clone();
                        s.spawn(move || {
                            PluginSandbox::from_wasm_cached(
                                path, &engine, perms, limits, es, ev,
                            )
                        })
                    })
                    .collect();

                handles
                    .into_iter()
                    .map(|h| h.join().unwrap_or_else(|_| {
                        Err(PluginHostError::Compilation(wasmtime::Error::msg(
                            "thread panicked during wasm compilation",
                        )))
                    }))
                    .collect()
            });

        // Sequential registration: policy check, duplicate check, insert
        sandboxes
            .into_iter()
            .map(|result| {
                let sandbox = result?;
                let plugin_id = sandbox.metadata.id.clone();

                if !self.policy_engine.is_plugin_allowed(&plugin_id, None) {
                    return Err(PluginHostError::PolicyDenied(format!(
                        "plugin '{}' blocked by policy",
                        plugin_id
                    )));
                }

                if self.plugins.contains_key(&plugin_id) {
                    return Err(PluginHostError::PluginAlreadyLoaded(plugin_id));
                }

                info!(plugin_id = %plugin_id, "Plugin loaded from Wasm component (parallel)");
                self.plugins.insert(plugin_id.clone(), sandbox);
                Ok(plugin_id)
            })
            .collect()
    }

    /// Unloads a plugin, calling dispose() if it's a Wasm component.
    pub fn unload_plugin(&mut self, plugin_id: &str) -> Result<(), PluginHostError> {
        match self.plugins.remove(plugin_id) {
            Some(mut sandbox) => {
                if sandbox.has_runtime() {
                    if let Err(e) = sandbox.call_dispose() {
                        warn!(plugin_id = %plugin_id, "dispose() failed during unload: {}", e);
                    }
                }
                info!(plugin_id = %plugin_id, "Plugin unloaded");
                Ok(())
            }
            None => Err(PluginHostError::PluginNotFound(plugin_id.to_string())),
        }
    }

    // ================================================================
    // Plugin access
    // ================================================================

    pub fn get_plugin(&self, plugin_id: &str) -> Result<&PluginSandbox, PluginHostError> {
        self.plugins
            .get(plugin_id)
            .ok_or_else(|| PluginHostError::PluginNotFound(plugin_id.to_string()))
    }

    pub fn get_plugin_mut(
        &mut self,
        plugin_id: &str,
    ) -> Result<&mut PluginSandbox, PluginHostError> {
        self.plugins
            .get_mut(plugin_id)
            .ok_or_else(|| PluginHostError::PluginNotFound(plugin_id.to_string()))
    }

    pub fn list_plugins(&self) -> Vec<&WitPluginMetadata> {
        self.plugins.values().map(|s| &s.metadata).collect()
    }

    /// Returns navigation items sorted by order.
    pub fn get_navigation_items(&self) -> Vec<WitNavigationItem> {
        let mut items: Vec<WitNavigationItem> = self
            .plugins
            .values()
            .map(|s| WitNavigationItem {
                id: s.metadata.id.clone(),
                display_name: s.metadata.name.clone(),
                subtitle: None,
                icon: s.metadata.icon.clone(),
                tooltip: None,
                order: s.metadata.navigation_order,
                show_badge: false,
                badge_count: 0,
                shortcut_hint: None,
            })
            .collect();
        items.sort_by_key(|i| i.order);
        items
    }

    // ================================================================
    // Plugin execution — command routing through Wasm exports
    // ================================================================

    /// Route an SDK message to a plugin (host-side path, for FFI backward compat).
    pub fn route_sdk_message(
        &self,
        plugin_id: &str,
        message: &WitSdkMessage,
    ) -> Result<WitSdkResponse, PluginHostError> {
        let sandbox = self.get_plugin(plugin_id)?;
        Ok(sandbox.handle_sdk_send(message))
    }

    /// Send a command to a plugin by calling its handle_command() export.
    pub fn send_command(
        &mut self,
        plugin_id: &str,
        command_name: &str,
        args: &str,
    ) -> Result<String, PluginHostError> {
        let sandbox = self.get_plugin_mut(plugin_id)?;
        sandbox.call_handle_command(command_name, args)
    }

    /// Get the view state JSON from a plugin.
    pub fn get_view_state(&mut self, plugin_id: &str) -> Result<String, PluginHostError> {
        let sandbox = self.get_plugin_mut(plugin_id)?;
        sandbox.call_get_view_state()
    }

    /// Get the raw view data JSON from a plugin (for host-side template evaluation).
    pub fn get_view_data(&mut self, plugin_id: &str) -> Result<String, PluginHostError> {
        let sandbox = self.get_plugin_mut(plugin_id)?;
        sandbox.call_get_view_data()
    }

    /// Initialize a loaded plugin.
    pub fn initialize_plugin(&mut self, plugin_id: &str) -> Result<bool, PluginHostError> {
        let sandbox = self.get_plugin_mut(plugin_id)?;
        sandbox.call_initialize()
    }

    /// Activate a loaded plugin.
    pub fn activate_plugin(&mut self, plugin_id: &str) -> Result<(), PluginHostError> {
        let sandbox = self.get_plugin_mut(plugin_id)?;
        sandbox.call_activate()
    }

    /// Notify a plugin it was navigated to.
    pub fn notify_navigated_to(&mut self, plugin_id: &str) -> Result<(), PluginHostError> {
        let sandbox = self.get_plugin_mut(plugin_id)?;
        sandbox.call_on_navigated_to()
    }

    /// Notify a plugin it was navigated away from.
    pub fn notify_navigated_from(&mut self, plugin_id: &str) -> Result<(), PluginHostError> {
        let sandbox = self.get_plugin_mut(plugin_id)?;
        sandbox.call_on_navigated_from()
    }

    /// Fetch a URL on behalf of a plugin, checking its Network permission first.
    /// Returns the response body bytes on success.
    pub fn fetch_url_for_plugin(
        &self,
        plugin_id: &str,
        url: &str,
    ) -> Result<Vec<u8>, PluginHostError> {
        use crate::permissions::Permission;

        let sandbox = self.get_plugin(plugin_id)?;
        sandbox.state().check_permission(Permission::Network)?;

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("PrivStack/1.0")
            .build()
            .map_err(|e| PluginHostError::NetworkError(format!("http client: {e}")))?;

        let resp = client
            .get(url)
            .header("Accept", "image/*,*/*;q=0.8")
            .send()
            .map_err(|e| PluginHostError::NetworkError(format!("fetch failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(PluginHostError::NetworkError(format!(
                "HTTP {} fetching {url}",
                resp.status()
            )));
        }

        resp.bytes()
            .map(|b| b.to_vec())
            .map_err(|e| PluginHostError::NetworkError(format!("read body: {e}")))
    }

    // ================================================================
    // Cross-plugin queries
    // ================================================================

    /// Search all plugins for linkable items matching a query.
    pub fn query_all_linkable_items(
        &mut self,
        query: &str,
        max_results: u32,
    ) -> Vec<WitLinkableItem> {
        let plugin_ids: Vec<String> = self
            .plugins
            .iter()
            .filter(|(_, s)| s.has_linkable_item_provider)
            .map(|(id, _)| id.clone())
            .collect();

        let mut all_items = Vec::new();
        for plugin_id in plugin_ids {
            if let Some(sandbox) = self.plugins.get_mut(&plugin_id) {
                match sandbox.call_search_linkable_items(query, max_results) {
                    Ok(items) => {
                        for mut item in items {
                            item.plugin_id = Some(plugin_id.clone());
                            all_items.push(item);
                        }
                    }
                    Err(e) => {
                        warn!(plugin_id = %plugin_id, "Linkable item search failed: {}", e);
                    }
                }
            }
        }
        all_items
    }

    /// Navigate to a specific item within a plugin via its deep-link-target export.
    pub fn navigate_to_item(
        &mut self,
        plugin_id: &str,
        item_id: &str,
    ) -> Result<(), PluginHostError> {
        let sandbox = self.get_plugin_mut(plugin_id)?;
        if !sandbox.has_deep_link_target {
            return Err(PluginHostError::CapabilityNotSupported {
                plugin_id: plugin_id.to_string(),
                capability: "deep-link-target".to_string(),
            });
        }
        sandbox.call_navigate_to_item(item_id)
    }

    /// Navigate to a specific item and return its view data in one call.
    /// Used for hover prefetch - navigates to the entity and returns the view data
    /// without requiring a separate get_view_data call.
    ///
    /// This is safe to call for cross-plugin prefetch (prefetching an entity in a
    /// plugin that isn't currently displayed).
    pub fn get_entity_view_data(
        &mut self,
        plugin_id: &str,
        item_id: &str,
    ) -> Result<String, PluginHostError> {
        let sandbox = self.get_plugin_mut(plugin_id)?;
        if !sandbox.has_deep_link_target {
            return Err(PluginHostError::CapabilityNotSupported {
                plugin_id: plugin_id.to_string(),
                capability: "deep-link-target".to_string(),
            });
        }
        // Navigate to the entity
        sandbox.call_navigate_to_item(item_id)?;
        // Get and return the view data for that entity
        sandbox.call_get_view_data()
    }

    /// Get metadata about all link providers across plugins.
    pub fn get_all_link_providers(&self) -> Vec<WitLinkProviderInfo> {
        let mut providers = Vec::new();
        for (_, sandbox) in &self.plugins {
            if !sandbox.has_linkable_item_provider {
                continue;
            }
            let link_type = sandbox
                .cached_link_type
                .clone()
                .unwrap_or_else(|| sandbox.metadata.id.clone());
            providers.push(WitLinkProviderInfo {
                plugin_id: sandbox.metadata.id.clone(),
                link_type,
                display_name: sandbox.metadata.name.clone(),
                icon: sandbox.metadata.icon.clone(),
            });
        }
        providers
    }

    /// Get commands from a specific plugin.
    pub fn get_commands(
        &self,
        plugin_id: &str,
    ) -> Result<Vec<WitCommandDefinition>, PluginHostError> {
        let _sandbox = self.get_plugin(plugin_id)?;
        Ok(Vec::new())
    }

    /// Get all commands from all plugins.
    pub fn get_all_commands(&self) -> Vec<(String, Vec<WitCommandDefinition>)> {
        self.plugins
            .iter()
            .map(|(id, _)| (id.clone(), Vec::new()))
            .collect()
    }

    /// Updates permissions for a loaded plugin at runtime.
    pub fn update_plugin_permissions(
        &mut self,
        plugin_id: &str,
        permissions: PermissionSet,
    ) -> Result<(), PluginHostError> {
        let sandbox = self.get_plugin_mut(plugin_id)?;
        sandbox.update_permissions(permissions);
        info!(plugin_id = %plugin_id, "Plugin permissions updated at runtime");
        Ok(())
    }

    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_loaded(&self, plugin_id: &str) -> bool {
        self.plugins.contains_key(plugin_id)
    }

    pub fn policy_engine(&self) -> &PolicyEngine {
        &self.policy_engine
    }

    // ================================================================
    // Resource Metrics
    // ================================================================

    /// Get resource metrics for a specific plugin.
    pub fn get_plugin_metrics(
        &self,
        plugin_id: &str,
    ) -> Result<crate::sandbox::PluginResourceMetrics, PluginHostError> {
        let sandbox = self.get_plugin(plugin_id)?;
        Ok(sandbox.get_resource_metrics())
    }

    /// Get resource metrics for all loaded plugins.
    /// Returns a Vec of (plugin_id, metrics) tuples.
    pub fn get_all_plugin_metrics(&self) -> Vec<(String, crate::sandbox::PluginResourceMetrics)> {
        self.plugins
            .iter()
            .map(|(id, sandbox)| (id.clone(), sandbox.get_resource_metrics()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::PermissionSet;
    use crate::policy::{PolicyConfig, PolicyMode};

    fn test_stores() -> (
        Arc<privstack_storage::EntityStore>,
        Arc<privstack_storage::EventStore>,
    ) {
        let es = privstack_storage::EntityStore::open_in_memory().unwrap();
        let ev = privstack_storage::EventStore::open_in_memory().unwrap();
        (Arc::new(es), Arc::new(ev))
    }

    fn test_metadata(id: &str) -> WitPluginMetadata {
        WitPluginMetadata {
            id: id.into(),
            name: format!("Test {}", id),
            description: "test".into(),
            version: "0.1.0".into(),
            author: "test".into(),
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
    fn load_and_list_plugins() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);

        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        mgr.load_plugin(
            test_metadata("p2"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        assert_eq!(mgr.plugin_count(), 2);
        assert!(mgr.is_loaded("p1"));
        assert!(mgr.is_loaded("p2"));
    }

    #[test]
    fn duplicate_load_rejected() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);

        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        let result = mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn unload_plugin() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);

        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        assert!(mgr.is_loaded("p1"));
        mgr.unload_plugin("p1").unwrap();
        assert!(!mgr.is_loaded("p1"));
    }

    #[test]
    fn sdk_message_routing_enforces_entity_type() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);

        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        // Allowed entity type
        let msg = WitSdkMessage {
            action: WitSdkAction::List,
            entity_type: "test_item".into(),
            entity_id: None,
            payload: None,
            parameters: vec![],
            source: None,
        };
        let resp = mgr.route_sdk_message("p1", &msg).unwrap();
        assert!(resp.success);

        // Disallowed entity type
        let bad_msg = WitSdkMessage {
            entity_type: "other_type".into(),
            ..msg
        };
        let resp = mgr.route_sdk_message("p1", &bad_msg).unwrap();
        assert!(!resp.success);
    }

    #[test]
    fn navigation_items_sorted_by_order() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);

        let mut m1 = test_metadata("p1");
        m1.navigation_order = 200;
        let mut m2 = test_metadata("p2");
        m2.navigation_order = 100;

        mgr.load_plugin(
            m1,
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();
        mgr.load_plugin(
            m2,
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        let items = mgr.get_navigation_items();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "p2");
        assert_eq!(items[1].id, "p1");
    }

    #[test]
    fn policy_blocks_disallowed_plugin() {
        let (es, ev) = test_stores();
        let policy = PolicyEngine::with_config(PolicyConfig {
            mode: PolicyMode::Allowlist,
            allowed_plugin_ids: vec!["allowed.plugin".into()],
            ..PolicyConfig::default()
        });

        let mut mgr = PluginHostManager::with_policy(es, ev, policy);

        let result = mgr.load_plugin(
            test_metadata("blocked.plugin"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        );
        assert!(matches!(result, Err(PluginHostError::PolicyDenied(_))));
    }

    // ================================================================
    // send_command on metadata-only plugin (no Wasm runtime)
    // ================================================================

    #[test]
    fn send_command_metadata_only_fails() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        let result = mgr.send_command("p1", "do_thing", "{}");
        assert!(result.is_err());
    }

    #[test]
    fn send_command_plugin_not_found() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);

        let result = mgr.send_command("nonexistent", "cmd", "{}");
        assert!(matches!(result, Err(PluginHostError::PluginNotFound(_))));
    }

    // ================================================================
    // get_view_state / get_view_data on metadata-only
    // ================================================================

    #[test]
    fn get_view_state_metadata_only_fails() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        assert!(mgr.get_view_state("p1").is_err());
    }

    #[test]
    fn get_view_data_metadata_only_fails() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        assert!(mgr.get_view_data("p1").is_err());
    }

    #[test]
    fn get_view_state_plugin_not_found() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        assert!(matches!(
            mgr.get_view_state("missing"),
            Err(PluginHostError::PluginNotFound(_))
        ));
    }

    // ================================================================
    // initialize_plugin / activate_plugin on metadata-only
    // ================================================================

    #[test]
    fn initialize_plugin_metadata_only_fails() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        assert!(mgr.initialize_plugin("p1").is_err());
    }

    #[test]
    fn activate_plugin_metadata_only_fails() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        assert!(mgr.activate_plugin("p1").is_err());
    }

    #[test]
    fn initialize_plugin_not_found() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        assert!(matches!(
            mgr.initialize_plugin("missing"),
            Err(PluginHostError::PluginNotFound(_))
        ));
    }

    // ================================================================
    // notify_navigated_to / notify_navigated_from
    // ================================================================

    #[test]
    fn notify_navigated_to_metadata_only_fails() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        assert!(mgr.notify_navigated_to("p1").is_err());
    }

    #[test]
    fn notify_navigated_from_metadata_only_fails() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        assert!(mgr.notify_navigated_from("p1").is_err());
    }

    #[test]
    fn notify_navigated_to_not_found() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        assert!(matches!(
            mgr.notify_navigated_to("missing"),
            Err(PluginHostError::PluginNotFound(_))
        ));
    }

    // ================================================================
    // query_all_linkable_items — metadata-only plugins have no provider
    // ================================================================

    #[test]
    fn query_all_linkable_items_no_providers() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        // Metadata-only sandboxes have has_linkable_item_provider = false
        let items = mgr.query_all_linkable_items("test", 10);
        assert!(items.is_empty());
    }

    // ================================================================
    // get_all_link_providers
    // ================================================================

    #[test]
    fn get_all_link_providers_empty_for_metadata_only() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        let providers = mgr.get_all_link_providers();
        assert!(providers.is_empty());
    }

    // ================================================================
    // get_commands / get_all_commands
    // ================================================================

    #[test]
    fn get_commands_returns_empty_vec() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        let cmds = mgr.get_commands("p1").unwrap();
        assert!(cmds.is_empty());
    }

    #[test]
    fn get_commands_not_found() {
        let (es, ev) = test_stores();
        let mgr = PluginHostManager::new_for_test(es, ev);
        assert!(matches!(
            mgr.get_commands("missing"),
            Err(PluginHostError::PluginNotFound(_))
        ));
    }

    #[test]
    fn get_all_commands_returns_entries_for_each_plugin() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();
        mgr.load_plugin(
            test_metadata("p2"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        let all_cmds = mgr.get_all_commands();
        assert_eq!(all_cmds.len(), 2);
    }

    // ================================================================
    // update_plugin_permissions
    // ================================================================

    #[test]
    fn update_plugin_permissions_success() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        // Initially no vault access
        assert!(mgr.get_plugin("p1").unwrap().check_vault_access().is_err());

        mgr.update_plugin_permissions("p1", PermissionSet::all_granted())
            .unwrap();

        assert!(mgr.get_plugin("p1").unwrap().check_vault_access().is_ok());
    }

    #[test]
    fn update_plugin_permissions_not_found() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        assert!(matches!(
            mgr.update_plugin_permissions("missing", PermissionSet::all_granted()),
            Err(PluginHostError::PluginNotFound(_))
        ));
    }

    // ================================================================
    // fetch_url_for_plugin permission check
    // ================================================================

    #[test]
    fn fetch_url_permission_denied_without_network() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(), // No Network permission
            ResourceLimits::first_party(),
        )
        .unwrap();

        let result = mgr.fetch_url_for_plugin("p1", "https://example.com");
        assert!(matches!(result, Err(PluginHostError::PermissionDenied { .. })));
    }

    #[test]
    fn fetch_url_plugin_not_found() {
        let (es, ev) = test_stores();
        let mgr = PluginHostManager::new_for_test(es, ev);
        assert!(matches!(
            mgr.fetch_url_for_plugin("missing", "https://example.com"),
            Err(PluginHostError::PluginNotFound(_))
        ));
    }

    // ================================================================
    // unload nonexistent plugin
    // ================================================================

    #[test]
    fn unload_nonexistent_plugin() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        assert!(matches!(
            mgr.unload_plugin("missing"),
            Err(PluginHostError::PluginNotFound(_))
        ));
    }

    // ================================================================
    // list_plugins
    // ================================================================

    #[test]
    fn list_plugins_returns_all_metadata() {
        let (es, ev) = test_stores();
        let mut mgr = PluginHostManager::new_for_test(es, ev);
        mgr.load_plugin(
            test_metadata("p1"),
            test_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

        let plugins = mgr.list_plugins();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].id, "p1");
    }

    // ================================================================
    // route_sdk_message plugin not found
    // ================================================================

    #[test]
    fn route_sdk_message_plugin_not_found() {
        let (es, ev) = test_stores();
        let mgr = PluginHostManager::new_for_test(es, ev);
        let msg = WitSdkMessage {
            action: WitSdkAction::List,
            entity_type: "test".into(),
            entity_id: None,
            payload: None,
            parameters: vec![],
            source: None,
        };
        assert!(matches!(
            mgr.route_sdk_message("missing", &msg),
            Err(PluginHostError::PluginNotFound(_))
        ));
    }

    // ================================================================
    // policy_engine accessor
    // ================================================================

    #[test]
    fn policy_engine_accessor() {
        let (es, ev) = test_stores();
        let mgr = PluginHostManager::new_for_test(es, ev);
        // Default policy is unrestricted
        assert!(mgr.policy_engine().is_plugin_allowed("anything", None));
    }

    // ================================================================
    // plugin_count and is_loaded
    // ================================================================

    #[test]
    fn plugin_count_starts_at_zero() {
        let (es, ev) = test_stores();
        let mgr = PluginHostManager::new_for_test(es, ev);
        assert_eq!(mgr.plugin_count(), 0);
        assert!(!mgr.is_loaded("anything"));
    }
}
