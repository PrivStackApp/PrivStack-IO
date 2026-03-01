//! C ABI exports for PrivStack cross-platform integration.
//!
//! This crate exposes the Rust core as a C-compatible library for:
//! - .NET/Avalonia desktop app (P/Invoke)
//! - Android (JNI)
//! - iOS (Swift C interop)
//!
//! All functions use C-compatible types and handle errors via return codes.
//! Zero domain logic — plugins consume generic vault, blob, and entity APIs.

use std::sync::atomic::{AtomicU8, Ordering};

/// FFI log level constants (lower = more severe).
pub(crate) const FFI_LEVEL_ERROR: u8 = 0;
pub(crate) const FFI_LEVEL_WARN: u8 = 1;
pub(crate) const FFI_LEVEL_INFO: u8 = 2;
pub(crate) const FFI_LEVEL_DEBUG: u8 = 3;

/// Global FFI log level. Default = INFO (2).
/// Set from `PRIVSTACK_LOG_LEVEL` env var in `privstack_init()`.
pub(crate) static FFI_LOG_LEVEL: AtomicU8 = AtomicU8::new(FFI_LEVEL_INFO);

macro_rules! ffi_error {
    ($($arg:tt)*) => {
        if $crate::FFI_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed) >= $crate::FFI_LEVEL_ERROR {
            eprintln!($($arg)*);
        }
    };
}

macro_rules! ffi_warn {
    ($($arg:tt)*) => {
        if $crate::FFI_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed) >= $crate::FFI_LEVEL_WARN {
            eprintln!($($arg)*);
        }
    };
}

macro_rules! ffi_info {
    ($($arg:tt)*) => {
        if $crate::FFI_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed) >= $crate::FFI_LEVEL_INFO {
            eprintln!($($arg)*);
        }
    };
}

macro_rules! ffi_debug {
    ($($arg:tt)*) => {
        if $crate::FFI_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed) >= $crate::FFI_LEVEL_DEBUG {
            eprintln!($($arg)*);
        }
    };
}

mod cloud;
mod datasets;
mod rag;
#[cfg(target_os = "android")]
mod android_jni;

use privstack_blobstore::BlobStore;
use privstack_license::{
    Activation, ActivationStore, DeviceFingerprint, DeviceInfo, LicenseError, LicenseKey,
    LicensePlan, LicenseStatus,
};
use privstack_model::{Entity, EntitySchema, PluginDomainHandler};
#[cfg(feature = "wasm-plugins")]
use privstack_plugin_host::PluginHostManager;
use privstack_storage::{EntityStore, EventStore};
use privstack_sync::{
    cloud::{CloudStorage, GoogleDriveConfig, GoogleDriveStorage, ICloudConfig, ICloudStorage},
    create_personal_orchestrator,
    pairing::{PairingManager, SyncCode},
    Keypair, OrchestratorConfig, OrchestratorHandle, P2pConfig, P2pTransport,
    PersonalSyncPolicy, SyncCommand, SyncConfig, SyncEngine, SyncEvent, SyncTransport,
};
use privstack_types::{EntityId, Event, PeerId};
use privstack_vault::VaultManager;
use serde::{Deserialize, Serialize};
use std::ffi::{c_char, c_int, CStr, CString};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, Mutex as TokioMutex};
use uuid::Uuid;

/// Error codes returned by FFI functions.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivStackError {
    /// Operation succeeded.
    Ok = 0,
    /// Null pointer argument.
    NullPointer = 1,
    /// Invalid UTF-8 string.
    InvalidUtf8 = 2,
    /// JSON serialization error.
    JsonError = 3,
    /// Storage error.
    StorageError = 4,
    /// Document not found.
    NotFound = 5,
    /// Handle not initialized.
    NotInitialized = 6,
    /// Sync not running.
    SyncNotRunning = 7,
    /// Sync already running.
    SyncAlreadyRunning = 8,
    /// Network/sync error.
    SyncError = 9,
    /// Peer not found.
    PeerNotFound = 10,
    /// Authentication error.
    AuthError = 11,
    /// Cloud storage error.
    CloudError = 12,
    /// License key format invalid.
    LicenseInvalidFormat = 13,
    /// License signature failed.
    LicenseInvalidSignature = 14,
    /// License expired.
    LicenseExpired = 15,
    /// License not activated.
    LicenseNotActivated = 16,
    /// License activation failed.
    LicenseActivationFailed = 17,
    /// Invalid sync code format.
    InvalidSyncCode = 18,
    /// Peer not trusted.
    PeerNotTrusted = 19,
    /// Pairing error.
    PairingError = 20,
    /// Vault is locked.
    VaultLocked = 21,
    /// Vault not found.
    VaultNotFound = 22,
    /// Plugin error (load, unload, or route failure).
    PluginError = 23,
    /// Plugin not found.
    PluginNotFound = 24,
    /// Plugin permission denied.
    PluginPermissionDenied = 25,
    /// Vault already initialized.
    VaultAlreadyInitialized = 26,
    /// Password too short.
    PasswordTooShort = 27,
    /// Invalid argument.
    InvalidArgument = 28,
    /// Cloud sync error (S3 transport, outbox, or orchestration failure).
    CloudSyncError = 29,
    /// Cloud storage quota exceeded.
    QuotaExceeded = 30,
    /// Share permission denied.
    ShareDenied = 31,
    /// Envelope encryption/decryption error.
    EnvelopeError = 32,
    /// Cloud API authentication error.
    CloudAuthError = 33,
    /// Recovery not configured for this vault.
    RecoveryNotConfigured = 34,
    /// Invalid recovery mnemonic.
    InvalidRecoveryMnemonic = 35,
    /// Rate limited by the cloud API — client should back off.
    RateLimited = 36,
    /// Unknown error.
    Unknown = 99,
}

/// Registry of entity schemas and optional domain handlers.
pub struct EntityRegistry {
    schemas: std::collections::HashMap<String, EntitySchema>,
    handlers: std::collections::HashMap<String, Box<dyn PluginDomainHandler>>,
}

impl EntityRegistry {
    pub fn new() -> Self {
        Self {
            schemas: std::collections::HashMap::new(),
            handlers: std::collections::HashMap::new(),
        }
    }

    pub fn register_schema(&mut self, schema: EntitySchema) {
        self.schemas.insert(schema.entity_type.clone(), schema);
    }

    #[allow(dead_code)]
    pub fn register_handler(&mut self, entity_type: String, handler: Box<dyn PluginDomainHandler>) {
        self.handlers.insert(entity_type, handler);
    }

    pub fn get_schema(&self, entity_type: &str) -> Option<&EntitySchema> {
        self.schemas.get(entity_type)
    }

    pub fn get_handler(&self, entity_type: &str) -> Option<&dyn PluginDomainHandler> {
        self.handlers.get(entity_type).map(|h| h.as_ref())
    }

    pub fn has_schema(&self, entity_type: &str) -> bool {
        self.schemas.contains_key(entity_type)
    }

    /// Clone all registered schemas for use in background tasks (e.g., cloud sync inbound).
    fn clone_schemas(&self) -> std::collections::HashMap<String, EntitySchema> {
        self.schemas.clone()
    }
}

/// Opaque handle to the PrivStack runtime.
pub struct PrivStackHandle {
    /// The database path passed to privstack_init (needed for deriving keypair file path).
    db_path: String,
    /// Shared database connection.  All stores (entity, event, blob, vault)
    /// hold an `Arc::clone` of this same `Arc<Mutex<Connection>>`.  When
    /// transitioning from an in-memory placeholder to an encrypted on-disk
    /// database, we swap the `Connection` *inside* the Mutex so every store
    /// automatically uses the new connection on its next operation.
    main_conn: Arc<std::sync::Mutex<privstack_db::rusqlite::Connection>>,
    entity_store: Arc<EntityStore>,
    #[allow(dead_code)]
    event_store: Arc<EventStore>,
    entity_registry: EntityRegistry,
    peer_id: PeerId,
    runtime: Runtime,
    #[allow(dead_code)]
    sync_engine: SyncEngine,
    p2p_transport: Option<Arc<TokioMutex<P2pTransport>>>,
    orchestrator_handle: Option<OrchestratorHandle>,
    sync_event_rx: Option<mpsc::Receiver<SyncEvent>>,
    pairing_manager: Arc<std::sync::Mutex<PairingManager>>,
    personal_policy: Option<Arc<PersonalSyncPolicy>>,
    device_name: String,
    google_drive: Option<GoogleDriveStorage>,
    icloud: Option<ICloudStorage>,
    pub activation_store: ActivationStore,
    // Generic capabilities — no domain logic
    vault_manager: Arc<VaultManager>,
    blob_store: BlobStore,
    // Tabular datasets (unencrypted SQLite for SQL queries)
    dataset_store: Option<privstack_datasets::DatasetStore>,
    // Wasm plugin host manager
    #[cfg(feature = "wasm-plugins")]
    plugin_host: PluginHostManager,
    // Cloud sync (S3-backed multi-device sync + sharing)
    cloud_api: Option<Arc<privstack_cloud::api_client::CloudApiClient>>,
    cloud_sync_handle: Option<privstack_cloud::sync_engine::CloudSyncHandle>,
    cloud_event_tx: Option<mpsc::Sender<Event>>,
    cloud_envelope_mgr: Option<Arc<TokioMutex<privstack_cloud::envelope::EnvelopeManager>>>,
    cloud_share_mgr: Option<Arc<privstack_cloud::sharing::ShareManager>>,
    cloud_config: Option<privstack_cloud::CloudConfig>,
    cloud_blob_mgr: Option<Arc<privstack_cloud::blob_sync::BlobSyncManager>>,
    cloud_dek_registry: Option<privstack_cloud::dek_registry::DekRegistry>,
    cloud_user_id: Option<i64>,
    cloud_active_workspace: Option<String>,
}

/// Salt file extension stored alongside the database.  If this file exists,
/// the database is SQLCipher-encrypted and requires a password to open.
const SALT_FILE_EXT: &str = "privstack.salt";

impl PrivStackHandle {
    /// Returns the path to the salt file for this database.
    fn salt_path(&self) -> std::path::PathBuf {
        Path::new(&self.db_path).with_extension(SALT_FILE_EXT)
    }

    /// Returns the path to the main database file.
    fn main_db_path(&self) -> std::path::PathBuf {
        Path::new(&self.db_path).with_extension("privstack.db")
    }

    /// Checks whether a salt file exists, indicating the database is encrypted.
    fn has_salt_file(&self) -> bool {
        self.db_path != ":memory:" && self.salt_path().exists()
    }

    /// Swap the placeholder in-memory connection for a real encrypted (or
    /// unencrypted) on-disk connection, then re-initialize all store schemas.
    ///
    /// This is the core of the two-phase init: `init_core()` opens an
    /// in-memory placeholder so the handle exists immediately, and this
    /// method replaces it with the real database once the password is
    /// available.
    fn swap_connection(&self, new_conn: privstack_db::rusqlite::Connection) -> Result<(), PrivStackError> {
        // Register custom SQL functions on the new connection.
        privstack_db::register_custom_functions(&new_conn).map_err(|e| {
            ffi_error!("[FFI] Failed to register custom functions after swap: {e:?}");
            PrivStackError::StorageError
        })?;

        // Swap the Connection inside the shared Mutex.
        {
            let mut guard = self.main_conn.lock().unwrap();
            *guard = new_conn;
        }

        // Re-initialize schemas on the new connection.
        self.entity_store.reinitialize_schema().map_err(|e| {
            ffi_error!("[FFI] Failed to reinitialize entity schema: {e:?}");
            PrivStackError::StorageError
        })?;
        self.event_store.reinitialize_schema().map_err(|e| {
            ffi_error!("[FFI] Failed to reinitialize event schema: {e:?}");
            PrivStackError::StorageError
        })?;
        self.blob_store.reinitialize_schema().map_err(|e| {
            ffi_error!("[FFI] Failed to reinitialize blob schema: {e:?}");
            PrivStackError::StorageError
        })?;

        // Clear cached vault instances so they re-run ensure_tables() on
        // the new connection the next time they are accessed.
        self.vault_manager.reinitialize_vaults();

        Ok(())
    }
}

/// Discovered peer info for JSON serialization.
#[derive(Serialize)]
pub struct DiscoveredPeerInfo {
    pub peer_id: String,
    pub device_name: Option<String>,
    pub discovery_method: String,
    pub addresses: Vec<String>,
}

/// Sync status for JSON serialization.
#[derive(Serialize)]
pub struct SyncStatus {
    pub running: bool,
    pub local_peer_id: String,
    pub discovered_peers: Vec<DiscoveredPeerInfo>,
}

/// Sync event DTO for JSON serialization.
#[derive(Serialize)]
pub struct SyncEventDto {
    pub event_type: String,
    pub peer_id: Option<String>,
    pub device_name: Option<String>,
    pub entity_id: Option<String>,
    pub events_sent: Option<usize>,
    pub events_received: Option<usize>,
    pub error: Option<String>,
    pub entity_type: Option<String>,
    pub json_data: Option<String>,
}

impl From<SyncEvent> for SyncEventDto {
    fn from(event: SyncEvent) -> Self {
        match event {
            SyncEvent::PeerDiscovered {
                peer_id,
                device_name,
            } => SyncEventDto {
                event_type: "peer_discovered".to_string(),
                peer_id: Some(peer_id.to_string()),
                device_name,
                entity_id: None,
                events_sent: None,
                events_received: None,
                error: None,
                entity_type: None,
                json_data: None,
            },
            SyncEvent::SyncStarted { peer_id } => SyncEventDto {
                event_type: "sync_started".to_string(),
                peer_id: Some(peer_id.to_string()),
                device_name: None,
                entity_id: None,
                events_sent: None,
                events_received: None,
                error: None,
                entity_type: None,
                json_data: None,
            },
            SyncEvent::SyncCompleted {
                peer_id,
                events_sent,
                events_received,
            } => SyncEventDto {
                event_type: "sync_completed".to_string(),
                peer_id: Some(peer_id.to_string()),
                device_name: None,
                entity_id: None,
                events_sent: Some(events_sent),
                events_received: Some(events_received),
                error: None,
                entity_type: None,
                json_data: None,
            },
            SyncEvent::SyncFailed { peer_id, error } => SyncEventDto {
                event_type: "sync_failed".to_string(),
                peer_id: Some(peer_id.to_string()),
                device_name: None,
                entity_id: None,
                events_sent: None,
                events_received: None,
                error: Some(error),
                entity_type: None,
                json_data: None,
            },
            SyncEvent::EntityUpdated { entity_id } => SyncEventDto {
                event_type: "entity_updated".to_string(),
                peer_id: None,
                device_name: None,
                entity_id: Some(entity_id.to_string()),
                events_sent: None,
                events_received: None,
                error: None,
                entity_type: None,
                json_data: None,
            },
        }
    }
}

/// Cloud file info for JSON serialization.
#[derive(Serialize)]
pub struct CloudFileInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified_at_ms: i64,
    pub content_hash: Option<String>,
}

/// Cloud provider type.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudProvider {
    GoogleDrive = 0,
    ICloud = 1,
}

/// Global handle storage (single instance for now).
pub static HANDLE: Mutex<Option<PrivStackHandle>> = Mutex::new(None);

/// Acquire the HANDLE lock, recovering from poison if a prior
/// `catch_unwind` caught a SQLite panic while the lock was held.
pub(crate) fn lock_handle() -> std::sync::MutexGuard<'static, Option<PrivStackHandle>> {
    HANDLE.lock().unwrap_or_else(|poisoned| {
        ffi_warn!("[FFI] recovering from poisoned HANDLE mutex");
        poisoned.into_inner()
    })
}

// ============================================================================
// Core Functions
// ============================================================================

/// Initializes the PrivStack runtime.
///
/// # Safety
/// - `db_path` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_init(db_path: *const c_char) -> PrivStackError { unsafe {
    // Read PRIVSTACK_LOG_LEVEL env var and configure both FFI macros and tracing.
    let (ffi_level, tracing_filter) = match std::env::var("PRIVSTACK_LOG_LEVEL")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "error" | "err" => (FFI_LEVEL_ERROR, "error"),
        "warn" | "warning" => (FFI_LEVEL_WARN, "warn"),
        "debug" | "dbg" => (FFI_LEVEL_DEBUG, "debug"),
        "trace" => (FFI_LEVEL_DEBUG, "trace"),
        _ => (FFI_LEVEL_INFO, "info"), // default
    };
    FFI_LOG_LEVEL.store(ffi_level, Ordering::Relaxed);

    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(tracing_filter)),
        )
        .with_writer(std::io::stderr)
        .try_init();

    if db_path.is_null() {
        return PrivStackError::NullPointer;
    }

    let path = match CStr::from_ptr(db_path).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    #[cfg(feature = "wasm-plugins")]
    { init_with_plugin_host_builder(path, |es, ev| PluginHostManager::new(es, ev)) }
    #[cfg(not(feature = "wasm-plugins"))]
    { init_core(path) }
}}

/// Loads an existing peer ID from disk, or generates and saves a new one.
/// The peer ID file lives alongside the database (e.g. `data.peer_id`).
/// For `:memory:` databases, a fresh ID is generated every time.
fn load_or_create_peer_id(db_path: &str) -> PeerId {
    if db_path == ":memory:" {
        return PeerId::new();
    }

    let peer_id_path = Path::new(db_path).with_extension("peer_id");

    // Try to read existing peer ID
    if let Ok(contents) = std::fs::read_to_string(&peer_id_path) {
        let trimmed = contents.trim();
        if let Ok(uuid) = Uuid::parse_str(trimmed) {
            ffi_debug!("[FFI] Loaded existing peer ID: {uuid}");
            return PeerId::from_uuid(uuid);
        }
        ffi_warn!("[FFI] Corrupt peer_id file at {}, generating new one", peer_id_path.display());
    }

    // Generate new peer ID and persist it
    let peer_id = PeerId::new();
    if let Err(e) = std::fs::write(&peer_id_path, peer_id.to_string()) {
        ffi_warn!("[FFI] Failed to persist peer ID to {}: {e}", peer_id_path.display());
    } else {
        ffi_debug!("[FFI] Generated and saved new peer ID: {peer_id}");
    }
    peer_id
}

/// Loads an existing libp2p keypair from disk, or generates and saves a new one.
/// The keypair file lives alongside the database (e.g. `data.keypair`).
/// For `:memory:` databases, a fresh keypair is generated every time.
fn load_or_create_keypair(db_path: &str) -> Keypair {
    if db_path == ":memory:" {
        return Keypair::generate_ed25519();
    }

    let keypair_path = Path::new(db_path).with_extension("keypair");

    // Try to read existing keypair
    if let Ok(bytes) = std::fs::read(&keypair_path) {
        if let Ok(kp) = Keypair::from_protobuf_encoding(&bytes) {
            ffi_debug!("[FFI] Loaded existing libp2p keypair from {}", keypair_path.display());
            return kp;
        }
        ffi_warn!("[FFI] Corrupt keypair file at {}, generating new one", keypair_path.display());
    }

    // Generate new keypair and persist it
    let keypair = Keypair::generate_ed25519();
    match keypair.to_protobuf_encoding() {
        Ok(bytes) => {
            if let Err(e) = std::fs::write(&keypair_path, &bytes) {
                ffi_warn!("[FFI] Failed to persist keypair to {}: {e}", keypair_path.display());
            } else {
                ffi_debug!("[FFI] Generated and saved new libp2p keypair to {}", keypair_path.display());
            }
        }
        Err(e) => {
            ffi_warn!("[FFI] Failed to encode keypair: {e}");
        }
    }
    keypair
}

/// Core init logic — sets up vault, blob, entity, event stores, runtime, sync engine.
/// Used directly when wasm-plugins feature is disabled.
///
/// **Two-phase initialization for SQLCipher:**
/// - If no salt file exists (first run or legacy unencrypted): opens the
///   database unencrypted so the app can operate immediately.  Encryption is
///   enabled later when the user calls `privstack_auth_initialize`.
/// - If a salt file exists (encrypted database): opens an **in-memory
///   placeholder** so the handle exists for non-DB operations.  The real
///   encrypted database is opened later by `privstack_auth_unlock`.
#[cfg(not(feature = "wasm-plugins"))]
pub fn init_core(path: &str) -> PrivStackError {
    let peer_id = load_or_create_peer_id(path);

    // Detect whether the database is encrypted (salt file present).
    let salt_path = Path::new(path).with_extension(SALT_FILE_EXT);
    let is_encrypted = path != ":memory:" && salt_path.exists();

    // Open the initial connection:
    //   :memory: path   → in-memory (testing)
    //   encrypted       → in-memory placeholder (real DB opened on auth)
    //   unencrypted     → open on-disk unencrypted (legacy / first run)
    let main_db_path = if path == ":memory:" {
        Path::new(":memory:").to_path_buf()
    } else {
        Path::new(path).with_extension("privstack.db")
    };

    let open_result = if path == ":memory:" || is_encrypted {
        if is_encrypted {
            ffi_info!(
                "[FFI] Salt file found — opening in-memory placeholder until auth_unlock"
            );
        }
        privstack_db::open_in_memory()
    } else {
        ffi_debug!("[FFI] Opening main database (unencrypted): {}", main_db_path.display());
        privstack_db::open_db_unencrypted(&main_db_path)
    };

    let main_conn = match open_result {
        Ok(conn) => {
            privstack_db::register_custom_functions(&conn).ok();
            Arc::new(std::sync::Mutex::new(conn))
        }
        Err(e) => {
            ffi_error!("[FFI] FAILED to open main database: {e:?}");
            return PrivStackError::StorageError;
        }
    };

    // All stores share the same connection
    let vault_manager = match VaultManager::open_with_conn(main_conn.clone()) {
        Ok(vm) => {
            ffi_debug!("[FFI] Vault manager initialized OK");
            Arc::new(vm)
        }
        Err(e) => {
            ffi_error!("[FFI] FAILED to init vault manager: {e:?}");
            return PrivStackError::StorageError;
        }
    };

    let blob_store = match BlobStore::open_with_conn(main_conn.clone()) {
        Ok(bs) => {
            ffi_debug!("[FFI] Blob store initialized OK");
            bs
        }
        Err(e) => {
            ffi_error!("[FFI] FAILED to init blob store: {e:?}");
            return PrivStackError::StorageError;
        }
    };

    let entity_store = match EntityStore::open_with_conn(main_conn.clone()) {
        Ok(s) => {
            ffi_debug!("[FFI] Entity store initialized OK");
            s
        }
        Err(e) => {
            ffi_error!("[FFI] FAILED to init entity store: {e:?}");
            return PrivStackError::StorageError;
        }
    };

    let event_store = match EventStore::open_with_conn(main_conn.clone()) {
        Ok(s) => {
            ffi_debug!("[FFI] Event store initialized OK");
            s
        }
        Err(e) => {
            ffi_error!("[FFI] FAILED to init event store: {e:?}");
            return PrivStackError::StorageError;
        }
    };

    let entity_store = Arc::new(entity_store);
    let event_store = Arc::new(event_store);

    let entity_registry = EntityRegistry::new();

    let runtime = match Runtime::new() {
        Ok(rt) => rt,
        Err(_) => return PrivStackError::Unknown,
    };

    let sync_config = SyncConfig::default();
    let sync_engine = SyncEngine::new(peer_id, sync_config);

    let activation_store = ActivationStore::new(ActivationStore::default_path());

    let device_name = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "PrivStack Device".to_string());

    // Dataset store (unencrypted SQLite for tabular data — not affected by SQLCipher)
    let dataset_store = {
        let ds_path = if path == ":memory:" {
            None
        } else {
            Some(Path::new(path).with_extension("datasets.db"))
        };
        match ds_path {
            Some(p) => {
                ffi_debug!("[FFI] Opening dataset store: {}", p.display());
                match privstack_datasets::DatasetStore::open(&p) {
                    Ok(ds) => {
                        ffi_debug!("[FFI] Dataset store opened OK");
                        Some(ds)
                    }
                    Err(e) => {
                        ffi_warn!("[FFI] WARN: Failed to open dataset store: {e:?}");
                        None
                    }
                }
            }
            None => match privstack_datasets::DatasetStore::open_in_memory() {
                Ok(ds) => Some(ds),
                Err(_) => None,
            },
        }
    };

    let mut handle = HANDLE.lock().unwrap();
    *handle = Some(PrivStackHandle {
        db_path: path.to_string(),
        main_conn,
        entity_store,
        event_store,
        entity_registry,
        peer_id,
        runtime,
        sync_engine,
        p2p_transport: None,
        orchestrator_handle: None,
        sync_event_rx: None,
        pairing_manager: Arc::new(std::sync::Mutex::new(PairingManager::new())),
        personal_policy: None,
        device_name,
        google_drive: None,
        icloud: None,
        activation_store,
        vault_manager,
        blob_store,
        dataset_store,
        cloud_api: None,
        cloud_sync_handle: None,
        cloud_event_tx: None,
        cloud_envelope_mgr: None,
        cloud_share_mgr: None,
        cloud_config: None,
        cloud_blob_mgr: None,
        cloud_dek_registry: None,
        cloud_user_id: None,
        cloud_active_workspace: None,
    });

    PrivStackError::Ok
}

/// Shared init logic with plugin host — accepts a closure to construct the PluginHostManager,
/// so tests can inject a policy-free manager without touching the filesystem.
///
/// See `init_core` for the two-phase initialization logic.
#[cfg(feature = "wasm-plugins")]
pub fn init_with_plugin_host_builder<F>(path: &str, build_plugin_host: F) -> PrivStackError
where
    F: FnOnce(Arc<EntityStore>, Arc<EventStore>) -> PluginHostManager,
{
    let peer_id = load_or_create_peer_id(path);

    // Detect whether the database is encrypted (salt file present).
    let salt_path = Path::new(path).with_extension(SALT_FILE_EXT);
    let is_encrypted = path != ":memory:" && salt_path.exists();

    let main_db_path = if path == ":memory:" {
        Path::new(":memory:").to_path_buf()
    } else {
        Path::new(path).with_extension("privstack.db")
    };

    let open_result = if path == ":memory:" || is_encrypted {
        if is_encrypted {
            ffi_info!(
                "[FFI] Salt file found — opening in-memory placeholder until auth_unlock"
            );
        }
        privstack_db::open_in_memory()
    } else {
        ffi_debug!("[FFI] Opening main database (unencrypted): {}", main_db_path.display());
        privstack_db::open_db_unencrypted(&main_db_path)
    };

    let main_conn = match open_result {
        Ok(conn) => {
            privstack_db::register_custom_functions(&conn).ok();
            Arc::new(std::sync::Mutex::new(conn))
        }
        Err(e) => {
            ffi_error!("[FFI] FAILED to open main database: {e:?}");
            return PrivStackError::StorageError;
        }
    };

    // All stores share the same connection
    let vault_manager = match VaultManager::open_with_conn(main_conn.clone()) {
        Ok(vm) => {
            ffi_debug!("[FFI] Vault manager initialized OK");
            Arc::new(vm)
        }
        Err(e) => {
            ffi_error!("[FFI] FAILED to init vault manager: {e:?}");
            return PrivStackError::StorageError;
        }
    };

    let blob_store = match BlobStore::open_with_conn(main_conn.clone()) {
        Ok(bs) => {
            ffi_debug!("[FFI] Blob store initialized OK");
            bs
        }
        Err(e) => {
            ffi_error!("[FFI] FAILED to init blob store: {e:?}");
            return PrivStackError::StorageError;
        }
    };

    let entity_store = match EntityStore::open_with_conn(main_conn.clone()) {
        Ok(s) => {
            ffi_debug!("[FFI] Entity store initialized OK");
            s
        }
        Err(e) => {
            ffi_error!("[FFI] FAILED to init entity store: {e:?}");
            return PrivStackError::StorageError;
        }
    };

    let event_store = match EventStore::open_with_conn(main_conn.clone()) {
        Ok(s) => {
            ffi_debug!("[FFI] Event store initialized OK");
            s
        }
        Err(e) => {
            ffi_error!("[FFI] FAILED to init event store: {e:?}");
            return PrivStackError::StorageError;
        }
    };

    let entity_store = Arc::new(entity_store);
    let event_store = Arc::new(event_store);

    let entity_registry = EntityRegistry::new();

    let plugin_host = build_plugin_host(
        Arc::clone(&entity_store),
        Arc::clone(&event_store),
    );

    let runtime = match Runtime::new() {
        Ok(rt) => rt,
        Err(_) => return PrivStackError::Unknown,
    };

    let sync_config = SyncConfig::default();
    let sync_engine = SyncEngine::new(peer_id, sync_config);

    let activation_store = ActivationStore::new(ActivationStore::default_path());

    let device_name = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "PrivStack Device".to_string());

    // Dataset store (unencrypted SQLite for tabular data)
    let dataset_store = {
        let ds_path = if path == ":memory:" {
            None
        } else {
            Some(Path::new(path).with_extension("datasets.db"))
        };
        match ds_path {
            Some(p) => {
                ffi_debug!("[FFI] Opening dataset store: {}", p.display());
                match privstack_datasets::DatasetStore::open(&p) {
                    Ok(ds) => {
                        ffi_debug!("[FFI] Dataset store opened OK");
                        Some(ds)
                    }
                    Err(e) => {
                        ffi_warn!("[FFI] WARN: Failed to open dataset store: {e:?}");
                        None
                    }
                }
            }
            None => match privstack_datasets::DatasetStore::open_in_memory() {
                Ok(ds) => Some(ds),
                Err(_) => None,
            },
        }
    };

    let mut handle = HANDLE.lock().unwrap();
    *handle = Some(PrivStackHandle {
        db_path: path.to_string(),
        main_conn,
        entity_store,
        event_store,
        entity_registry,
        peer_id,
        runtime,
        sync_engine,
        p2p_transport: None,
        orchestrator_handle: None,
        sync_event_rx: None,
        pairing_manager: Arc::new(std::sync::Mutex::new(PairingManager::new())),
        personal_policy: None,
        device_name,
        google_drive: None,
        icloud: None,
        activation_store,
        vault_manager,
        blob_store,
        dataset_store,
        plugin_host,
        cloud_api: None,
        cloud_sync_handle: None,
        cloud_event_tx: None,
        cloud_envelope_mgr: None,
        cloud_share_mgr: None,
        cloud_config: None,
        cloud_blob_mgr: None,
        cloud_dek_registry: None,
        cloud_user_id: None,
        cloud_active_workspace: None,
    });

    PrivStackError::Ok
}

/// Shuts down the PrivStack runtime and frees resources.
/// Checkpoints all SQLite databases to flush WAL files before dropping connections.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_shutdown() {
    let mut handle = HANDLE.lock().unwrap();

    // Flush WAL files before dropping connections so they don't persist on disk.
    if let Some(h) = handle.as_ref() {
        if let Err(e) = h.entity_store.checkpoint() {
            ffi_warn!("[shutdown] entity store checkpoint failed: {e}");
        }
        if let Err(e) = h.event_store.checkpoint() {
            ffi_warn!("[shutdown] event store checkpoint failed: {e}");
        }
        if let Err(e) = h.blob_store.checkpoint() {
            ffi_warn!("[shutdown] blob store checkpoint failed: {e}");
        }
        if let Err(e) = h.vault_manager.checkpoint() {
            ffi_warn!("[shutdown] vault checkpoint failed: {e}");
        }
        if let Some(ds) = &h.dataset_store {
            if let Err(e) = ds.checkpoint() {
                ffi_warn!("[shutdown] dataset store checkpoint failed: {e}");
            }
        }
        ffi_info!("[shutdown] all databases checkpointed");
    }

    *handle = None;
}

/// Returns the library version as a string.
///
/// # Safety
/// - The returned string is statically allocated and must not be freed.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_version() -> *const c_char {
    static VERSION: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();
    VERSION.as_ptr() as *const c_char
}

/// Frees a string allocated by this library.
///
/// # Safety
/// - `s` must be a string allocated by this library, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_free_string(s: *mut c_char) { unsafe {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}}

/// Frees a byte buffer allocated by this library.
///
/// # Safety
/// - `data` must be a pointer allocated by this library, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_free_bytes(data: *mut u8, len: usize) { unsafe {
    if !data.is_null() && len > 0 {
        drop(Box::from_raw(std::slice::from_raw_parts_mut(data, len)));
    }
}}

// ============================================================================
// App-Level Authentication Functions
// ============================================================================

/// Derives a SQLCipher encryption key from a password and salt.
///
/// Uses Argon2id (via `privstack_crypto`) to derive a 256-bit key, then
/// formats it as a SQLCipher raw hex key string (`x'...'`).
fn derive_db_key(
    password: &str,
    salt: &privstack_crypto::Salt,
) -> Result<String, PrivStackError> {
    let params = privstack_crypto::KdfParams::default();
    let derived = privstack_crypto::derive_key(password, salt, &params).map_err(|e| {
        ffi_error!("[FFI] Key derivation failed: {e:?}");
        PrivStackError::AuthError
    })?;
    Ok(privstack_crypto::derive_sqlcipher_key(&derived))
}

/// Checks if the app master password has been initialized.
///
/// Returns `true` if the salt file exists (encrypted database was created)
/// OR if the vault metadata indicates initialization (legacy unencrypted mode).
#[unsafe(no_mangle)]
pub extern "C" fn privstack_auth_is_initialized() -> bool {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return false,
    };

    // Salt file is the primary indicator for encrypted databases.
    if handle.has_salt_file() {
        return true;
    }

    // Fallback: check vault metadata (legacy unencrypted or :memory: mode).
    handle.vault_manager.is_initialized("default")
}

/// Checks if the app is currently unlocked.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_auth_is_unlocked() -> bool {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return false,
    };
    handle.vault_manager.is_unlocked("default")
}

/// Initializes the app with a master password (first-time setup).
///
/// For on-disk databases this:
/// 1. Generates a random salt and writes it to `<db_path>.privstack.salt`
/// 2. Derives a SQLCipher key from the password + salt
/// 3. Creates the encrypted `privstack.db` and swaps the in-memory placeholder
/// 4. Initializes the "default" vault on the now-open encrypted database
///
/// For `:memory:` databases, only step 4 is performed.
///
/// # Safety
/// - `master_password` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_auth_initialize(
    master_password: *const c_char,
) -> PrivStackError { unsafe {
    if master_password.is_null() {
        return PrivStackError::NullPointer;
    }

    let password = match CStr::from_ptr(master_password).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    // For on-disk databases: generate salt, create encrypted DB, swap connection
    if handle.db_path != ":memory:" {
        if handle.has_salt_file() {
            ffi_warn!("[FFI] auth_initialize called but salt file already exists");
            return PrivStackError::VaultAlreadyInitialized;
        }

        // 1. Generate salt
        let salt = privstack_crypto::Salt::random();

        // 2. Derive SQLCipher key
        let db_key = match derive_db_key(password, &salt) {
            Ok(k) => k,
            Err(e) => return e,
        };

        // 3. Create encrypted database
        let db_path = handle.main_db_path();
        ffi_info!("[FFI] Creating encrypted database: {}", db_path.display());

        let new_conn = match privstack_db::open_db(&db_path, &db_key) {
            Ok(c) => c,
            Err(e) => {
                ffi_error!("[FFI] Failed to create encrypted database: {e:?}");
                return PrivStackError::StorageError;
            }
        };

        // 4. Swap connection — all stores now point at the encrypted DB
        if let Err(e) = handle.swap_connection(new_conn) {
            ffi_error!("[FFI] Failed to swap connection: {e:?}");
            return e;
        }

        // 5. Write salt file AFTER successful DB creation (crash-safe ordering)
        if let Err(e) = std::fs::write(handle.salt_path(), salt.as_bytes()) {
            ffi_error!("[FFI] Failed to write salt file: {e:?}");
            return PrivStackError::StorageError;
        }

        ffi_info!("[FFI] Encrypted database created and salt file written");
    }

    // Initialize the vault on the (now real) database
    match handle.vault_manager.initialize("default", password) {
        Ok(_) => PrivStackError::Ok,
        Err(privstack_vault::VaultError::PasswordTooShort) => PrivStackError::PasswordTooShort,
        Err(privstack_vault::VaultError::AlreadyInitialized) => PrivStackError::VaultAlreadyInitialized,
        Err(privstack_vault::VaultError::Storage(_)) => PrivStackError::StorageError,
        Err(_) => PrivStackError::AuthError,
    }
}}

/// Unlocks the app with a master password.
///
/// For encrypted databases (salt file exists):
/// 1. Reads the salt from `<db_path>.privstack.salt`
/// 2. Derives the SQLCipher key from the password + salt
/// 3. Opens the encrypted `privstack.db` and swaps the in-memory placeholder
/// 4. Unlocks all initialized vaults on the now-open encrypted database
///
/// For unencrypted databases, only step 4 is performed (DB already open).
///
/// # Safety
/// - `master_password` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_auth_unlock(
    master_password: *const c_char,
) -> PrivStackError { unsafe {
    if master_password.is_null() {
        return PrivStackError::NullPointer;
    }

    let password = match CStr::from_ptr(master_password).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    // For encrypted databases: read salt, derive key, open DB, swap connection
    if handle.has_salt_file() {
        // 1. Read salt
        let salt_bytes = match std::fs::read(handle.salt_path()) {
            Ok(b) => b,
            Err(e) => {
                ffi_error!("[FFI] Failed to read salt file: {e:?}");
                return PrivStackError::StorageError;
            }
        };

        if salt_bytes.len() != privstack_crypto::SALT_SIZE {
            ffi_error!(
                "[FFI] Salt file has invalid size: {} (expected {})",
                salt_bytes.len(),
                privstack_crypto::SALT_SIZE
            );
            return PrivStackError::StorageError;
        }

        let mut salt_arr = [0u8; privstack_crypto::SALT_SIZE];
        salt_arr.copy_from_slice(&salt_bytes);
        let salt = privstack_crypto::Salt::from_bytes(salt_arr);

        // 2. Derive SQLCipher key
        let db_key = match derive_db_key(password, &salt) {
            Ok(k) => k,
            Err(e) => return e,
        };

        // 3. Open encrypted database
        let db_path = handle.main_db_path();
        ffi_info!("[FFI] Opening encrypted database: {}", db_path.display());

        let new_conn = match privstack_db::open_db(&db_path, &db_key) {
            Ok(c) => c,
            Err(e) => {
                ffi_error!("[FFI] Failed to open encrypted database (wrong password?): {e:?}");
                return PrivStackError::AuthError;
            }
        };

        // 4. Swap connection — all stores now point at the encrypted DB
        if let Err(e) = handle.swap_connection(new_conn) {
            ffi_error!("[FFI] Failed to swap connection: {e:?}");
            return e;
        }

        ffi_info!("[FFI] Encrypted database opened successfully");
    }

    // Unlock vaults on the (now real) database
    match handle.vault_manager.unlock_all(password) {
        Ok(_) => PrivStackError::Ok,
        Err(_) => PrivStackError::AuthError,
    }
}}

/// Locks the app, securing all sensitive data.
///
/// For encrypted databases, this also swaps the real database connection
/// back to an in-memory placeholder, ensuring no data is accessible from
/// the encrypted DB while locked.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_auth_lock() -> PrivStackError {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    // Lock all vaults first (clears in-memory DEKs)
    handle.vault_manager.lock_all();

    // For encrypted databases: swap back to in-memory placeholder so the
    // encrypted DB file is no longer held open.
    if handle.has_salt_file() {
        match privstack_db::open_in_memory() {
            Ok(placeholder) => {
                let mut guard = handle.main_conn.lock().unwrap();
                *guard = placeholder;
                ffi_info!("[FFI] Swapped to in-memory placeholder on lock");
            }
            Err(e) => {
                ffi_warn!("[FFI] Failed to create placeholder on lock: {e:?}");
            }
        }
    }

    PrivStackError::Ok
}

/// Changes the master password for all vaults.
///
/// For encrypted databases, this also re-keys the SQLCipher database with
/// a new key derived from the new password + a fresh salt.
///
/// # Safety
/// - `old_password` and `new_password` must be valid null-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_auth_change_password(
    old_password: *const c_char,
    new_password: *const c_char,
) -> PrivStackError { unsafe {
    if old_password.is_null() || new_password.is_null() {
        return PrivStackError::NullPointer;
    }

    let old_pwd = match CStr::from_ptr(old_password).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let new_pwd = match CStr::from_ptr(new_password).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    if new_pwd.len() < 8 {
        return PrivStackError::PasswordTooShort;
    }

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    // For encrypted databases: rekey with new password + fresh salt
    if handle.has_salt_file() {
        // Generate new salt
        let new_salt = privstack_crypto::Salt::random();

        // Derive new SQLCipher key
        let new_db_key = match derive_db_key(new_pwd, &new_salt) {
            Ok(k) => k,
            Err(e) => return e,
        };

        // Rekey the database (must be done while DB is open with old key)
        {
            let conn = handle.main_conn.lock().unwrap();
            if let Err(e) = privstack_db::rekey(&conn, &new_db_key) {
                ffi_error!("[FFI] Failed to rekey database: {e:?}");
                return PrivStackError::StorageError;
            }
        }

        // Write new salt file AFTER successful rekey
        if let Err(e) = std::fs::write(handle.salt_path(), new_salt.as_bytes()) {
            ffi_error!("[FFI] Failed to update salt file: {e:?}");
            return PrivStackError::StorageError;
        }

        ffi_info!("[FFI] Database rekeyed with new password");
    }

    // Change vault password (re-encrypts vault blobs with new key)
    match handle.vault_manager.change_password_all(old_pwd, new_pwd) {
        Ok(_) => PrivStackError::Ok,
        Err(_) => PrivStackError::AuthError,
    }
}}

// ============================================================================
// Recovery Functions
// ============================================================================

/// Sets up recovery for the default vault. Returns a 12-word BIP39 mnemonic
/// via `out_mnemonic`. The caller must free it with `privstack_free_string`.
///
/// # Safety
/// - `out_mnemonic` must be a valid pointer to a `*mut c_char`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_auth_setup_recovery(
    out_mnemonic: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if out_mnemonic.is_null() {
        return PrivStackError::NullPointer;
    }

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match handle.vault_manager.setup_recovery("default") {
        Ok(mnemonic) => {
            let c_str = match std::ffi::CString::new(mnemonic) {
                Ok(s) => s,
                Err(_) => return PrivStackError::Unknown,
            };
            *out_mnemonic = c_str.into_raw();
            PrivStackError::Ok
        }
        Err(privstack_vault::VaultError::Locked) => PrivStackError::VaultLocked,
        Err(_) => PrivStackError::AuthError,
    }
}}

/// Checks whether recovery is configured for the default vault.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_auth_has_recovery() -> bool {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return false,
    };
    handle
        .vault_manager
        .has_recovery("default")
        .unwrap_or(false)
}

/// Resets the master password using a recovery mnemonic.
///
/// For encrypted databases, also rekeys the SQLCipher database with a key
/// derived from the new password + fresh salt.  The database must already
/// be open (the user must have authenticated at least once in this session).
///
/// # Safety
/// - `mnemonic` and `new_password` must be valid null-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_auth_reset_with_recovery(
    mnemonic: *const c_char,
    new_password: *const c_char,
) -> PrivStackError { unsafe {
    if mnemonic.is_null() || new_password.is_null() {
        return PrivStackError::NullPointer;
    }

    let mnemonic_str = match CStr::from_ptr(mnemonic).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let new_pwd = match CStr::from_ptr(new_password).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    if new_pwd.len() < 8 {
        return PrivStackError::PasswordTooShort;
    }

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    // Vault recovery (re-encrypts vault blobs)
    match handle
        .vault_manager
        .reset_password_with_recovery("default", mnemonic_str, new_pwd)
    {
        Ok((_old_kb, _new_kb)) => {}
        Err(privstack_vault::VaultError::RecoveryNotConfigured) => {
            return PrivStackError::RecoveryNotConfigured;
        }
        Err(privstack_vault::VaultError::InvalidRecoveryMnemonic) => {
            return PrivStackError::InvalidRecoveryMnemonic;
        }
        Err(privstack_vault::VaultError::PasswordTooShort) => {
            return PrivStackError::PasswordTooShort;
        }
        Err(_) => {
            return PrivStackError::AuthError;
        }
    }

    // Rekey encrypted database with new password + fresh salt
    if handle.has_salt_file() {
        let new_salt = privstack_crypto::Salt::random();
        let new_db_key = match derive_db_key(new_pwd, &new_salt) {
            Ok(k) => k,
            Err(e) => return e,
        };

        {
            let conn = handle.main_conn.lock().unwrap();
            if let Err(e) = privstack_db::rekey(&conn, &new_db_key) {
                ffi_error!("[FFI] Failed to rekey database during recovery: {e:?}");
                return PrivStackError::StorageError;
            }
        }

        if let Err(e) = std::fs::write(handle.salt_path(), new_salt.as_bytes()) {
            ffi_error!("[FFI] Failed to update salt file during recovery: {e:?}");
            return PrivStackError::StorageError;
        }

        ffi_info!("[FFI] Database rekeyed during recovery");
    }

    PrivStackError::Ok
}}

/// Resets the master password using a recovery mnemonic and also recovers
/// cloud keypair (best-effort). Unified recovery for both vault + cloud.
///
/// For encrypted databases, also rekeys the SQLCipher database with a key
/// derived from the new password + fresh salt.
///
/// # Safety
/// - `mnemonic` and `new_password` must be valid null-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_auth_reset_with_unified_recovery(
    mnemonic: *const c_char,
    new_password: *const c_char,
) -> PrivStackError { unsafe {
    if mnemonic.is_null() || new_password.is_null() {
        return PrivStackError::NullPointer;
    }

    let mnemonic_str = match CStr::from_ptr(mnemonic).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let new_pwd = match CStr::from_ptr(new_password).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    if new_pwd.len() < 8 {
        return PrivStackError::PasswordTooShort;
    }

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    // 1. Vault recovery (mandatory)
    match handle
        .vault_manager
        .reset_password_with_recovery("default", mnemonic_str, new_pwd)
    {
        Ok((_old_kb, _new_kb)) => {}
        Err(privstack_vault::VaultError::RecoveryNotConfigured) => {
            return PrivStackError::RecoveryNotConfigured;
        }
        Err(privstack_vault::VaultError::InvalidRecoveryMnemonic) => {
            return PrivStackError::InvalidRecoveryMnemonic;
        }
        Err(privstack_vault::VaultError::PasswordTooShort) => {
            return PrivStackError::PasswordTooShort;
        }
        Err(_) => {
            return PrivStackError::AuthError;
        }
    }

    // 2. Rekey encrypted database with new password + fresh salt
    if handle.has_salt_file() {
        let new_salt = privstack_crypto::Salt::random();
        let new_db_key = match derive_db_key(new_pwd, &new_salt) {
            Ok(k) => k,
            Err(e) => return e,
        };

        {
            let conn = handle.main_conn.lock().unwrap();
            if let Err(e) = privstack_db::rekey(&conn, &new_db_key) {
                ffi_error!("[FFI] Failed to rekey database during unified recovery: {e:?}");
                return PrivStackError::StorageError;
            }
        }

        if let Err(e) = std::fs::write(handle.salt_path(), new_salt.as_bytes()) {
            ffi_error!("[FFI] Failed to update salt file during unified recovery: {e:?}");
            return PrivStackError::StorageError;
        }

        ffi_info!("[FFI] Database rekeyed during unified recovery");
    }

    // 3. Cloud recovery (best-effort — log failures, don't block vault recovery)
    if let (Some(env_mgr), Some(api), Some(config)) = (
        &handle.cloud_envelope_mgr,
        &handle.cloud_api,
        &handle.cloud_config,
    ) {
        let api = api.clone();
        let config = config.clone();
        let env_mgr = env_mgr.clone();

        let cloud_result = handle.runtime.block_on(async {
            if !api.is_authenticated().await {
                return Err("cloud API not authenticated".to_string());
            }

            let workspaces = api.list_workspaces().await.map_err(|e| e.to_string())?;
            let ws = workspaces.first().ok_or("no cloud workspace")?;

            let user_id = api.user_id().await.ok_or("no user ID")?;

            let transport = privstack_cloud::s3_transport::S3Transport::new(
                config.s3_bucket.clone(),
                config.s3_region.clone(),
                config.s3_endpoint_override.clone(),
            );

            let cred_mgr = privstack_cloud::credential_manager::CredentialManager::new(
                api.clone(),
                ws.workspace_id.clone(),
                config.credential_refresh_margin_secs,
            );
            let creds = cred_mgr.get_credentials().await.map_err(|e| e.to_string())?;

            let recovery_path = privstack_cloud::compaction::recovery_key_s3_key(user_id, &ws.workspace_id);
            let data = transport.download(&creds, &recovery_path).await.map_err(|e| e.to_string())?;

            let encrypted: privstack_crypto::EncryptedData =
                serde_json::from_slice(&data).map_err(|e| e.to_string())?;

            let secret_key = privstack_crypto::envelope::decrypt_private_key_with_mnemonic(
                &encrypted, mnemonic_str,
            ).map_err(|e| e.to_string())?;

            let keypair = privstack_crypto::envelope::CloudKeyPair::from_secret_bytes(
                secret_key.to_bytes(),
            );

            env_mgr.lock().await.set_keypair(keypair);
            Ok::<(), String>(())
        });

        if let Err(e) = cloud_result {
            ffi_warn!("[FFI] Cloud key recovery skipped during unified recovery: {e}");
        }
    }

    PrivStackError::Ok
}}

// ============================================================================
// Vault Management Functions
// ============================================================================

/// Creates a new vault with the given ID.
///
/// # Safety
/// - `vault_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_vault_create(vault_id: *const c_char) -> PrivStackError { unsafe {
    if vault_id.is_null() {
        return PrivStackError::NullPointer;
    }

    let id = match CStr::from_ptr(vault_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match handle.vault_manager.create_vault(id) {
        Ok(_) => PrivStackError::Ok,
        Err(_) => PrivStackError::StorageError,
    }
}}

/// Initializes a vault with a password.
///
/// # Safety
/// - `vault_id` and `password` must be valid null-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_vault_initialize(
    vault_id: *const c_char,
    password: *const c_char,
) -> PrivStackError { unsafe {
    if vault_id.is_null() || password.is_null() {
        return PrivStackError::NullPointer;
    }

    let id = match CStr::from_ptr(vault_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };
    let pwd = match CStr::from_ptr(password).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match handle.vault_manager.initialize(id, pwd) {
        Ok(_) => PrivStackError::Ok,
        Err(privstack_vault::VaultError::PasswordTooShort) => PrivStackError::PasswordTooShort,
        Err(privstack_vault::VaultError::AlreadyInitialized) => PrivStackError::VaultAlreadyInitialized,
        Err(privstack_vault::VaultError::Storage(_)) => PrivStackError::StorageError,
        Err(privstack_vault::VaultError::Crypto(_)) => PrivStackError::StorageError,
        Err(_) => PrivStackError::AuthError,
    }
}}

/// Unlocks a vault with a password.
///
/// # Safety
/// - `vault_id` and `password` must be valid null-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_vault_unlock(
    vault_id: *const c_char,
    password: *const c_char,
) -> PrivStackError { unsafe {
    if vault_id.is_null() || password.is_null() {
        return PrivStackError::NullPointer;
    }

    let id = match CStr::from_ptr(vault_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };
    let pwd = match CStr::from_ptr(password).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match handle.vault_manager.unlock(id, pwd) {
        Ok(_) => PrivStackError::Ok,
        Err(privstack_vault::VaultError::NotInitialized) => PrivStackError::NotInitialized,
        Err(privstack_vault::VaultError::InvalidPassword) => PrivStackError::AuthError,
        Err(privstack_vault::VaultError::VaultNotFound(_)) => PrivStackError::VaultNotFound,
        Err(privstack_vault::VaultError::Storage(_)) => PrivStackError::StorageError,
        Err(_) => PrivStackError::AuthError,
    }
}}

/// Locks a vault.
///
/// # Safety
/// - `vault_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_vault_lock(vault_id: *const c_char) -> PrivStackError { unsafe {
    if vault_id.is_null() {
        return PrivStackError::NullPointer;
    }

    let id = match CStr::from_ptr(vault_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    handle.vault_manager.lock(id);
    PrivStackError::Ok
}}

/// Locks all vaults.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_vault_lock_all() -> PrivStackError {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    handle.vault_manager.lock_all();
    PrivStackError::Ok
}

/// Checks if a vault has been initialized.
///
/// # Safety
/// - `vault_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_vault_is_initialized(vault_id: *const c_char) -> bool { unsafe {
    if vault_id.is_null() {
        return false;
    }

    let id = match CStr::from_ptr(vault_id).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let handle = HANDLE.lock().unwrap();
    match handle.as_ref() {
        Some(h) => h.vault_manager.is_initialized(id),
        None => false,
    }
}}

/// Checks if a vault is unlocked.
///
/// # Safety
/// - `vault_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_vault_is_unlocked(vault_id: *const c_char) -> bool { unsafe {
    if vault_id.is_null() {
        return false;
    }

    let id = match CStr::from_ptr(vault_id).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let handle = HANDLE.lock().unwrap();
    match handle.as_ref() {
        Some(h) => h.vault_manager.is_unlocked(id),
        None => false,
    }
}}

/// Changes a vault's password.
///
/// # Safety
/// - All parameters must be valid null-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_vault_change_password(
    vault_id: *const c_char,
    old_password: *const c_char,
    new_password: *const c_char,
) -> PrivStackError { unsafe {
    if vault_id.is_null() || old_password.is_null() || new_password.is_null() {
        return PrivStackError::NullPointer;
    }

    let id = match CStr::from_ptr(vault_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };
    let old_pwd = match CStr::from_ptr(old_password).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };
    let new_pwd = match CStr::from_ptr(new_password).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match handle.vault_manager.change_password(id, old_pwd, new_pwd) {
        Ok(_) => PrivStackError::Ok,
        Err(privstack_vault::VaultError::PasswordTooShort) => PrivStackError::PasswordTooShort,
        Err(privstack_vault::VaultError::InvalidPassword) => PrivStackError::AuthError,
        Err(privstack_vault::VaultError::Storage(_)) => PrivStackError::StorageError,
        Err(_) => PrivStackError::AuthError,
    }
}}

// ============================================================================
// Encrypted Blob Storage (Vault Blobs)
// ============================================================================

/// Stores an encrypted blob in a vault.
///
/// # Safety
/// - `vault_id` and `blob_id` must be valid null-terminated UTF-8 strings.
/// - `data` must point to `data_len` valid bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_vault_blob_store(
    vault_id: *const c_char,
    blob_id: *const c_char,
    data: *const u8,
    data_len: usize,
) -> PrivStackError { unsafe {
    if vault_id.is_null() || blob_id.is_null() || data.is_null() {
        return PrivStackError::NullPointer;
    }

    let vid = match CStr::from_ptr(vault_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };
    let bid = match CStr::from_ptr(blob_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let content = std::slice::from_raw_parts(data, data_len);

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match handle.vault_manager.store_blob(vid, bid, content) {
        Ok(_) => PrivStackError::Ok,
        Err(privstack_vault::VaultError::Locked) => PrivStackError::VaultLocked,
        Err(_) => PrivStackError::StorageError,
    }
}}

/// Reads an encrypted blob from a vault.
///
/// # Safety
/// - `vault_id` and `blob_id` must be valid null-terminated UTF-8 strings.
/// - `out_data` and `out_len` must be valid pointers.
/// - The returned data must be freed with `privstack_free_bytes`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_vault_blob_read(
    vault_id: *const c_char,
    blob_id: *const c_char,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> PrivStackError { unsafe {
    if vault_id.is_null() || blob_id.is_null() || out_data.is_null() || out_len.is_null() {
        return PrivStackError::NullPointer;
    }

    let vid = match CStr::from_ptr(vault_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };
    let bid = match CStr::from_ptr(blob_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match handle.vault_manager.read_blob(vid, bid) {
        Ok(data) => {
            let boxed = data.into_boxed_slice();
            *out_len = boxed.len();
            *out_data = Box::into_raw(boxed) as *mut u8;
            PrivStackError::Ok
        }
        Err(privstack_vault::VaultError::Locked) => PrivStackError::VaultLocked,
        Err(privstack_vault::VaultError::BlobNotFound(_)) => PrivStackError::NotFound,
        Err(_) => PrivStackError::StorageError,
    }
}}

/// Deletes an encrypted blob from a vault.
///
/// # Safety
/// - `vault_id` and `blob_id` must be valid null-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_vault_blob_delete(
    vault_id: *const c_char,
    blob_id: *const c_char,
) -> PrivStackError { unsafe {
    if vault_id.is_null() || blob_id.is_null() {
        return PrivStackError::NullPointer;
    }

    let vid = match CStr::from_ptr(vault_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };
    let bid = match CStr::from_ptr(blob_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match handle.vault_manager.delete_blob(vid, bid) {
        Ok(_) => PrivStackError::Ok,
        Err(privstack_vault::VaultError::BlobNotFound(_)) => PrivStackError::NotFound,
        Err(_) => PrivStackError::StorageError,
    }
}}

/// Lists blobs in a vault as JSON.
///
/// # Safety
/// - `vault_id` must be a valid null-terminated UTF-8 string.
/// - `out_json` must be a valid pointer. The result must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_vault_blob_list(
    vault_id: *const c_char,
    out_json: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if vault_id.is_null() || out_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let vid = match CStr::from_ptr(vault_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match handle.vault_manager.list_blobs(vid) {
        Ok(blobs) => match serde_json::to_string(&blobs) {
            Ok(json) => {
                *out_json = CString::new(json).unwrap().into_raw();
                PrivStackError::Ok
            }
            Err(_) => PrivStackError::JsonError,
        },
        Err(_) => PrivStackError::StorageError,
    }
}}

// ============================================================================
// Unencrypted Blob Storage
// ============================================================================

/// Stores a blob in the unencrypted blob store.
///
/// # Safety
/// - `namespace`, `blob_id` must be valid null-terminated UTF-8 strings.
/// - `data` must point to `data_len` valid bytes.
/// - `metadata_json` can be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_blob_store(
    namespace: *const c_char,
    blob_id: *const c_char,
    data: *const u8,
    data_len: usize,
    metadata_json: *const c_char,
) -> PrivStackError { unsafe {
    if namespace.is_null() || blob_id.is_null() || data.is_null() {
        return PrivStackError::NullPointer;
    }

    let ns = match CStr::from_ptr(namespace).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };
    let bid = match CStr::from_ptr(blob_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };
    let meta = if metadata_json.is_null() {
        None
    } else {
        match CStr::from_ptr(metadata_json).to_str() {
            Ok(s) => Some(s),
            Err(_) => return PrivStackError::InvalidUtf8,
        }
    };

    let content = std::slice::from_raw_parts(data, data_len);

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match handle.blob_store.store(ns, bid, content, meta) {
        Ok(_) => PrivStackError::Ok,
        Err(_) => PrivStackError::StorageError,
    }
}}

/// Reads a blob from the unencrypted blob store.
///
/// # Safety
/// - `namespace` and `blob_id` must be valid null-terminated UTF-8 strings.
/// - The returned data must be freed with `privstack_free_bytes`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_blob_read(
    namespace: *const c_char,
    blob_id: *const c_char,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> PrivStackError { unsafe {
    if namespace.is_null() || blob_id.is_null() || out_data.is_null() || out_len.is_null() {
        return PrivStackError::NullPointer;
    }

    let ns = match CStr::from_ptr(namespace).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };
    let bid = match CStr::from_ptr(blob_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match handle.blob_store.read(ns, bid) {
        Ok(data) => {
            let boxed = data.into_boxed_slice();
            *out_len = boxed.len();
            *out_data = Box::into_raw(boxed) as *mut u8;
            PrivStackError::Ok
        }
        Err(_) => PrivStackError::NotFound,
    }
}}

/// Deletes a blob from the unencrypted blob store.
///
/// # Safety
/// - `namespace` and `blob_id` must be valid null-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_blob_delete(
    namespace: *const c_char,
    blob_id: *const c_char,
) -> PrivStackError { unsafe {
    if namespace.is_null() || blob_id.is_null() {
        return PrivStackError::NullPointer;
    }

    let ns = match CStr::from_ptr(namespace).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };
    let bid = match CStr::from_ptr(blob_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match handle.blob_store.delete(ns, bid) {
        Ok(_) => PrivStackError::Ok,
        Err(_) => PrivStackError::NotFound,
    }
}}

/// Lists blobs in a namespace as JSON.
///
/// # Safety
/// - `namespace` must be a valid null-terminated UTF-8 string.
/// - `out_json` must be a valid pointer. The result must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_blob_list(
    namespace: *const c_char,
    out_json: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if namespace.is_null() || out_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let ns = match CStr::from_ptr(namespace).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match handle.blob_store.list(ns) {
        Ok(blobs) => match serde_json::to_string(&blobs) {
            Ok(json) => {
                *out_json = CString::new(json).unwrap().into_raw();
                PrivStackError::Ok
            }
            Err(_) => PrivStackError::JsonError,
        },
        Err(_) => PrivStackError::StorageError,
    }
}}

// ============================================================================
// Sync Functions
// ============================================================================

/// Starts the P2P sync transport and orchestrator.
///
/// # Safety
/// - Must be called after `privstack_init`.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_sync_start() -> PrivStackError {
    ffi_debug!("[FFI SYNC] privstack_sync_start called");
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    if handle.p2p_transport.is_some() {
        ffi_warn!("[FFI SYNC] privstack_sync_start: already running");
        return PrivStackError::SyncAlreadyRunning;
    }

    let mut config = P2pConfig::default();

    if let Some(sync_code) = handle.pairing_manager.lock().unwrap().current_code().cloned() {
        ffi_debug!("[FFI SYNC] privstack_sync_start: using sync code hash");
        if let Ok(hash_bytes) = hex::decode(&sync_code.hash) {
            config.sync_code_hash = Some(hash_bytes);
        }
    }

    config.device_name = handle.device_name.clone();
    ffi_debug!("[FFI SYNC] privstack_sync_start: device_name={}", config.device_name);

    // Load or create a persistent keypair so the libp2p PeerId is stable across restarts.
    // This ensures remote peers map to the same PrivStack PeerId every time.
    let keypair = load_or_create_keypair(&handle.db_path);

    let mut transport = match P2pTransport::with_keypair(handle.peer_id, keypair, config) {
        Ok(t) => t,
        Err(e) => {
            ffi_error!("[FFI SYNC] privstack_sync_start: failed to create transport: {:?}", e);
            return PrivStackError::SyncError;
        }
    };

    ffi_debug!("[FFI SYNC] privstack_sync_start: libp2p_peer_id={}", transport.libp2p_peer_id());
    ffi_debug!("[FFI SYNC] privstack_sync_start: starting transport...");
    let result = handle.runtime.block_on(transport.start());
    if let Err(e) = result {
        ffi_error!("[FFI SYNC] privstack_sync_start: transport start failed: {:?}", e);
        return PrivStackError::SyncError;
    }
    ffi_debug!("[FFI SYNC] privstack_sync_start: transport started");

    let transport = Arc::new(TokioMutex::new(transport));

    ffi_debug!("[FFI SYNC] privstack_sync_start: creating orchestrator...");
    let orch_entity_store = Arc::clone(&handle.entity_store);
    let orch_event_store = Arc::clone(&handle.event_store);

    // Always use PersonalSyncPolicy + pairing gate
    let policy = Arc::new(PersonalSyncPolicy::new());
    handle.personal_policy = Some(policy.clone());

    let (orch_handle, event_rx, command_rx, orchestrator) = {
        ffi_debug!("[FFI SYNC] privstack_sync_start: using personal orchestrator with pairing");
        create_personal_orchestrator(
            handle.peer_id,
            orch_entity_store,
            orch_event_store,
            OrchestratorConfig::default(),
            policy,
            handle.pairing_manager.clone(),
        )
    };

    let transport_clone = transport.clone();
    handle.runtime.spawn(async move {
        ffi_debug!("[FFI SYNC] Orchestrator task starting...");
        if let Err(e) = orchestrator.run(transport_clone, command_rx).await {
            ffi_error!("[FFI SYNC] Orchestrator error: {}", e);
        }
        ffi_debug!("[FFI SYNC] Orchestrator task exiting");
    });

    handle.p2p_transport = Some(transport);
    handle.orchestrator_handle = Some(orch_handle);
    handle.sync_event_rx = Some(event_rx);

    ffi_debug!("[FFI SYNC] privstack_sync_start: complete");
    PrivStackError::Ok
}

/// Stops the P2P sync transport and orchestrator.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_sync_stop() -> PrivStackError {
    ffi_debug!("[FFI SYNC] privstack_sync_stop called");
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    if let Some(ref orch_handle) = handle.orchestrator_handle {
        ffi_debug!("[FFI SYNC] privstack_sync_stop: shutting down orchestrator...");
        let _ = handle.runtime.block_on(orch_handle.shutdown());
        ffi_debug!("[FFI SYNC] privstack_sync_stop: orchestrator shutdown complete");
    }
    handle.orchestrator_handle = None;
    handle.sync_event_rx = None;

    if let Some(ref transport) = handle.p2p_transport {
        ffi_debug!("[FFI SYNC] privstack_sync_stop: stopping transport...");
        let _ = handle.runtime.block_on(async {
            let mut t = transport.lock().await;
            t.stop().await
        });
        ffi_debug!("[FFI SYNC] privstack_sync_stop: transport stopped");
    }
    handle.p2p_transport = None;

    ffi_debug!("[FFI SYNC] privstack_sync_stop: complete");
    PrivStackError::Ok
}

/// Returns whether sync is running.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_sync_is_running() -> bool {
    let handle = HANDLE.lock().unwrap();
    match handle.as_ref() {
        Some(h) => {
            if let Some(ref transport) = h.p2p_transport {
                h.runtime.block_on(async {
                    match transport.try_lock() {
                        Ok(t) => t.is_running(),
                        Err(_) => true,
                    }
                })
            } else {
                false
            }
        }
        None => false,
    }
}

/// Gets the current sync status.
///
/// # Safety
/// - `out_json` must be a valid pointer.
/// - The returned string must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_sync_status(out_json: *mut *mut c_char) -> PrivStackError { unsafe {
    if out_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let running = if let Some(ref transport) = handle.p2p_transport {
        handle.runtime.block_on(async {
            match transport.try_lock() {
                Ok(t) => t.is_running(),
                Err(_) => true,
            }
        })
    } else {
        false
    };

    let discovered_peers = if let Some(ref transport) = handle.p2p_transport {
        handle.runtime.block_on(async {
            let t = transport.lock().await;
            t.discovered_peers()
                .into_iter()
                .map(|p| DiscoveredPeerInfo {
                    peer_id: p.peer_id.to_string(),
                    device_name: p.device_name.clone(),
                    discovery_method: format!("{:?}", p.discovery_method),
                    addresses: p.addresses.iter().map(|a| a.to_string()).collect(),
                })
                .collect()
        })
    } else {
        Vec::new()
    };

    let status = SyncStatus {
        running,
        local_peer_id: handle.peer_id.to_string(),
        discovered_peers,
    };

    match serde_json::to_string(&status) {
        Ok(json) => {
            let c_json = CString::new(json).unwrap();
            *out_json = c_json.into_raw();
            PrivStackError::Ok
        }
        Err(_) => PrivStackError::JsonError,
    }
}}

/// Polls for the next sync event.
///
/// # Safety
/// - `out_json` must be a valid pointer.
/// - The returned string must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_sync_poll_event(out_json: *mut *mut c_char) -> PrivStackError { unsafe {
    if out_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let rx = match handle.sync_event_rx.as_mut() {
        Some(rx) => rx,
        None => {
            *out_json = std::ptr::null_mut();
            return PrivStackError::SyncNotRunning;
        }
    };

    match rx.try_recv() {
        Ok(event) => {
            let dto = SyncEventDto::from(event);
            match serde_json::to_string(&dto) {
                Ok(json) => {
                    let c_json = CString::new(json).unwrap();
                    *out_json = c_json.into_raw();
                    PrivStackError::Ok
                }
                Err(_) => PrivStackError::JsonError,
            }
        }
        Err(mpsc::error::TryRecvError::Empty) => {
            *out_json = std::ptr::null_mut();
            PrivStackError::Ok
        }
        Err(mpsc::error::TryRecvError::Disconnected) => {
            *out_json = std::ptr::null_mut();
            PrivStackError::SyncNotRunning
        }
    }
}}

/// Triggers a manual sync with all discovered peers.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_sync_trigger() -> PrivStackError {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let orch_handle = match &handle.orchestrator_handle {
        Some(oh) => oh,
        None => return PrivStackError::SyncNotRunning,
    };

    // trigger_sync is not a direct method; sync is continuous while running
    let _ = orch_handle;
    PrivStackError::Ok
}

/// Publishes an event payload for sync.
///
/// # Safety
/// - `event_json` must be a valid null-terminated UTF-8 JSON string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_sync_publish_event(
    event_json: *const c_char,
) -> PrivStackError { unsafe {
    if event_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let json_str = match CStr::from_ptr(event_json).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let event: Event = match serde_json::from_str(json_str) {
        Ok(e) => e,
        Err(_) => return PrivStackError::JsonError,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let orch_handle = match &handle.orchestrator_handle {
        Some(oh) => oh,
        None => return PrivStackError::SyncNotRunning,
    };

    match handle.runtime.block_on(orch_handle.record_event(event)) {
        Ok(_) => PrivStackError::Ok,
        Err(_) => PrivStackError::SyncError,
    }
}}

/// Alias for `privstack_sync_status` — gets the current sync status.
///
/// # Safety
/// - `out_json` must be a valid pointer. The result must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_sync_get_status(
    out_json: *mut *mut c_char,
) -> PrivStackError { unsafe {
    privstack_sync_status(out_json)
}}

/// Gets the local peer ID as a string.
///
/// # Safety
/// - `out_peer_id` must be a valid pointer. The result must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_sync_get_peer_id(
    out_peer_id: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if out_peer_id.is_null() {
        return PrivStackError::NullPointer;
    }

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let c_str = CString::new(handle.peer_id.to_string()).unwrap();
    *out_peer_id = c_str.into_raw();
    PrivStackError::Ok
}}

/// Gets discovered peers as a JSON array.
///
/// # Safety
/// - `out_json` must be a valid pointer. The result must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_sync_get_peers(
    out_json: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if out_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let peers = if let Some(ref transport) = handle.p2p_transport {
        handle.runtime.block_on(async {
            let t = transport.lock().await;
            t.discovered_peers()
                .into_iter()
                .map(|p| DiscoveredPeerInfo {
                    peer_id: p.peer_id.to_string(),
                    device_name: p.device_name.clone(),
                    discovery_method: format!("{:?}", p.discovery_method),
                    addresses: p.addresses.iter().map(|a| a.to_string()).collect(),
                })
                .collect::<Vec<_>>()
        })
    } else {
        Vec::new()
    };

    match serde_json::to_string(&peers) {
        Ok(json) => {
            let c_json = CString::new(json).unwrap();
            *out_json = c_json.into_raw();
            PrivStackError::Ok
        }
        Err(_) => PrivStackError::JsonError,
    }
}}

/// Gets the count of discovered peers. Returns -1 if not initialized.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_sync_peer_count() -> c_int {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return -1,
    };

    if let Some(ref transport) = handle.p2p_transport {
        handle.runtime.block_on(async {
            match transport.try_lock() {
                Ok(t) => t.discovered_peers().len() as c_int,
                Err(_) => -1,
            }
        })
    } else {
        0
    }
}

/// Shares a document for sync with all peers.
///
/// # Safety
/// - `document_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_sync_share_document(
    document_id: *const c_char,
) -> PrivStackError { unsafe {
    if document_id.is_null() {
        return PrivStackError::NullPointer;
    }

    let doc_str = match CStr::from_ptr(document_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let eid: EntityId = match doc_str.parse() {
        Ok(id) => id,
        Err(_) => return PrivStackError::InvalidArgument,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let orch_handle = match &handle.orchestrator_handle {
        Some(oh) => oh,
        None => return PrivStackError::SyncNotRunning,
    };

    match handle.runtime.block_on(orch_handle.share_entity(eid)) {
        Ok(_) => PrivStackError::Ok,
        Err(_) => PrivStackError::SyncError,
    }
}}

/// Records a local event for sync. Takes a document ID and event JSON payload.
///
/// # Safety
/// - `document_id` and `event_json` must be valid null-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_sync_record_event(
    document_id: *const c_char,
    event_json: *const c_char,
) -> PrivStackError { unsafe {
    if document_id.is_null() || event_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let _doc_str = match CStr::from_ptr(document_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let json_str = match CStr::from_ptr(event_json).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let event: Event = match serde_json::from_str(json_str) {
        Ok(e) => e,
        Err(_) => return PrivStackError::JsonError,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    // Save to event store immediately (same rationale as privstack_sync_snapshot).
    if let Err(e) = handle.event_store.save_event(&event) {
        ffi_error!("[FFI SYNC] record_event: failed to save event to store: {:?}", e);
        return PrivStackError::SyncError;
    }

    let orch_handle = match &handle.orchestrator_handle {
        Some(oh) => oh,
        None => return PrivStackError::SyncNotRunning,
    };

    match handle.runtime.block_on(orch_handle.record_event(event)) {
        Ok(_) => PrivStackError::Ok,
        Err(_) => PrivStackError::SyncError,
    }
}}

/// Polls for all available sync events (non-blocking). Returns a JSON array.
///
/// # Safety
/// - `out_json` must be a valid pointer. The result must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_sync_poll_events(
    out_json: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if out_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let rx = match handle.sync_event_rx.as_mut() {
        Some(rx) => rx,
        None => {
            // Return empty array when sync is not running
            let c_json = CString::new("[]").unwrap();
            *out_json = c_json.into_raw();
            return PrivStackError::Ok;
        }
    };

    let mut events = Vec::new();
    loop {
        match rx.try_recv() {
            Ok(event) => events.push(SyncEventDto::from(event)),
            Err(_) => break,
        }
    }

    match serde_json::to_string(&events) {
        Ok(json) => {
            let c_json = CString::new(json).unwrap();
            *out_json = c_json.into_raw();
            PrivStackError::Ok
        }
        Err(_) => PrivStackError::JsonError,
    }
}}

/// Triggers immediate sync for a document with all known peers.
///
/// # Safety
/// - `document_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_sync_document(
    document_id: *const c_char,
) -> PrivStackError { unsafe {
    if document_id.is_null() {
        return PrivStackError::NullPointer;
    }

    let doc_str = match CStr::from_ptr(document_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let eid: EntityId = match doc_str.parse() {
        Ok(id) => id,
        Err(_) => return PrivStackError::InvalidArgument,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let orch_handle = match &handle.orchestrator_handle {
        Some(oh) => oh,
        None => return PrivStackError::SyncNotRunning,
    };

    match handle.runtime.block_on(orch_handle.sync_entity(eid)) {
        Ok(_) => PrivStackError::Ok,
        Err(_) => PrivStackError::SyncError,
    }
}}

/// Records a full entity snapshot for sync.
///
/// # Safety
/// - All string parameters must be valid null-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_sync_snapshot(
    document_id: *const c_char,
    entity_type: *const c_char,
    json_data: *const c_char,
) -> PrivStackError { unsafe {
    if document_id.is_null() || entity_type.is_null() || json_data.is_null() {
        return PrivStackError::NullPointer;
    }

    let doc_str = match CStr::from_ptr(document_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let eid: EntityId = match doc_str.parse() {
        Ok(id) => id,
        Err(_) => return PrivStackError::InvalidArgument,
    };

    let etype_str = match CStr::from_ptr(entity_type).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let data_str = match CStr::from_ptr(json_data).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let event = Event::full_snapshot(eid, handle.peer_id, etype_str, data_str);

    // Save to event store immediately so it's visible even if a sync cycle is in
    // progress (periodic_sync holds the command loop, blocking RecordLocalEvent).
    // The duplicate INSERT OR IGNORE in handle_local_event is harmless.
    if let Err(e) = handle.event_store.save_event(&event) {
        ffi_error!("[FFI SYNC] snapshot: failed to save event to store: {:?}", e);
        return PrivStackError::SyncError;
    }

    let orch_handle = match &handle.orchestrator_handle {
        Some(oh) => oh,
        None => return PrivStackError::SyncNotRunning,
    };

    match handle.runtime.block_on(orch_handle.record_event(event)) {
        Ok(_) => PrivStackError::Ok,
        Err(_) => PrivStackError::SyncError,
    }
}}

/// Imports an entity received from sync into the local store.
///
/// # Safety
/// - All string parameters must be valid null-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_sync_import_entity(
    entity_type: *const c_char,
    json_data: *const c_char,
) -> PrivStackError { unsafe {
    if entity_type.is_null() || json_data.is_null() {
        return PrivStackError::NullPointer;
    }

    let _etype_str = match CStr::from_ptr(entity_type).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let data_str = match CStr::from_ptr(json_data).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let entity: Entity = match serde_json::from_str(data_str) {
        Ok(e) => e,
        Err(_) => return PrivStackError::JsonError,
    };

    match handle.entity_store.save_entity_raw(&entity) {
        Ok(_) => PrivStackError::Ok,
        Err(_) => PrivStackError::StorageError,
    }
}}

// ============================================================================
// Pairing Functions
// ============================================================================

/// Generates a new sync code for pairing.
///
/// # Safety
/// - `out_code` must be a valid pointer. The result must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_pairing_generate_code(
    out_code: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if out_code.is_null() {
        return PrivStackError::NullPointer;
    }

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let code = SyncCode::generate();
    handle.pairing_manager.lock().unwrap().set_sync_code(code.clone());
    let json = match serde_json::to_string(&code) {
        Ok(j) => j,
        Err(_) => return PrivStackError::JsonError,
    };
    let c_str = CString::new(json).unwrap();
    *out_code = c_str.into_raw();
    PrivStackError::Ok
}}

/// Joins a sync group using a code.
///
/// # Safety
/// - `code` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_pairing_join_code(code: *const c_char) -> PrivStackError { unsafe {
    if code.is_null() {
        return PrivStackError::NullPointer;
    }

    let code_str = match CStr::from_ptr(code).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let sync_code = match SyncCode::from_input(code_str) {
        Ok(c) => c,
        Err(_) => return PrivStackError::InvalidSyncCode,
    };

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    handle.pairing_manager.lock().unwrap().set_sync_code(sync_code);
    PrivStackError::Ok
}}

/// Gets the current sync code (if any).
///
/// # Safety
/// - `out_code` must be a valid pointer. The result must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_pairing_get_code(
    out_code: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if out_code.is_null() {
        return PrivStackError::NullPointer;
    }

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let code = handle.pairing_manager.lock().unwrap().current_code().cloned();
    match code {
        Some(code) => {
            let json = match serde_json::to_string(&code) {
                Ok(j) => j,
                Err(_) => return PrivStackError::JsonError,
            };
            let c_str = CString::new(json).unwrap();
            *out_code = c_str.into_raw();
            PrivStackError::Ok
        }
        None => {
            *out_code = std::ptr::null_mut();
            PrivStackError::Ok
        }
    }
}}

/// Lists trusted peers as JSON.
///
/// # Safety
/// - `out_json` must be a valid pointer. The result must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_pairing_get_trusted_peers(
    out_json: *mut *mut c_char,
) -> PrivStackError { unsafe {
    privstack_pairing_list_peers(out_json)
}}

/// Lists all discovered peers pending approval (JSON array).
///
/// # Safety
/// - `out_json` must be a valid pointer. The result must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_pairing_get_discovered_peers(
    out_json: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if out_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let peers: Vec<_> = handle.pairing_manager.lock().unwrap().discovered_peers().into_iter().cloned().collect();
    match serde_json::to_string(&peers) {
        Ok(json) => {
            let c_json = CString::new(json).unwrap();
            *out_json = c_json.into_raw();
            PrivStackError::Ok
        }
        Err(_) => PrivStackError::JsonError,
    }
}}

/// Lists all trusted peers (JSON array).
///
/// # Safety
/// - `out_json` must be a valid pointer. The result must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_pairing_list_peers(
    out_json: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if out_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let peers: Vec<_> = handle.pairing_manager.lock().unwrap().trusted_peers().into_iter().cloned().collect();
    match serde_json::to_string(&peers) {
        Ok(json) => {
            let c_json = CString::new(json).unwrap();
            *out_json = c_json.into_raw();
            PrivStackError::Ok
        }
        Err(_) => PrivStackError::JsonError,
    }
}}

/// Adds a discovered peer as trusted.
///
/// # Safety
/// - `peer_id` and `device_name` must be valid null-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_pairing_trust_peer(
    peer_id: *const c_char,
    device_name: *const c_char,
) -> PrivStackError { unsafe {
    if peer_id.is_null() {
        return PrivStackError::NullPointer;
    }

    let pid_str = match CStr::from_ptr(peer_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let _name = if device_name.is_null() {
        None
    } else {
        match CStr::from_ptr(device_name).to_str() {
            Ok(s) => Some(s.to_string()),
            Err(_) => return PrivStackError::InvalidUtf8,
        }
    };

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    handle.pairing_manager.lock().unwrap().approve_peer(pid_str);
    PrivStackError::Ok
}}

/// Removes a trusted peer.
///
/// # Safety
/// - `peer_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_pairing_remove_peer(peer_id: *const c_char) -> PrivStackError { unsafe {
    if peer_id.is_null() {
        return PrivStackError::NullPointer;
    }

    let pid_str = match CStr::from_ptr(peer_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    handle.pairing_manager.lock().unwrap().remove_trusted_peer(pid_str);
    PrivStackError::Ok
}}

/// Alias for `privstack_pairing_remove_peer` — removes a trusted peer.
///
/// # Safety
/// - `peer_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_pairing_remove_trusted_peer(
    peer_id: *const c_char,
) -> PrivStackError { unsafe {
    privstack_pairing_remove_peer(peer_id)
}}

/// Alias for `privstack_pairing_join_code` — sets sync code from user input.
///
/// # Safety
/// - `code` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_pairing_set_code(
    code: *const c_char,
) -> PrivStackError { unsafe {
    privstack_pairing_join_code(code)
}}

/// Clears the current sync code and discovered peers.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_pairing_clear_code() -> PrivStackError {
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    handle.pairing_manager.lock().unwrap().clear_sync_code();
    PrivStackError::Ok
}

/// Alias for `privstack_pairing_trust_peer` — approves a discovered peer.
///
/// # Safety
/// - `peer_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_pairing_approve_peer(
    peer_id: *const c_char,
) -> PrivStackError { unsafe {
    privstack_pairing_trust_peer(peer_id, std::ptr::null())
}}

/// Rejects a discovered peer.
///
/// # Safety
/// - `peer_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_pairing_reject_peer(
    peer_id: *const c_char,
) -> PrivStackError { unsafe {
    if peer_id.is_null() {
        return PrivStackError::NullPointer;
    }

    let pid_str = match CStr::from_ptr(peer_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    handle.pairing_manager.lock().unwrap().reject_peer(pid_str);
    PrivStackError::Ok
}}

/// Checks if a peer is trusted.
///
/// # Safety
/// - `peer_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_pairing_is_trusted(
    peer_id: *const c_char,
) -> bool { unsafe {
    if peer_id.is_null() {
        return false;
    }

    let pid_str = match CStr::from_ptr(peer_id).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return false,
    };

    handle.pairing_manager.lock().unwrap().is_trusted(pid_str)
}}

/// Saves the pairing state to JSON.
///
/// # Safety
/// - `out_json` must be a valid pointer. The result must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_pairing_save_state(
    out_json: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if out_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let result = handle.pairing_manager.lock().unwrap().to_json();
    match result {
        Ok(json) => {
            let c_json = CString::new(json).unwrap();
            *out_json = c_json.into_raw();
            PrivStackError::Ok
        }
        Err(_) => PrivStackError::JsonError,
    }
}}

/// Loads the pairing state from JSON.
///
/// # Safety
/// - `json` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_pairing_load_state(json: *const c_char) -> PrivStackError { unsafe {
    if json.is_null() {
        return PrivStackError::NullPointer;
    }

    let json_str = match CStr::from_ptr(json).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match PairingManager::from_json(json_str) {
        Ok(manager) => {
            *handle.pairing_manager.lock().unwrap() = manager;
            PrivStackError::Ok
        }
        Err(_) => PrivStackError::JsonError,
    }
}}

/// Gets the device name.
///
/// # Safety
/// - `out_name` must be a valid pointer. The result must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_pairing_get_device_name(
    out_name: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if out_name.is_null() {
        return PrivStackError::NullPointer;
    }

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let c_str = CString::new(handle.device_name.clone()).unwrap();
    *out_name = c_str.into_raw();
    PrivStackError::Ok
}}

/// Sets the device name.
///
/// # Safety
/// - `name` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_pairing_set_device_name(name: *const c_char) -> PrivStackError { unsafe {
    if name.is_null() {
        return PrivStackError::NullPointer;
    }

    let name_str = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    handle.device_name = name_str.to_string();
    PrivStackError::Ok
}}

// ============================================================================
// Personal Sharing Functions
// ============================================================================

/// Shares an entity with a specific peer (personal sharing).
///
/// # Safety
/// - `entity_id` and `peer_id` must be valid null-terminated UTF-8 UUID strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_share_entity_with_peer(
    entity_id: *const c_char,
    peer_id: *const c_char,
) -> PrivStackError { unsafe {
    if entity_id.is_null() || peer_id.is_null() {
        return PrivStackError::NullPointer;
    }

    let eid_str = match CStr::from_ptr(entity_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };
    let pid_str = match CStr::from_ptr(peer_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let eid: EntityId = match eid_str.parse() {
        Ok(id) => id,
        Err(_) => return PrivStackError::JsonError,
    };
    let pid: PeerId = match pid_str.parse() {
        Ok(id) => id,
        Err(_) => return PrivStackError::JsonError,
    };

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    if let Some(policy) = &handle.personal_policy {
        handle.runtime.block_on(policy.share(eid, pid));
    }

    if let Some(orch) = &handle.orchestrator_handle {
        let _ = handle.runtime.block_on(
            orch.send(SyncCommand::ShareEntityWithPeer {
                entity_id: eid,
                peer_id: pid,
            }),
        );
    }

    PrivStackError::Ok
}}

/// Unshares an entity from a specific peer (personal sharing).
///
/// # Safety
/// - `entity_id` and `peer_id` must be valid null-terminated UTF-8 UUID strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_unshare_entity_with_peer(
    entity_id: *const c_char,
    peer_id: *const c_char,
) -> PrivStackError { unsafe {
    if entity_id.is_null() || peer_id.is_null() {
        return PrivStackError::NullPointer;
    }

    let eid_str = match CStr::from_ptr(entity_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };
    let pid_str = match CStr::from_ptr(peer_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let eid: EntityId = match eid_str.parse() {
        Ok(id) => id,
        Err(_) => return PrivStackError::JsonError,
    };
    let pid: PeerId = match pid_str.parse() {
        Ok(id) => id,
        Err(_) => return PrivStackError::JsonError,
    };

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    if let Some(policy) = &handle.personal_policy {
        handle.runtime.block_on(policy.unshare(eid, pid));
    }

    PrivStackError::Ok
}}

/// Lists all peers that have access to a given entity.
/// Returns a JSON array of peer ID strings.
///
/// # Safety
/// - `entity_id` must be a valid null-terminated UTF-8 UUID string.
/// - `out_json` must be a valid pointer. Result must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_list_shared_peers(
    entity_id: *const c_char,
    out_json: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if entity_id.is_null() || out_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let eid_str = match CStr::from_ptr(entity_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };
    let eid: EntityId = match eid_str.parse() {
        Ok(id) => id,
        Err(_) => return PrivStackError::JsonError,
    };

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let peers: Vec<String> = if let Some(policy) = &handle.personal_policy {
        handle
            .runtime
            .block_on(policy.shared_peers(&eid))
            .into_iter()
            .map(|p| p.to_string())
            .collect()
    } else {
        Vec::new()
    };

    match serde_json::to_string(&peers) {
        Ok(json) => {
            let c_str = CString::new(json).unwrap();
            *out_json = c_str.into_raw();
            PrivStackError::Ok
        }
        Err(_) => PrivStackError::JsonError,
    }
}}

// ============================================================================
// Cloud Sync Functions
// ============================================================================

/// Initializes Google Drive cloud storage.
///
/// # Safety
/// - `client_id` and `client_secret` must be valid null-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_cloud_init_google_drive(
    client_id: *const c_char,
    client_secret: *const c_char,
) -> PrivStackError { unsafe {
    if client_id.is_null() || client_secret.is_null() {
        return PrivStackError::NullPointer;
    }

    let client_id_str = match CStr::from_ptr(client_id).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let client_secret_str = match CStr::from_ptr(client_secret).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let config = GoogleDriveConfig {
        client_id: client_id_str,
        client_secret: client_secret_str,
        ..Default::default()
    };

    handle.google_drive = Some(GoogleDriveStorage::new(config));
    PrivStackError::Ok
}}

/// Initializes iCloud Drive storage.
///
/// # Safety
/// - `bundle_id` can be null to use the default, or a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_cloud_init_icloud(bundle_id: *const c_char) -> PrivStackError { unsafe {
    let bundle_id_str = if bundle_id.is_null() {
        "com.privstack.app".to_string()
    } else {
        match CStr::from_ptr(bundle_id).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return PrivStackError::InvalidUtf8,
        }
    };

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let config = ICloudConfig {
        bundle_id: bundle_id_str,
        ..Default::default()
    };

    handle.icloud = Some(ICloudStorage::new(config));
    PrivStackError::Ok
}}

/// Starts authentication for a cloud provider.
///
/// # Safety
/// - `out_auth_url` will receive a pointer to a URL string (or null if no auth needed).
/// - The returned string must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_cloud_authenticate(
    provider: CloudProvider,
    out_auth_url: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if out_auth_url.is_null() {
        return PrivStackError::NullPointer;
    }

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let result = match provider {
        CloudProvider::GoogleDrive => {
            let storage = match handle.google_drive.as_mut() {
                Some(s) => s,
                None => return PrivStackError::NotInitialized,
            };
            handle.runtime.block_on(storage.authenticate())
        }
        CloudProvider::ICloud => {
            let storage = match handle.icloud.as_mut() {
                Some(s) => s,
                None => return PrivStackError::NotInitialized,
            };
            handle.runtime.block_on(storage.authenticate())
        }
    };

    match result {
        Ok(Some(url)) => {
            let c_str = CString::new(url).unwrap();
            *out_auth_url = c_str.into_raw();
            PrivStackError::Ok
        }
        Ok(None) => {
            *out_auth_url = std::ptr::null_mut();
            PrivStackError::Ok
        }
        Err(_) => PrivStackError::AuthError,
    }
}}

/// Completes OAuth authentication with an authorization code.
///
/// # Safety
/// - `auth_code` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_cloud_complete_auth(
    provider: CloudProvider,
    auth_code: *const c_char,
) -> PrivStackError { unsafe {
    if auth_code.is_null() {
        return PrivStackError::NullPointer;
    }

    let code_str = match CStr::from_ptr(auth_code).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let result = match provider {
        CloudProvider::GoogleDrive => {
            let storage = match handle.google_drive.as_mut() {
                Some(s) => s,
                None => return PrivStackError::NotInitialized,
            };
            handle.runtime.block_on(storage.complete_auth(code_str))
        }
        CloudProvider::ICloud => {
            let storage = match handle.icloud.as_mut() {
                Some(s) => s,
                None => return PrivStackError::NotInitialized,
            };
            handle.runtime.block_on(storage.complete_auth(code_str))
        }
    };

    match result {
        Ok(_) => PrivStackError::Ok,
        Err(_) => PrivStackError::AuthError,
    }
}}

/// Checks if cloud storage is authenticated.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_cloud_is_authenticated(provider: CloudProvider) -> bool {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return false,
    };

    match provider {
        CloudProvider::GoogleDrive => handle
            .google_drive
            .as_ref()
            .map_or(false, |s| s.is_authenticated()),
        CloudProvider::ICloud => handle
            .icloud
            .as_ref()
            .map_or(false, |s| s.is_authenticated()),
    }
}

/// Lists files in cloud storage sync folder.
///
/// # Safety
/// - `out_json` will receive a pointer to a JSON array string.
/// - The returned string must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_cloud_list_files(
    provider: CloudProvider,
    out_json: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if out_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let result = match provider {
        CloudProvider::GoogleDrive => {
            let storage = match handle.google_drive.as_ref() {
                Some(s) => s,
                None => return PrivStackError::NotInitialized,
            };
            handle.runtime.block_on(storage.list_files())
        }
        CloudProvider::ICloud => {
            let storage = match handle.icloud.as_ref() {
                Some(s) => s,
                None => return PrivStackError::NotInitialized,
            };
            handle.runtime.block_on(storage.list_files())
        }
    };

    match result {
        Ok(files) => {
            let file_infos: Vec<CloudFileInfo> = files
                .into_iter()
                .map(|f| {
                    let modified_at_ms = f
                        .modified_at
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0);
                    CloudFileInfo {
                        id: f.id,
                        name: f.name,
                        path: f.path,
                        size: f.size,
                        modified_at_ms,
                        content_hash: f.content_hash,
                    }
                })
                .collect();

            match serde_json::to_string(&file_infos) {
                Ok(json) => {
                    let c_json = CString::new(json).unwrap();
                    *out_json = c_json.into_raw();
                    PrivStackError::Ok
                }
                Err(_) => PrivStackError::JsonError,
            }
        }
        Err(_) => PrivStackError::CloudError,
    }
}}

/// Uploads a file to cloud storage.
///
/// # Safety
/// - `name` must be a valid null-terminated UTF-8 string.
/// - `data` must be a valid pointer to `data_len` bytes.
/// - `out_json` will receive a pointer to a JSON string with file info.
/// - The returned string must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_cloud_upload(
    provider: CloudProvider,
    name: *const c_char,
    data: *const u8,
    data_len: usize,
    out_json: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if name.is_null() || data.is_null() || out_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let name_str = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let content = std::slice::from_raw_parts(data, data_len);

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let result = match provider {
        CloudProvider::GoogleDrive => {
            let storage = match handle.google_drive.as_ref() {
                Some(s) => s,
                None => return PrivStackError::NotInitialized,
            };
            handle.runtime.block_on(storage.upload(name_str, content))
        }
        CloudProvider::ICloud => {
            let storage = match handle.icloud.as_ref() {
                Some(s) => s,
                None => return PrivStackError::NotInitialized,
            };
            handle.runtime.block_on(storage.upload(name_str, content))
        }
    };

    match result {
        Ok(f) => {
            let modified_at_ms = f
                .modified_at
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let file_info = CloudFileInfo {
                id: f.id,
                name: f.name,
                path: f.path,
                size: f.size,
                modified_at_ms,
                content_hash: f.content_hash,
            };

            match serde_json::to_string(&file_info) {
                Ok(json) => {
                    let c_json = CString::new(json).unwrap();
                    *out_json = c_json.into_raw();
                    PrivStackError::Ok
                }
                Err(_) => PrivStackError::JsonError,
            }
        }
        Err(_) => PrivStackError::CloudError,
    }
}}

/// Downloads a file from cloud storage.
///
/// # Safety
/// - `file_id` must be a valid null-terminated UTF-8 string.
/// - `out_data` will receive a pointer to the file data.
/// - `out_len` will receive the data length.
/// - The returned data must be freed with `privstack_free_bytes`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_cloud_download(
    provider: CloudProvider,
    file_id: *const c_char,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> PrivStackError { unsafe {
    if file_id.is_null() || out_data.is_null() || out_len.is_null() {
        return PrivStackError::NullPointer;
    }

    let file_id_str = match CStr::from_ptr(file_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let result = match provider {
        CloudProvider::GoogleDrive => {
            let storage = match handle.google_drive.as_ref() {
                Some(s) => s,
                None => return PrivStackError::NotInitialized,
            };
            handle.runtime.block_on(storage.download(file_id_str))
        }
        CloudProvider::ICloud => {
            let storage = match handle.icloud.as_ref() {
                Some(s) => s,
                None => return PrivStackError::NotInitialized,
            };
            handle.runtime.block_on(storage.download(file_id_str))
        }
    };

    match result {
        Ok(data) => {
            let len = data.len();
            let ptr = Box::into_raw(data.into_boxed_slice()) as *mut u8;
            *out_data = ptr;
            *out_len = len;
            PrivStackError::Ok
        }
        Err(_) => PrivStackError::CloudError,
    }
}}

/// Deletes a file from cloud storage.
///
/// # Safety
/// - `file_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_cloud_delete(
    provider: CloudProvider,
    file_id: *const c_char,
) -> PrivStackError { unsafe {
    if file_id.is_null() {
        return PrivStackError::NullPointer;
    }

    let file_id_str = match CStr::from_ptr(file_id).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let result = match provider {
        CloudProvider::GoogleDrive => {
            let storage = match handle.google_drive.as_ref() {
                Some(s) => s,
                None => return PrivStackError::NotInitialized,
            };
            handle.runtime.block_on(storage.delete(file_id_str))
        }
        CloudProvider::ICloud => {
            let storage = match handle.icloud.as_ref() {
                Some(s) => s,
                None => return PrivStackError::NotInitialized,
            };
            handle.runtime.block_on(storage.delete(file_id_str))
        }
    };

    match result {
        Ok(_) => PrivStackError::Ok,
        Err(_) => PrivStackError::CloudError,
    }
}}

/// Gets the name of a cloud provider.
///
/// # Safety
/// - The returned string is statically allocated and must not be freed.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_cloud_provider_name(provider: CloudProvider) -> *const c_char {
    match provider {
        CloudProvider::GoogleDrive => b"Google Drive\0".as_ptr() as *const c_char,
        CloudProvider::ICloud => b"iCloud Drive\0".as_ptr() as *const c_char,
    }
}

// ============================================================================
// License Functions
// ============================================================================

/// License plan enum for FFI.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiLicensePlan {
    Monthly = 0,
    Annual = 1,
    Perpetual = 2,
    Trial = 3,
}

impl From<LicensePlan> for FfiLicensePlan {
    fn from(lp: LicensePlan) -> Self {
        match lp {
            LicensePlan::Trial => FfiLicensePlan::Trial,
            LicensePlan::Monthly => FfiLicensePlan::Monthly,
            LicensePlan::Annual => FfiLicensePlan::Annual,
            LicensePlan::Perpetual => FfiLicensePlan::Perpetual,
        }
    }
}

/// License status enum for FFI.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiLicenseStatus {
    Active = 0,
    Expired = 1,
    Grace = 2,
    ReadOnly = 3,
    NotActivated = 4,
}

impl From<LicenseStatus> for FfiLicenseStatus {
    fn from(ls: LicenseStatus) -> Self {
        match ls {
            LicenseStatus::Active => FfiLicenseStatus::Active,
            LicenseStatus::Expired => FfiLicenseStatus::Expired,
            LicenseStatus::Grace { .. } => FfiLicenseStatus::Grace,
            LicenseStatus::ReadOnly => FfiLicenseStatus::ReadOnly,
            LicenseStatus::NotActivated => FfiLicenseStatus::NotActivated,
        }
    }
}

#[derive(Serialize)]
pub struct LicenseInfo {
    pub raw: String,
    pub plan: String,
    pub email: String,
    pub sub: i64,
    pub status: String,
    pub issued_at_ms: i64,
    pub expires_at_ms: Option<i64>,
    pub grace_days_remaining: Option<u32>,
}

#[derive(Serialize)]
pub struct ActivationInfo {
    pub license_key: String,
    pub plan: String,
    pub email: String,
    pub sub: i64,
    pub activated_at_ms: i64,
    pub expires_at_ms: Option<i64>,
    pub device_fingerprint: String,
    pub status: String,
    pub is_valid: bool,
    pub grace_days_remaining: Option<u32>,
}

#[derive(Serialize)]
pub struct FfiDeviceInfo {
    pub os_name: String,
    pub os_version: String,
    pub hostname: String,
    pub arch: String,
    pub fingerprint: String,
}

pub fn license_error_to_ffi(err: LicenseError) -> PrivStackError {
    match err {
        LicenseError::InvalidKeyFormat(_) => PrivStackError::LicenseInvalidFormat,
        LicenseError::InvalidSignature => PrivStackError::LicenseInvalidSignature,
        LicenseError::InvalidPayload(_) => PrivStackError::LicenseInvalidFormat,
        LicenseError::Expired(_) => PrivStackError::LicenseExpired,
        LicenseError::NotActivated => PrivStackError::LicenseNotActivated,
        LicenseError::ActivationFailed(_) => PrivStackError::LicenseActivationFailed,
        LicenseError::DeviceLimitExceeded(_) => PrivStackError::LicenseActivationFailed,
        LicenseError::Revoked => PrivStackError::LicenseExpired,
        LicenseError::Network(_) => PrivStackError::SyncError,
        LicenseError::Storage(_) => PrivStackError::StorageError,
        LicenseError::Serialization(_) => PrivStackError::JsonError,
    }
}

/// Parses and validates a license key.
///
/// # Safety
/// - `key` must be a valid null-terminated UTF-8 string.
/// - `out_json` will receive a pointer to a JSON string with license info.
/// - The returned string must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_license_parse(
    key: *const c_char,
    out_json: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if key.is_null() || out_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let key_str = match CStr::from_ptr(key).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let parsed = match LicenseKey::parse(key_str) {
        Ok(k) => k,
        Err(e) => return license_error_to_ffi(e),
    };

    let status = parsed.status();
    let grace_days = match &status {
        LicenseStatus::Grace { days_remaining } => Some(*days_remaining),
        _ => None,
    };
    let info = LicenseInfo {
        raw: parsed.raw().to_string(),
        plan: format!("{:?}", parsed.license_plan()).to_lowercase(),
        email: parsed.payload().email.clone(),
        sub: parsed.payload().sub,
        status: format!("{:?}", status).to_lowercase(),
        issued_at_ms: parsed.issued_at_secs() * 1000,
        expires_at_ms: parsed.expires_at_secs().map(|s| s * 1000),
        grace_days_remaining: grace_days,
    };

    match serde_json::to_string(&info) {
        Ok(json) => {
            let c_json = CString::new(json).unwrap();
            *out_json = c_json.into_raw();
            PrivStackError::Ok
        }
        Err(_) => PrivStackError::JsonError,
    }
}}

/// Gets the license plan from a parsed key.
///
/// # Safety
/// - `key` must be a valid null-terminated UTF-8 string.
/// - `out_plan` will receive the license plan.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_license_get_plan(
    key: *const c_char,
    out_plan: *mut FfiLicensePlan,
) -> PrivStackError { unsafe {
    if key.is_null() || out_plan.is_null() {
        return PrivStackError::NullPointer;
    }

    let key_str = match CStr::from_ptr(key).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let parsed = match LicenseKey::parse(key_str) {
        Ok(k) => k,
        Err(e) => return license_error_to_ffi(e),
    };

    *out_plan = parsed.license_plan().into();
    PrivStackError::Ok
}}

/// Gets device information including fingerprint.
///
/// # Safety
/// - `out_json` will receive a pointer to a JSON string with device info.
/// - The returned string must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_device_info(out_json: *mut *mut c_char) -> PrivStackError { unsafe {
    if out_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let device_info = DeviceInfo::collect();
    let fingerprint = DeviceFingerprint::generate();

    let info = FfiDeviceInfo {
        os_name: device_info.os_name,
        os_version: device_info.os_version,
        hostname: device_info.hostname,
        arch: device_info.arch,
        fingerprint: fingerprint.id().to_string(),
    };

    match serde_json::to_string(&info) {
        Ok(json) => {
            let c_json = CString::new(json).unwrap();
            *out_json = c_json.into_raw();
            PrivStackError::Ok
        }
        Err(_) => PrivStackError::JsonError,
    }
}}

/// Generates and returns the device fingerprint.
///
/// # Safety
/// - `out_fingerprint` will receive a pointer to the fingerprint string.
/// - The returned string must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_device_fingerprint(
    out_fingerprint: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if out_fingerprint.is_null() {
        return PrivStackError::NullPointer;
    }

    let fingerprint = DeviceFingerprint::generate();
    let c_str = CString::new(fingerprint.id().to_string()).unwrap();
    *out_fingerprint = c_str.into_raw();

    PrivStackError::Ok
}}

/// Activates a license key (offline activation).
///
/// # Safety
/// - `key` must be a valid null-terminated UTF-8 string.
/// - `out_json` will receive a pointer to a JSON string with activation info.
/// - The returned string must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_license_activate(
    key: *const c_char,
    out_json: *mut *mut c_char,
) -> PrivStackError { unsafe {
    if key.is_null() || out_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let key_str = match CStr::from_ptr(key).to_str() {
        Ok(s) => s,
        Err(_) => return PrivStackError::InvalidUtf8,
    };

    let parsed = match LicenseKey::parse(key_str) {
        Ok(k) => k,
        Err(e) => return license_error_to_ffi(e),
    };

    let fingerprint = DeviceFingerprint::generate();
    let token = format!("offline-{}-{}", parsed.raw(), fingerprint.id());
    let activation = Activation::new(&parsed, fingerprint, token);

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    if let Err(e) = handle.activation_store.save(&activation) {
        return license_error_to_ffi(e);
    }

    let status = activation.status();
    let grace_days = match &status {
        LicenseStatus::Grace { days_remaining } => Some(*days_remaining),
        _ => None,
    };
    let info = ActivationInfo {
        license_key: activation.license_key().to_string(),
        plan: format!("{:?}", activation.license_plan()).to_lowercase(),
        email: activation.email().to_string(),
        sub: activation.sub(),
        activated_at_ms: activation.activated_at().timestamp_millis(),
        expires_at_ms: None,
        device_fingerprint: activation.device_fingerprint().id().to_string(),
        status: format!("{:?}", status).to_lowercase(),
        is_valid: activation.is_valid(),
        grace_days_remaining: grace_days,
    };

    match serde_json::to_string(&info) {
        Ok(json) => {
            let c_json = CString::new(json).unwrap();
            *out_json = c_json.into_raw();
            PrivStackError::Ok
        }
        Err(_) => PrivStackError::JsonError,
    }
}}

/// Checks if a valid license is activated.
///
/// # Safety
/// - `out_json` will receive a pointer to a JSON string with activation info (or null if not activated).
/// - The returned string must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_license_check(out_json: *mut *mut c_char) -> PrivStackError { unsafe {
    if out_json.is_null() {
        return PrivStackError::NullPointer;
    }

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match handle.activation_store.load() {
        Ok(Some(activation)) => {
            let status = activation.status();
            let grace_days = match &status {
                LicenseStatus::Grace { days_remaining } => Some(*days_remaining),
                _ => None,
            };
            let info = ActivationInfo {
                license_key: activation.license_key().to_string(),
                plan: format!("{:?}", activation.license_plan()).to_lowercase(),
                email: activation.email().to_string(),
                sub: activation.sub(),
                activated_at_ms: activation.activated_at().timestamp_millis(),
                expires_at_ms: None,
                device_fingerprint: activation.device_fingerprint().id().to_string(),
                status: format!("{:?}", status).to_lowercase(),
                is_valid: activation.is_valid(),
                grace_days_remaining: grace_days,
            };

            match serde_json::to_string(&info) {
                Ok(json) => {
                    let c_json = CString::new(json).unwrap();
                    *out_json = c_json.into_raw();
                    PrivStackError::Ok
                }
                Err(_) => PrivStackError::JsonError,
            }
        }
        Ok(None) => {
            *out_json = std::ptr::null_mut();
            PrivStackError::LicenseNotActivated
        }
        Err(e) => license_error_to_ffi(e),
    }
}}

/// Checks if the license is valid and usable.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_license_is_valid() -> bool {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return false,
    };

    match handle.activation_store.load() {
        Ok(Some(activation)) => activation.is_valid(),
        _ => false,
    }
}

/// Gets the current license status.
///
/// # Safety
/// - `out_status` will receive the license status.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_license_status(
    out_status: *mut FfiLicenseStatus,
) -> PrivStackError { unsafe {
    if out_status.is_null() {
        return PrivStackError::NullPointer;
    }

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match handle.activation_store.load() {
        Ok(Some(activation)) => {
            *out_status = activation.status().into();
            PrivStackError::Ok
        }
        Ok(None) => {
            *out_status = FfiLicenseStatus::NotActivated;
            PrivStackError::Ok
        }
        Err(e) => license_error_to_ffi(e),
    }
}}

/// Gets the activated license plan.
///
/// # Safety
/// - `out_plan` will receive the license plan.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_license_activated_plan(
    out_plan: *mut FfiLicensePlan,
) -> PrivStackError { unsafe {
    if out_plan.is_null() {
        return PrivStackError::NullPointer;
    }

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match handle.activation_store.load() {
        Ok(Some(activation)) => {
            *out_plan = activation.license_plan().into();
            PrivStackError::Ok
        }
        Ok(None) => PrivStackError::LicenseNotActivated,
        Err(e) => license_error_to_ffi(e),
    }
}}

/// Deactivates the current license.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_license_deactivate() -> PrivStackError {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    match handle.activation_store.clear() {
        Ok(_) => PrivStackError::Ok,
        Err(e) => license_error_to_ffi(e),
    }
}

/// Returns the maximum number of devices for a license plan.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_license_max_devices(plan: FfiLicensePlan) -> u32 {
    match plan {
        FfiLicensePlan::Trial => 1,
        FfiLicensePlan::Monthly => 3,
        FfiLicensePlan::Annual => 5,
        FfiLicensePlan::Perpetual => 5,
    }
}

/// Returns whether a license plan includes priority support.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_license_has_priority_support(plan: FfiLicensePlan) -> bool {
    matches!(plan, FfiLicensePlan::Annual | FfiLicensePlan::Perpetual)
}

// ============================================================================
// Generic SDK Execute Endpoint
// ============================================================================

/// Request structure for the generic execute endpoint.
#[derive(Deserialize)]
pub struct SdkRequest {
    #[allow(dead_code)]
    pub plugin_id: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub payload: Option<String>,
    #[allow(dead_code)]
    pub parameters: Option<std::collections::HashMap<String, String>>,
}

/// Response structure for the generic execute endpoint.
#[derive(Serialize)]
pub struct SdkResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl SdkResponse {
    pub fn ok(data: serde_json::Value) -> Self {
        SdkResponse { success: true, error_code: None, error_message: None, data: Some(data) }
    }
    pub fn ok_empty() -> Self {
        SdkResponse { success: true, error_code: None, error_message: None, data: None }
    }
    pub fn err(code: &str, message: &str) -> Self {
        SdkResponse {
            success: false,
            error_code: Some(code.to_string()),
            error_message: Some(message.to_string()),
            data: None,
        }
    }
}

/// Generic SDK execute endpoint. Routes (entity_type, action) to the entity store.
///
/// # Safety
/// - `request_json` must be a valid null-terminated UTF-8 string.
/// - The returned pointer must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_execute(request_json: *const c_char) -> *mut c_char { unsafe {
    let response = execute_inner(request_json);
    let json = serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"success":false,"error_code":"json_error","error_message":"Failed to serialize response"}"#.to_string()
    });
    CString::new(json).unwrap_or_default().into_raw()
}}

unsafe fn execute_inner(request_json: *const c_char) -> SdkResponse {
    if request_json.is_null() {
        return SdkResponse::err("null_pointer", "Request JSON is null");
    }

    let json_str = match unsafe { CStr::from_ptr(request_json) }.to_str() {
        Ok(s) => s,
        Err(_) => return SdkResponse::err("invalid_utf8", "Request JSON is not valid UTF-8"),
    };

    let request: SdkRequest = match serde_json::from_str(json_str) {
        Ok(r) => r,
        Err(e) => return SdkResponse::err("json_parse_error", &format!("Failed to parse request: {e}")),
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return SdkResponse::err("not_initialized", "PrivStack runtime not initialized"),
    };

    if handle.entity_registry.has_schema(&request.entity_type) {
        return execute_generic(handle, &request);
    }

    SdkResponse::err("unknown_entity", &format!("No schema registered for entity type: {}. Ensure the plugin registered its EntitySchemas.", request.entity_type))
}

// ========================================================================
// Generic Entity Engine
// ========================================================================

/// Flatten an Entity into a single JSON object by merging the wrapper metadata
/// (id, entity_type, created_at, modified_at, created_by) into the inner `data`.
/// C# plugins expect flat domain objects, not the Entity wrapper.
fn flatten_entity(entity: &Entity) -> serde_json::Value {
    let mut merged = entity.data.clone();
    if let Some(obj) = merged.as_object_mut() {
        obj.insert("id".into(), serde_json::Value::String(entity.id.clone()));
        obj.insert("entity_type".into(), serde_json::Value::String(entity.entity_type.clone()));
        obj.insert("created_at".into(), serde_json::json!(entity.created_at));
        obj.insert("modified_at".into(), serde_json::json!(entity.modified_at));
        obj.insert("created_by".into(), serde_json::Value::String(entity.created_by.clone()));
    }
    merged
}

/// Flatten a list of entities into a JSON array of flat objects.
fn flatten_entities(entities: &[Entity]) -> serde_json::Value {
    serde_json::Value::Array(entities.iter().map(flatten_entity).collect())
}

/// Check if the license allows write operations. Returns `Ok(())` if writable,
/// or an appropriate `PrivStackError` if the license is expired/missing.
/// Fail-open: if the activation file can't be read, writes are allowed.
pub(crate) fn check_license_writable(handle: &PrivStackHandle) -> Result<(), PrivStackError> {
    match handle.activation_store.load() {
        Ok(Some(activation)) if activation.status().is_usable() => Ok(()),
        Ok(Some(_)) => Err(PrivStackError::LicenseExpired),
        Ok(None) => Err(PrivStackError::LicenseNotActivated),
        Err(_) => Ok(()), // fail-open
    }
}

fn execute_generic(handle: &PrivStackHandle, req: &SdkRequest) -> SdkResponse {
    // Block write operations when license is not usable (expired trial, past grace period)
    let is_mutation = matches!(
        req.action.as_str(),
        "create" | "update" | "delete" | "trash" | "restore" | "link" | "unlink"
    );
    if is_mutation {
        match handle.activation_store.load() {
            Ok(Some(activation)) => {
                if !activation.status().is_usable() {
                    return SdkResponse::err(
                        "license_read_only",
                        "Your license has expired. The app is in read-only mode.",
                    );
                }
            }
            Ok(None) => {
                return SdkResponse::err(
                    "license_read_only",
                    "No active license. The app is in read-only mode.",
                );
            }
            Err(_) => {
                // If we can't read activation, allow the operation (fail-open for robustness)
            }
        }
    }

    let schema = match handle.entity_registry.get_schema(&req.entity_type) {
        Some(s) => s,
        None => return SdkResponse::err("unknown_entity", &format!("No schema for: {}", req.entity_type)),
    };
    let handler = handle.entity_registry.get_handler(&req.entity_type);

    match req.action.as_str() {
        "create" | "update" => {
            let payload = match req.payload.as_deref() {
                Some(p) => p,
                None => return SdkResponse::err("missing_payload", "Create/update requires a payload"),
            };
            let data: serde_json::Value = match serde_json::from_str(payload) {
                Ok(v) => v,
                Err(e) => return SdkResponse::err("json_error", &format!("Invalid JSON: {e}")),
            };

            let now = chrono::Utc::now().timestamp_millis();
            let id = req.entity_id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());

            let override_created = req.parameters.as_ref()
                .and_then(|p| p.get("created_at"))
                .and_then(|v| v.parse::<i64>().ok());
            let override_modified = req.parameters.as_ref()
                .and_then(|p| p.get("modified_at"))
                .and_then(|v| v.parse::<i64>().ok());

            let (created_at, created_by) = if req.action == "update" {
                match handle.entity_store.get_entity(&id) {
                    Ok(Some(existing)) => (
                        override_created.unwrap_or(existing.created_at),
                        existing.created_by,
                    ),
                    _ => (override_created.unwrap_or(now), handle.peer_id.to_string()),
                }
            } else {
                (override_created.unwrap_or(now), handle.peer_id.to_string())
            };

            // Inject local_only flag into data if parameter is set
            let mut data = data;
            let is_local_only = req.parameters.as_ref()
                .and_then(|p| p.get("local_only"))
                .map(|v| v == "true")
                .unwrap_or(false);
            if is_local_only {
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("local_only".into(), serde_json::Value::Bool(true));
                }
            }

            let mut entity = Entity {
                id,
                entity_type: req.entity_type.clone(),
                data,
                created_at,
                modified_at: override_modified.unwrap_or(now),
                created_by,
            };

            if let Some(h) = handler {
                if let Err(msg) = h.validate(&entity) {
                    return SdkResponse::err("validation_error", &msg);
                }
            }

            match handle.entity_store.save_entity(&entity, schema) {
                Ok(_) => {
                    if let Some(h) = handler {
                        h.on_after_load(&mut entity);
                    }
                    SdkResponse::ok(flatten_entity(&entity))
                }
                Err(e) => SdkResponse::err("storage_error", &format!("Failed to save: {e}")),
            }
        }
        "read" => {
            let id = match &req.entity_id {
                Some(id) => id,
                None => return SdkResponse::err("missing_id", "Read requires entity_id"),
            };
            match handle.entity_store.get_entity(id) {
                Ok(Some(mut entity)) => {
                    if let Some(h) = handler {
                        h.on_after_load(&mut entity);
                    }
                    SdkResponse::ok(flatten_entity(&entity))
                }
                Ok(None) => SdkResponse::err("not_found", &format!("Entity not found: {id}")),
                Err(e) => SdkResponse::err("storage_error", &format!("Read failed: {e}")),
            }
        }
        "count" => {
            let include_trashed = req.parameters.as_ref()
                .and_then(|p| p.get("include_trashed"))
                .map(|v| v == "true")
                .unwrap_or(false);

            match handle.entity_store.count_entities(&req.entity_type, include_trashed) {
                Ok(count) => SdkResponse::ok(serde_json::json!({"count": count})),
                Err(e) => SdkResponse::err("storage_error", &format!("Count failed: {e}")),
            }
        }
        "read_list" => {
            let limit = req.parameters.as_ref()
                .and_then(|p| p.get("limit"))
                .and_then(|v| v.parse().ok());
            let offset = req.parameters.as_ref()
                .and_then(|p| p.get("offset"))
                .and_then(|v| v.parse().ok());
            let include_trashed = req.parameters.as_ref()
                .and_then(|p| p.get("include_trashed"))
                .map(|v| v == "true")
                .unwrap_or(false);

            match handle.entity_store.list_entities(&req.entity_type, include_trashed, limit, offset) {
                Ok(mut entities) => {
                    if let Some(h) = handler {
                        for entity in &mut entities {
                            h.on_after_load(entity);
                        }
                    }
                    SdkResponse::ok(flatten_entities(&entities))
                }
                Err(e) => SdkResponse::err("storage_error", &format!("List failed: {e}")),
            }
        }
        "delete" => {
            let id = match &req.entity_id {
                Some(id) => id,
                None => return SdkResponse::err("missing_id", "Delete requires entity_id"),
            };
            match handle.entity_store.delete_entity(id) {
                Ok(_) => SdkResponse::ok_empty(),
                Err(e) => SdkResponse::err("storage_error", &format!("Delete failed: {e}")),
            }
        }
        "trash" => {
            let id = match &req.entity_id {
                Some(id) => id,
                None => return SdkResponse::err("missing_id", "Trash requires entity_id"),
            };
            match handle.entity_store.trash_entity(id) {
                Ok(_) => SdkResponse::ok_empty(),
                Err(e) => SdkResponse::err("storage_error", &format!("Trash failed: {e}")),
            }
        }
        "restore" => {
            let id = match &req.entity_id {
                Some(id) => id,
                None => return SdkResponse::err("missing_id", "Restore requires entity_id"),
            };
            match handle.entity_store.restore_entity(id) {
                Ok(_) => SdkResponse::ok_empty(),
                Err(e) => SdkResponse::err("storage_error", &format!("Restore failed: {e}")),
            }
        }
        "query" => {
            let mut filters: Vec<(String, serde_json::Value)> = req.payload.as_deref()
                .and_then(|p| serde_json::from_str(p).ok())
                .unwrap_or_default();
            let limit = req.parameters.as_ref()
                .and_then(|p| p.get("limit"))
                .and_then(|v| v.parse().ok());
            let include_trashed = req.parameters.as_ref()
                .and_then(|p| p.get("include_trashed"))
                .map(|v| v == "true")
                .unwrap_or(false);

            // Also extract field-level filters from parameters (excluding reserved keys)
            const RESERVED: &[&str] = &["limit", "offset", "query", "search", "action", "include_trashed"];
            if let Some(params) = &req.parameters {
                for (k, v) in params {
                    if !RESERVED.contains(&k.as_str()) {
                        filters.push((k.clone(), serde_json::Value::String(v.clone())));
                    }
                }
            }

            match handle.entity_store.query_entities(&req.entity_type, &filters, include_trashed, limit) {
                Ok(mut entities) => {
                    if let Some(h) = handler {
                        for entity in &mut entities {
                            h.on_after_load(entity);
                        }
                    }
                    SdkResponse::ok(flatten_entities(&entities))
                }
                Err(e) => SdkResponse::err("storage_error", &format!("Query failed: {e}")),
            }
        }
        "link" => {
            let target_type = req.parameters.as_ref().and_then(|p| p.get("target_type"));
            let target_id = req.parameters.as_ref().and_then(|p| p.get("target_id"));
            let source_id = match &req.entity_id {
                Some(id) => id,
                None => return SdkResponse::err("missing_id", "Link requires entity_id"),
            };
            match (target_type, target_id) {
                (Some(tt), Some(ti)) => {
                    match handle.entity_store.save_link(&req.entity_type, source_id, tt, ti) {
                        Ok(_) => SdkResponse::ok_empty(),
                        Err(e) => SdkResponse::err("storage_error", &format!("Link failed: {e}")),
                    }
                }
                _ => SdkResponse::err("missing_params", "Link requires target_type and target_id parameters"),
            }
        }
        "unlink" => {
            let target_type = req.parameters.as_ref().and_then(|p| p.get("target_type"));
            let target_id = req.parameters.as_ref().and_then(|p| p.get("target_id"));
            let source_id = match &req.entity_id {
                Some(id) => id,
                None => return SdkResponse::err("missing_id", "Unlink requires entity_id"),
            };
            match (target_type, target_id) {
                (Some(tt), Some(ti)) => {
                    match handle.entity_store.remove_link(&req.entity_type, source_id, tt, ti) {
                        Ok(_) => SdkResponse::ok_empty(),
                        Err(e) => SdkResponse::err("storage_error", &format!("Unlink failed: {e}")),
                    }
                }
                _ => SdkResponse::err("missing_params", "Unlink requires target_type and target_id parameters"),
            }
        }
        "get_links" => {
            let source_id = match &req.entity_id {
                Some(id) => id,
                None => return SdkResponse::err("missing_id", "get_links requires entity_id"),
            };
            match handle.entity_store.get_links_from(&req.entity_type, source_id) {
                Ok(links) => {
                    let link_data: Vec<serde_json::Value> = links.iter().map(|(t, id)| {
                        serde_json::json!({"target_type": t, "target_id": id})
                    }).collect();
                    SdkResponse::ok(serde_json::Value::Array(link_data))
                }
                Err(e) => SdkResponse::err("storage_error", &format!("Get links failed: {e}")),
            }
        }
        "command" => {
            let entity_id = match &req.entity_id {
                Some(id) => id,
                None => return SdkResponse::err("missing_id", "Command requires entity_id"),
            };
            let command = req.parameters.as_ref().and_then(|p| p.get("command"));
            let command = match command {
                Some(c) => c.as_str(),
                None => return SdkResponse::err("missing_params", "Command requires 'command' parameter"),
            };

            match (req.entity_type.as_str(), command) {
                ("contact_group", "add_member") | ("contact_group", "remove_member") => {
                    let contact_id = match req.parameters.as_ref().and_then(|p| p.get("contact_id")) {
                        Some(cid) => cid.clone(),
                        None => return SdkResponse::err("missing_params", "add_member/remove_member requires 'contact_id' parameter"),
                    };

                    let mut entity = match handle.entity_store.get_entity(entity_id) {
                        Ok(Some(e)) => e,
                        Ok(None) => return SdkResponse::err("not_found", &format!("Entity not found: {entity_id}")),
                        Err(e) => return SdkResponse::err("storage_error", &format!("Read failed: {e}")),
                    };

                    // Get or create contact_ids array
                    let contact_ids = entity.data.as_object_mut()
                        .and_then(|obj| {
                            if !obj.contains_key("contact_ids") {
                                obj.insert("contact_ids".into(), serde_json::json!([]));
                            }
                            obj.get_mut("contact_ids")
                        })
                        .and_then(|v| v.as_array_mut());

                    let contact_ids = match contact_ids {
                        Some(arr) => arr,
                        None => return SdkResponse::err("data_error", "Failed to access contact_ids array"),
                    };

                    if command == "add_member" {
                        let already_exists = contact_ids.iter().any(|v| v.as_str() == Some(&contact_id));
                        if !already_exists {
                            contact_ids.push(serde_json::Value::String(contact_id));
                        }
                    } else {
                        contact_ids.retain(|v| v.as_str() != Some(&contact_id));
                    }

                    // Update contact_count
                    let count = contact_ids.len();
                    if let Some(obj) = entity.data.as_object_mut() {
                        obj.insert("contact_count".into(), serde_json::json!(count));
                    }

                    entity.modified_at = chrono::Utc::now().timestamp_millis();

                    match handle.entity_store.save_entity(&entity, schema) {
                        Ok(_) => {
                            let mut result = entity.clone();
                            if let Some(h) = handler {
                                h.on_after_load(&mut result);
                            }
                            SdkResponse::ok(flatten_entity(&result))
                        }
                        Err(e) => SdkResponse::err("storage_error", &format!("Failed to save: {e}")),
                    }
                }
                _ => SdkResponse::err("unknown_command", &format!("Unknown command '{}' for entity type '{}'", command, req.entity_type)),
            }
        }
        other => SdkResponse::err("unknown_action", &format!("Unknown action: {other}")),
    }
}

/// Register an entity type schema at runtime.
///
/// # Safety
/// `schema_json` must be a valid null-terminated UTF-8 JSON string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_register_entity_type(schema_json: *const c_char) -> c_int { unsafe {
    if schema_json.is_null() {
        return -1;
    }

    let json_str = match CStr::from_ptr(schema_json).to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };

    let schema: EntitySchema = match serde_json::from_str(json_str) {
        Ok(s) => s,
        Err(_) => return -3,
    };

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return -4,
    };

    handle.entity_registry.register_schema(schema);
    0
}}

/// Search across all registered entity types.
///
/// # Safety
/// `query_json` must be a valid null-terminated UTF-8 JSON string with fields:
///   `query` (string), `entity_types` (optional string array), `limit` (optional int).
/// The returned pointer must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_search(query_json: *const c_char) -> *mut c_char { unsafe {
    let response = search_inner(query_json);
    let json = serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"success":false,"error_code":"json_error","error_message":"Failed to serialize response"}"#.to_string()
    });
    CString::new(json).unwrap_or_default().into_raw()
}}

unsafe fn search_inner(query_json: *const c_char) -> SdkResponse {
    if query_json.is_null() {
        return SdkResponse::err("null_pointer", "Query JSON is null");
    }

    let json_str = match unsafe { CStr::from_ptr(query_json) }.to_str() {
        Ok(s) => s,
        Err(_) => return SdkResponse::err("invalid_utf8", "Query JSON is not valid UTF-8"),
    };

    #[derive(Deserialize)]
    struct SearchQuery {
        query: String,
        entity_types: Option<Vec<String>>,
        limit: Option<usize>,
    }

    let sq: SearchQuery = match serde_json::from_str(json_str) {
        Ok(q) => q,
        Err(e) => return SdkResponse::err("json_parse_error", &format!("Invalid query: {e}")),
    };

    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return SdkResponse::err("not_initialized", "PrivStack runtime not initialized"),
    };

    let types_refs: Option<Vec<&str>> = sq.entity_types.as_ref().map(|v| v.iter().map(|s| s.as_str()).collect());
    let limit = sq.limit.unwrap_or(50);

    match handle.entity_store.search(&sq.query, types_refs.as_deref(), limit) {
        Ok(entities) => SdkResponse::ok(flatten_entities(&entities)),
        Err(e) => SdkResponse::err("storage_error", &format!("Search failed: {e}")),
    }
}

// ============================================================================
// Plugin Host FFI (requires wasm-plugins feature)
// ============================================================================

#[cfg(feature = "wasm-plugins")]
pub mod plugin_ffi {
use super::*;

/// Loads a Wasm plugin into the plugin host manager.
///
/// # Safety
/// - `metadata_json` must be a valid null-terminated UTF-8 JSON string.
/// - `schemas_json` must be a valid null-terminated UTF-8 JSON array string.
/// - `permissions_json` must be a valid null-terminated UTF-8 JSON string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_load(
    metadata_json: *const c_char,
    schemas_json: *const c_char,
    permissions_json: *const c_char,
) -> PrivStackError { unsafe {
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let metadata_str = match nullable_cstr_to_str(metadata_json) {
        Some(s) => s,
        None => return PrivStackError::NullPointer,
    };
    let schemas_str = match nullable_cstr_to_str(schemas_json) {
        Some(s) => s,
        None => return PrivStackError::NullPointer,
    };
    let permissions_str = match nullable_cstr_to_str(permissions_json) {
        Some(s) => s,
        None => return PrivStackError::NullPointer,
    };

    let metadata: privstack_plugin_host::WitPluginMetadata = match serde_json::from_str(metadata_str) {
        Ok(m) => m,
        Err(_) => return PrivStackError::JsonError,
    };
    let schemas: Vec<privstack_plugin_host::WitEntitySchema> = match serde_json::from_str(schemas_str) {
        Ok(s) => s,
        Err(_) => return PrivStackError::JsonError,
    };
    let permissions: privstack_plugin_host::PermissionSet = match serde_json::from_str(permissions_str) {
        Ok(p) => p,
        Err(_) => return PrivStackError::JsonError,
    };

    let resource_limits = privstack_plugin_host::ResourceLimits::first_party();

    match handle.plugin_host.load_plugin(metadata, schemas, permissions, resource_limits) {
        Ok(()) => PrivStackError::Ok,
        Err(privstack_plugin_host::PluginHostError::PolicyDenied(_)) => PrivStackError::PluginPermissionDenied,
        Err(privstack_plugin_host::PluginHostError::PluginAlreadyLoaded(_)) => PrivStackError::PluginError,
        Err(_) => PrivStackError::PluginError,
    }
}}

/// Unloads a plugin from the host manager.
///
/// # Safety
/// - `plugin_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_unload(plugin_id: *const c_char) -> PrivStackError { unsafe {
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let id = match nullable_cstr_to_str(plugin_id) {
        Some(s) => s,
        None => return PrivStackError::NullPointer,
    };

    match handle.plugin_host.unload_plugin(id) {
        Ok(()) => PrivStackError::Ok,
        Err(privstack_plugin_host::PluginHostError::PluginNotFound(_)) => PrivStackError::PluginNotFound,
        Err(_) => PrivStackError::PluginError,
    }
}}

/// Routes an SDK message to a loaded plugin. Returns a JSON response string.
///
/// # Safety
/// - `plugin_id` must be a valid null-terminated UTF-8 string.
/// - `message_json` must be a valid null-terminated UTF-8 JSON string.
/// - The returned pointer must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_route_sdk(
    plugin_id: *const c_char,
    message_json: *const c_char,
) -> *mut c_char { unsafe {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return to_c_string(r#"{"success":false,"error":"not_initialized","error_code":6}"#),
    };

    let id = match nullable_cstr_to_str(plugin_id) {
        Some(s) => s,
        None => return to_c_string(r#"{"success":false,"error":"null_plugin_id","error_code":1}"#),
    };
    let msg_str = match nullable_cstr_to_str(message_json) {
        Some(s) => s,
        None => return to_c_string(r#"{"success":false,"error":"null_message","error_code":1}"#),
    };

    let message: privstack_plugin_host::WitSdkMessage = match serde_json::from_str(msg_str) {
        Ok(m) => m,
        Err(e) => return to_c_string(&format!(r#"{{"success":false,"error":"invalid_json: {}","error_code":3}}"#, e)),
    };

    match handle.plugin_host.route_sdk_message(id, &message) {
        Ok(resp) => {
            let json = serde_json::to_string(&resp).unwrap_or_else(|_| {
                r#"{"success":false,"error":"serialization_error","error_code":3}"#.to_string()
            });
            to_c_string(&json)
        }
        Err(e) => to_c_string(&format!(r#"{{"success":false,"error":"{}","error_code":23}}"#, e)),
    }
}}

/// Lists all loaded plugins as a JSON array of metadata objects.
///
/// # Safety
/// - The returned pointer must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_plugin_list() -> *mut c_char {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return to_c_string("[]"),
    };

    let plugins = handle.plugin_host.list_plugins();
    let json = serde_json::to_string(&plugins).unwrap_or_else(|_| "[]".to_string());
    to_c_string(&json)
}

/// Returns navigation items for all loaded plugins as JSON array.
///
/// # Safety
/// - The returned pointer must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_plugin_get_nav_items() -> *mut c_char {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return to_c_string("[]"),
    };

    let items = handle.plugin_host.get_navigation_items();
    let json = serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string());
    to_c_string(&json)
}

/// Returns the number of loaded plugins.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_plugin_count() -> c_int {
    let handle = HANDLE.lock().unwrap();
    match handle.as_ref() {
        Some(h) => h.plugin_host.plugin_count() as c_int,
        None => 0,
    }
}

/// Checks if a plugin is loaded.
///
/// # Safety
/// - `plugin_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_is_loaded(plugin_id: *const c_char) -> bool { unsafe {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return false,
    };

    let id = match nullable_cstr_to_str(plugin_id) {
        Some(s) => s,
        None => return false,
    };

    handle.plugin_host.is_loaded(id)
}}

/// Gets resource metrics for a specific plugin as JSON.
///
/// Returns a JSON object with memory, CPU fuel, and disk usage metrics.
///
/// # Safety
/// - `plugin_id` must be a valid null-terminated UTF-8 string.
/// - The returned pointer must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_get_metrics(plugin_id: *const c_char) -> *mut c_char { unsafe {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return to_c_string(r#"{"error":"not initialized"}"#),
    };

    let id = match nullable_cstr_to_str(plugin_id) {
        Some(s) => s,
        None => return to_c_string(r#"{"error":"invalid plugin_id"}"#),
    };

    match handle.plugin_host.get_plugin_metrics(id) {
        Ok(metrics) => {
            let json = serde_json::to_string(&metrics).unwrap_or_else(|e| {
                format!(r#"{{"error":"serialization failed: {}"}}"#, e)
            });
            to_c_string(&json)
        }
        Err(e) => to_c_string(&format!(r#"{{"error":"{}"}}"#, e)),
    }
}}

/// Gets resource metrics for all loaded plugins as JSON.
///
/// Returns a JSON array of objects, each containing plugin_id and metrics.
///
/// # Safety
/// - The returned pointer must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_plugin_get_all_metrics() -> *mut c_char {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return to_c_string("[]"),
    };

    let all_metrics = handle.plugin_host.get_all_plugin_metrics();

    // Convert to a JSON-serializable format
    #[derive(serde::Serialize)]
    struct PluginMetricsEntry {
        plugin_id: String,
        #[serde(flatten)]
        metrics: privstack_plugin_host::PluginResourceMetrics,
    }

    let entries: Vec<PluginMetricsEntry> = all_metrics
        .into_iter()
        .map(|(id, metrics)| PluginMetricsEntry {
            plugin_id: id,
            metrics,
        })
        .collect();

    let json = serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string());
    to_c_string(&json)
}

/// Gets commands from a specific plugin as JSON.
///
/// # Safety
/// - `plugin_id` must be a valid null-terminated UTF-8 string.
/// - The returned pointer must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_get_commands(plugin_id: *const c_char) -> *mut c_char { unsafe {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return to_c_string("[]"),
    };

    let id = match nullable_cstr_to_str(plugin_id) {
        Some(s) => s,
        None => return to_c_string("[]"),
    };

    match handle.plugin_host.get_commands(id) {
        Ok(cmds) => {
            let json = serde_json::to_string(&cmds).unwrap_or_else(|_| "[]".to_string());
            to_c_string(&json)
        }
        Err(_) => to_c_string("[]"),
    }
}}

/// Gets all linkable item providers across loaded plugins as JSON.
///
/// # Safety
/// - The returned pointer must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_plugin_get_link_providers() -> *mut c_char {
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return to_c_string("[]"),
    };

    let providers = handle.plugin_host.get_all_link_providers();
    ffi_debug!("[FFI] get_link_providers returning {} providers", providers.len());
    for p in &providers {
        ffi_debug!("[FFI]   provider: {} -> link_type={}", p.plugin_id, p.link_type);
    }
    let json = serde_json::to_string(&providers).unwrap_or_else(|_| "[]".to_string());
    to_c_string(&json)
}

/// Searches across all loaded plugins for linkable items matching a query.
/// Returns a JSON array of results.
///
/// # Safety
/// - `query` must be a valid null-terminated UTF-8 string.
/// - The returned pointer must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_search_items(
    query: *const c_char,
    max_results: c_int,
) -> *mut c_char { unsafe {
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return to_c_string("[]"),
    };

    let q = match nullable_cstr_to_str(query) {
        Some(s) => s,
        None => return to_c_string("[]"),
    };

    let results = handle.plugin_host.query_all_linkable_items(q, max_results as u32);
    let json = serde_json::to_string(&results).unwrap_or_else(|_| "[]".to_string());
    to_c_string(&json)
}}

/// Navigates to a specific item within a plugin via its deep-link-target export.
///
/// # Safety
/// - `plugin_id` and `item_id` must be valid null-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_navigate_to_item(
    plugin_id: *const c_char,
    item_id: *const c_char,
) -> PrivStackError { unsafe {
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let pid = match nullable_cstr_to_str(plugin_id) {
        Some(s) => s,
        None => return PrivStackError::NullPointer,
    };

    let iid = match nullable_cstr_to_str(item_id) {
        Some(s) => s,
        None => return PrivStackError::NullPointer,
    };

    match handle.plugin_host.navigate_to_item(pid, iid) {
        Ok(()) => PrivStackError::Ok,
        Err(_) => PrivStackError::PluginError,
    }
}}

/// Navigates to a specific item within a plugin and returns its view data.
/// Combines navigate_to_item + get_view_data in a single call for hover prefetch.
///
/// This is safe for cross-plugin prefetch (when the target plugin is not currently
/// displayed). For same-plugin prefetch, use with caution as it changes the plugin's
/// internal state.
///
/// Returns JSON string (caller must free with `privstack_free_string`).
///
/// # Safety
/// - `plugin_id` and `item_id` must be valid null-terminated UTF-8 strings.
/// - The returned pointer must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_get_entity_view_data(
    plugin_id: *const c_char,
    item_id: *const c_char,
) -> *mut c_char { unsafe {
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return to_c_string(r#"{}"#),
    };

    let pid = match nullable_cstr_to_str(plugin_id) {
        Some(s) => s,
        None => return to_c_string(r#"{}"#),
    };

    let iid = match nullable_cstr_to_str(item_id) {
        Some(s) => s,
        None => return to_c_string(r#"{}"#),
    };

    match handle.plugin_host.get_entity_view_data(pid, iid) {
        Ok(json) => to_c_string(&json),
        Err(e) => {
            ffi_error!(
                "[privstack-ffi] get_entity_view_data({}, {}) failed: {:?}",
                pid, iid, e
            );
            to_c_string(r#"{}"#)
        }
    }
}}

// ============================================================
// Plugin Install / Update (P6.4)
// ============================================================

/// Installs a plugin from a .ppk file path. Validates the manifest and loads the Wasm module.
/// Returns Ok on success, PluginError on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_install_ppk(
    ppk_path: *const c_char,
) -> PrivStackError { unsafe {
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let path_str = match nullable_cstr_to_str(ppk_path) {
        Some(s) => s,
        None => return PrivStackError::NullPointer,
    };

    let path = Path::new(path_str);
    if !path.exists() {
        return PrivStackError::NotFound;
    }

    // Read and validate the .ppk file
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return PrivStackError::StorageError,
    };

    let reader = std::io::BufReader::new(file);
    let package = match privstack_ppk::PpkPackage::open(reader) {
        Ok(p) => p,
        Err(_) => return PrivStackError::PluginError,
    };

    if package.manifest.validate().is_err() {
        return PrivStackError::PluginError;
    }

    // Convert PpkManifest to WitPluginMetadata
    let m = &package.manifest;
    let category = match m.category.to_lowercase().as_str() {
        "productivity" => privstack_plugin_host::WitPluginCategory::Productivity,
        "security" => privstack_plugin_host::WitPluginCategory::Security,
        "communication" => privstack_plugin_host::WitPluginCategory::Communication,
        "information" => privstack_plugin_host::WitPluginCategory::Information,
        "extension" => privstack_plugin_host::WitPluginCategory::Extension,
        _ => privstack_plugin_host::WitPluginCategory::Utility,
    };

    let metadata = privstack_plugin_host::WitPluginMetadata {
        id: m.id.clone(),
        name: m.name.clone(),
        description: m.description.clone(),
        version: m.version.clone(),
        author: m.author.clone(),
        icon: m.icon.clone(),
        navigation_order: m.navigation_order,
        category,
        can_disable: m.can_disable,
        is_experimental: m.is_experimental,
    };

    // Convert schemas
    let schemas: Vec<privstack_plugin_host::WitEntitySchema> = m.schemas.iter().map(|s| {
        let merge_strategy = match s.merge_strategy.as_str() {
            "lww_document" => privstack_plugin_host::WitMergeStrategy::LwwDocument,
            "lww_per_field" => privstack_plugin_host::WitMergeStrategy::LwwPerField,
            _ => privstack_plugin_host::WitMergeStrategy::Custom,
        };
        privstack_plugin_host::WitEntitySchema {
            entity_type: s.entity_type.clone(),
            indexed_fields: s.indexed_fields.iter().map(|f| {
                let field_type = match f.field_type.as_str() {
                    "text" => privstack_plugin_host::WitFieldType::Text,
                    "tag" => privstack_plugin_host::WitFieldType::Tag,
                    "date_time" => privstack_plugin_host::WitFieldType::DateTime,
                    "number" => privstack_plugin_host::WitFieldType::Number,
                    "bool" => privstack_plugin_host::WitFieldType::Boolean,
                    "vector" => privstack_plugin_host::WitFieldType::Vector,
                    "counter" => privstack_plugin_host::WitFieldType::Counter,
                    "relation" => privstack_plugin_host::WitFieldType::Relation,
                    "decimal" => privstack_plugin_host::WitFieldType::Decimal,
                    "json" => privstack_plugin_host::WitFieldType::Json,
                    _ => privstack_plugin_host::WitFieldType::Text,
                };
                privstack_plugin_host::WitIndexedField {
                    field_path: f.field_path.clone(),
                    field_type,
                    searchable: f.searchable,
                    vector_dim: None,
                    enum_options: None,
                }
            }).collect(),
            merge_strategy,
        }
    }).collect();

    // Determine resource limits and permissions based on first-party status
    let is_first_party = m.is_first_party();
    let resource_limits = if is_first_party {
        privstack_plugin_host::ResourceLimits::first_party()
    } else {
        privstack_plugin_host::ResourceLimits::third_party()
    };
    let permissions = if is_first_party {
        privstack_plugin_host::PermissionSet::default_first_party()
    } else {
        privstack_plugin_host::PermissionSet::default_third_party()
    };

    match handle.plugin_host.load_plugin(metadata, schemas, permissions, resource_limits) {
        Ok(()) => PrivStackError::Ok,
        Err(_) => PrivStackError::PluginError,
    }
}}

// ============================================================
// Plugin Wasm Runtime (Phase 4: real command routing)
// ============================================================

/// Loads a plugin from a .wasm component file path.
/// Returns the plugin ID via out_plugin_id on success.
///
/// # Safety
/// - `wasm_path` must be a valid null-terminated UTF-8 file path.
/// - `permissions_json` must be a valid null-terminated UTF-8 JSON string.
/// - `out_plugin_id` receives a heap-allocated C string (free with `privstack_free_string`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_load_wasm(
    wasm_path: *const c_char,
    permissions_json: *const c_char,
    out_plugin_id: *mut *mut c_char,
) -> PrivStackError { unsafe {
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let path_str = match nullable_cstr_to_str(wasm_path) {
        Some(s) => s,
        None => return PrivStackError::NullPointer,
    };

    let permissions: privstack_plugin_host::PermissionSet = if let Some(p_str) =
        nullable_cstr_to_str(permissions_json)
    {
        serde_json::from_str(p_str).unwrap_or_else(|_| {
            privstack_plugin_host::PermissionSet::default_first_party()
        })
    } else {
        privstack_plugin_host::PermissionSet::default_first_party()
    };

    let resource_limits = privstack_plugin_host::ResourceLimits::first_party();
    let path = Path::new(path_str);

    match handle
        .plugin_host
        .load_plugin_from_wasm(path, permissions, resource_limits)
    {
        Ok(plugin_id) => {
            if !out_plugin_id.is_null() {
                *out_plugin_id = to_c_string(&plugin_id);
            }
            PrivStackError::Ok
        }
        Err(privstack_plugin_host::PluginHostError::PolicyDenied(_)) => {
            PrivStackError::PluginPermissionDenied
        }
        Err(privstack_plugin_host::PluginHostError::PluginAlreadyLoaded(_)) => {
            PrivStackError::PluginError
        }
        Err(e) => {
            ffi_error!("[privstack-ffi] Failed to load Wasm plugin from {}: {:?}", path_str, e);
            PrivStackError::PluginError
        }
    }
}}

/// Loads multiple Wasm plugins in parallel (compilation is concurrent).
///
/// Input: JSON array `[{"path": "...", "permissions": {...}}, ...]`
/// Output: JSON array `[{"plugin_id": "...", "error": null}, ...]`
///
/// # Safety
/// - `plugins_json` must be a valid null-terminated UTF-8 C string.
/// - The returned pointer must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_load_wasm_batch(
    plugins_json: *const c_char,
) -> *mut c_char { unsafe {
    #[derive(Deserialize)]
    struct BatchEntry {
        path: String,
        #[serde(default)]
        permissions: Option<serde_json::Value>,
    }

    #[derive(Serialize)]
    struct BatchResult {
        plugin_id: Option<String>,
        error: Option<String>,
    }

    let json_str = match nullable_cstr_to_str(plugins_json) {
        Some(s) => s,
        None => return to_c_string("[]"),
    };

    let entries: Vec<BatchEntry> = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            let err = vec![BatchResult {
                plugin_id: None,
                error: Some(format!("invalid batch JSON: {e}")),
            }];
            return to_c_string(&serde_json::to_string(&err).unwrap_or_default());
        }
    };

    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => {
            let results: Vec<BatchResult> = entries
                .iter()
                .map(|_| BatchResult {
                    plugin_id: None,
                    error: Some("not initialized".into()),
                })
                .collect();
            return to_c_string(&serde_json::to_string(&results).unwrap_or_default());
        }
    };

    let tuples: Vec<_> = entries
        .into_iter()
        .map(|e| {
            let perms: privstack_plugin_host::PermissionSet = e
                .permissions
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_else(privstack_plugin_host::PermissionSet::default_first_party);
            let limits = privstack_plugin_host::ResourceLimits::first_party();
            (std::path::PathBuf::from(e.path), perms, limits)
        })
        .collect();

    let results = handle.plugin_host.load_plugins_from_wasm_parallel(tuples);

    let batch_results: Vec<BatchResult> = results
        .into_iter()
        .map(|r| match r {
            Ok(id) => BatchResult {
                plugin_id: Some(id),
                error: None,
            },
            Err(e) => BatchResult {
                plugin_id: None,
                error: Some(format!("{e:?}")),
            },
        })
        .collect();

    to_c_string(&serde_json::to_string(&batch_results).unwrap_or_default())
}}

/// Sends a named command to a plugin's `handle_command()` export.
/// Returns JSON response string (caller must free with `privstack_free_string`).
///
/// # Safety
/// - All string parameters must be valid null-terminated UTF-8.
/// - The returned pointer must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_send_command(
    plugin_id: *const c_char,
    command_name: *const c_char,
    args_json: *const c_char,
) -> *mut c_char { unsafe {
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => {
            return to_c_string(
                r#"{"success":false,"error":"not_initialized","error_code":6}"#,
            )
        }
    };

    let id = match nullable_cstr_to_str(plugin_id) {
        Some(s) => s,
        None => {
            return to_c_string(
                r#"{"success":false,"error":"null_plugin_id","error_code":1}"#,
            )
        }
    };
    let cmd = match nullable_cstr_to_str(command_name) {
        Some(s) => s,
        None => {
            return to_c_string(
                r#"{"success":false,"error":"null_command_name","error_code":1}"#,
            )
        }
    };
    let args = nullable_cstr_to_str(args_json).unwrap_or("{}");

    match handle.plugin_host.send_command(id, cmd, args) {
        Ok(result_json) => to_c_string(&result_json),
        Err(e) => to_c_string(&format!(
            r#"{{"success":false,"error":"{}","error_code":23}}"#,
            e
        )),
    }
}}

/// Fetch a URL on behalf of a plugin, checking its Network permission.
/// Returns the response body bytes. Caller must free with `privstack_free_bytes`.
///
/// # Safety
/// - `plugin_id` and `url` must be valid null-terminated UTF-8.
/// - `out_data` and `out_len` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_fetch_url(
    plugin_id: *const c_char,
    url: *const c_char,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> PrivStackError { unsafe {
    let handle = HANDLE.lock().unwrap();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let id = match nullable_cstr_to_str(plugin_id) {
        Some(s) => s,
        None => return PrivStackError::NullPointer,
    };
    let url_str = match nullable_cstr_to_str(url) {
        Some(s) => s,
        None => return PrivStackError::NullPointer,
    };

    match handle.plugin_host.fetch_url_for_plugin(id, url_str) {
        Ok(bytes) => {
            let len = bytes.len();
            let ptr = if len > 0 {
                let layout = std::alloc::Layout::from_size_align(len, 1).unwrap();
                let p = std::alloc::alloc(layout);
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, len);
                p
            } else {
                std::ptr::null_mut()
            };
            *out_data = ptr;
            *out_len = len;
            PrivStackError::Ok
        }
        Err(e) => {
            *out_data = std::ptr::null_mut();
            *out_len = 0;
            ffi_error!("[privstack_ffi] plugin fetch_url failed: plugin={id} url={url_str} error={e}");
            PrivStackError::PluginPermissionDenied
        }
    }
}}

/// Gets the view state JSON from a plugin's `get_view_state()` export.
/// Returns JSON string (caller must free with `privstack_free_string`).
///
/// # Safety
/// - `plugin_id` must be a valid null-terminated UTF-8 string.
/// - The returned pointer must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_get_view_state(
    plugin_id: *const c_char,
) -> *mut c_char { unsafe {
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => {
            return to_c_string(r#"{"components":{"type":"error","message":"Core not initialized"}}"#);
        }
    };

    let id = match nullable_cstr_to_str(plugin_id) {
        Some(s) => s,
        None => {
            return to_c_string(r#"{"components":{"type":"error","message":"Invalid plugin ID (null)"}}"#);
        }
    };

    match handle.plugin_host.get_view_state(id) {
        Ok(json) => to_c_string(&json),
        Err(e) => {
            ffi_error!("[privstack-ffi] get_view_state({}) failed: {:?}", id, e);
            let msg = format!(
                r#"{{"components":{{"type":"error","message":"get_view_state failed: {}"}}}}"#,
                e.to_string().replace('"', "'")
            );
            to_c_string(&msg)
        }
    }
}}

/// Gets the raw view data JSON from a plugin's `get_view_data()` export.
/// Used for host-side template evaluation. Returns JSON string (caller must
/// free with `privstack_free_string`).
///
/// # Safety
/// - `plugin_id` must be a valid null-terminated UTF-8 string.
/// - The returned pointer must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_get_view_data(
    plugin_id: *const c_char,
) -> *mut c_char { unsafe {
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => {
            return to_c_string(r#"{}"#);
        }
    };

    let id = match nullable_cstr_to_str(plugin_id) {
        Some(s) => s,
        None => {
            return to_c_string(r#"{}"#);
        }
    };

    match handle.plugin_host.get_view_data(id) {
        Ok(json) => to_c_string(&json),
        Err(e) => {
            ffi_error!("[privstack-ffi] get_view_data({}) failed: {:?}", id, e);
            to_c_string(r#"{}"#)
        }
    }
}}

/// Activates a plugin — calls its `activate()` export.
/// Must be called after loading and entity type registration.
///
/// # Safety
/// - `plugin_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_activate(
    plugin_id: *const c_char,
) -> PrivStackError { unsafe {
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let id = match nullable_cstr_to_str(plugin_id) {
        Some(s) => s,
        None => return PrivStackError::NullPointer,
    };

    match handle.plugin_host.activate_plugin(id) {
        Ok(()) => PrivStackError::Ok,
        Err(_) => PrivStackError::PluginError,
    }
}}

/// Notifies a plugin that the user navigated to its view.
///
/// # Safety
/// - `plugin_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_navigated_to(
    plugin_id: *const c_char,
) -> PrivStackError { unsafe {
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let id = match nullable_cstr_to_str(plugin_id) {
        Some(s) => s,
        None => return PrivStackError::NullPointer,
    };

    match handle.plugin_host.notify_navigated_to(id) {
        Ok(()) => PrivStackError::Ok,
        Err(_) => PrivStackError::PluginError,
    }
}}

/// Notifies a plugin that the user navigated away from its view.
///
/// # Safety
/// - `plugin_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_navigated_from(
    plugin_id: *const c_char,
) -> PrivStackError { unsafe {
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let id = match nullable_cstr_to_str(plugin_id) {
        Some(s) => s,
        None => return PrivStackError::NullPointer,
    };

    match handle.plugin_host.notify_navigated_from(id) {
        Ok(()) => PrivStackError::Ok,
        Err(_) => PrivStackError::PluginError,
    }
}}

/// Updates the permission set for a loaded plugin at runtime.
///
/// # Safety
/// - `plugin_id` must be a valid null-terminated UTF-8 string.
/// - `permissions_json` must be a valid null-terminated UTF-8 JSON string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_plugin_update_permissions(
    plugin_id: *const c_char,
    permissions_json: *const c_char,
) -> PrivStackError { unsafe {
    let mut handle = HANDLE.lock().unwrap();
    let handle = match handle.as_mut() {
        Some(h) => h,
        None => return PrivStackError::NotInitialized,
    };

    let id = match nullable_cstr_to_str(plugin_id) {
        Some(s) => s,
        None => return PrivStackError::NullPointer,
    };

    let perms_str = match nullable_cstr_to_str(permissions_json) {
        Some(s) => s,
        None => return PrivStackError::NullPointer,
    };

    let permissions: privstack_plugin_host::PermissionSet = match serde_json::from_str(perms_str) {
        Ok(p) => p,
        Err(_) => return PrivStackError::JsonError,
    };

    match handle.plugin_host.update_plugin_permissions(id, permissions) {
        Ok(()) => PrivStackError::Ok,
        Err(privstack_plugin_host::PluginHostError::PluginNotFound(_)) => PrivStackError::PluginNotFound,
        Err(_) => PrivStackError::PluginError,
    }
}}

} // mod plugin_ffi (cfg wasm-plugins)

/// Returns JSON metadata for a .ppk file without installing it.
/// Caller must free the returned string with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_ppk_inspect(
    ppk_path: *const c_char,
) -> *mut c_char { unsafe {
    let path_str = match nullable_cstr_to_str(ppk_path) {
        Some(s) => s,
        None => return to_c_string("{}"),
    };

    let path = Path::new(path_str);
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return to_c_string("{}"),
    };

    let reader = std::io::BufReader::new(file);
    let package = match privstack_ppk::PpkPackage::open(reader) {
        Ok(p) => p,
        Err(_) => return to_c_string("{}"),
    };

    let json = serde_json::to_string(&package.manifest).unwrap_or_else(|_| "{}".to_string());
    to_c_string(&json)
}}

/// Returns the content hash of a .ppk file for integrity verification.
/// Caller must free the returned string with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_ppk_content_hash(
    ppk_path: *const c_char,
) -> *mut c_char { unsafe {
    let path_str = match nullable_cstr_to_str(ppk_path) {
        Some(s) => s,
        None => return to_c_string(""),
    };

    let path = Path::new(path_str);
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return to_c_string(""),
    };

    let reader = std::io::BufReader::new(file);
    let package = match privstack_ppk::PpkPackage::open(reader) {
        Ok(p) => p,
        Err(_) => return to_c_string(""),
    };

    to_c_string(&package.content_hash())
}}

// ========================================================================
// Database Maintenance
// ========================================================================

/// Runs database maintenance (orphan cleanup + checkpoint) to reclaim space.
/// Note: SQLite VACUUM does NOT reclaim space — CHECKPOINT is the correct approach.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_db_maintenance() -> PrivStackError {
    let handle = HANDLE.lock().unwrap();
    match handle.as_ref() {
        Some(h) => match h.entity_store.run_maintenance() {
            Ok(_) => PrivStackError::Ok,
            Err(_) => PrivStackError::StorageError,
        },
        None => PrivStackError::NotInitialized,
    }
}

/// Finds orphan entities not matching any known (plugin_id, entity_type) pair.
/// `valid_types_json` is a JSON array of {"plugin_id": "...", "entity_type": "..."} objects.
/// Returns JSON array of orphan summaries. Caller must free with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_find_orphan_entities(
    valid_types_json: *const c_char,
) -> *mut c_char { unsafe {
    let json_str = match nullable_cstr_to_str(valid_types_json) {
        Some(s) => s,
        None => return to_c_string("[]"),
    };

    let valid_types: Vec<(String, String)> = match serde_json::from_str::<Vec<serde_json::Value>>(json_str) {
        Ok(arr) => arr.iter().filter_map(|v| {
            let pid = v.get("plugin_id")?.as_str()?.to_string();
            let etype = v.get("entity_type")?.as_str()?.to_string();
            Some((pid, etype))
        }).collect(),
        Err(_) => return to_c_string("[]"),
    };

    let handle = HANDLE.lock().unwrap();
    match handle.as_ref() {
        Some(h) => match h.entity_store.find_orphan_entities(&valid_types) {
            Ok(orphans) => to_c_string(&serde_json::json!(orphans).to_string()),
            Err(_) => to_c_string("[]"),
        },
        None => to_c_string("[]"),
    }
}}

/// Deletes orphan entities not matching any known (plugin_id, entity_type) pair.
/// Cascades to auxiliary tables. Returns deleted count as JSON. Caller must free.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_delete_orphan_entities(
    valid_types_json: *const c_char,
) -> *mut c_char { unsafe {
    let json_str = match nullable_cstr_to_str(valid_types_json) {
        Some(s) => s,
        None => return to_c_string("{\"deleted\":0}"),
    };

    let valid_types: Vec<(String, String)> = match serde_json::from_str::<Vec<serde_json::Value>>(json_str) {
        Ok(arr) => arr.iter().filter_map(|v| {
            let pid = v.get("plugin_id")?.as_str()?.to_string();
            let etype = v.get("entity_type")?.as_str()?.to_string();
            Some((pid, etype))
        }).collect(),
        Err(_) => return to_c_string("{\"deleted\":0}"),
    };

    let handle = HANDLE.lock().unwrap();
    match handle.as_ref() {
        Some(h) => {
            // Delete orphans then checkpoint to reclaim space
            let count = h.entity_store.delete_orphan_entities(&valid_types).unwrap_or(0);
            if count > 0 {
                let _ = h.entity_store.run_maintenance();
            }
            to_c_string(&serde_json::json!({"deleted": count}).to_string())
        }
        None => to_c_string("{\"deleted\":0}"),
    }
}}

/// Returns diagnostics for ALL SQLite files as JSON. Caller must free with `privstack_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_db_diagnostics() -> *mut c_char {
    use privstack_storage::scan_db_file;

    let handle = HANDLE.lock().unwrap();
    match handle.as_ref() {
        Some(h) => {
            let mut all_dbs = serde_json::Map::new();

            // 1. Main database (shared connection for entities, events, blobs, vault)
            if let Ok(diag) = h.entity_store.db_diagnostics() {
                // Add file_size from disk
                let main_path = Path::new(&h.db_path).with_extension("privstack.db");
                let file_size = std::fs::metadata(&main_path)
                    .map(|m| m.len() as i64)
                    .unwrap_or(0);
                let mut val = diag;
                if let Some(obj) = val.as_object_mut() {
                    obj.insert("file_size".to_string(), serde_json::json!(file_size));
                }
                all_dbs.insert("main".to_string(), val);
            }

            // 2. Scan sibling SQLite files (opens read-only connections)
            let base = Path::new(&h.db_path);
            let siblings = [
                ("datasets", base.with_extension("datasets.db")),
            ];

            for (label, path) in &siblings {
                if let Some(diag) = scan_db_file(&path) {
                    all_dbs.insert(label.to_string(), diag);
                }
            }

            to_c_string(&serde_json::Value::Object(all_dbs).to_string())
        }
        None => to_c_string("{}"),
    }
}

/// Compacts all SQLite databases by copying data to fresh files (reclaims allocated-but-empty blocks).
/// Returns JSON with per-database before/after sizes. Caller must free with `privstack_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn privstack_compact_databases() -> *mut c_char {
    use privstack_storage::compact_db_file;

    let handle = HANDLE.lock().unwrap();
    match handle.as_ref() {
        Some(h) => {
            let mut results = serde_json::Map::new();
            let base = Path::new(&h.db_path);

            // 1. Main database (shared connection) — compact covers entities, events, blobs, vault
            let main_path = base.with_extension("privstack.db");
            match h.entity_store.compact(&main_path) {
                Ok((before, after)) => {
                    results.insert("main".into(), serde_json::json!({
                        "before": before, "after": after,
                    }));
                }
                Err(e) => {
                    ffi_error!("[compact] main database failed: {}", e);
                    results.insert("main".into(), serde_json::json!({
                        "error": format!("{}", e),
                    }));
                }
            }

            // 2. Sibling databases — standalone compact (open, copy, close, swap)
            let siblings = [
                ("datasets", base.with_extension("datasets.db")),
            ];

            for (label, path) in &siblings {
                if path.exists() {
                    match compact_db_file(path) {
                        Some((before, after)) => {
                            results.insert((*label).into(), serde_json::json!({
                                "before": before, "after": after,
                            }));
                        }
                        None => {
                            ffi_error!("[compact] {} failed", label);
                            results.insert((*label).into(), serde_json::json!({
                                "error": "compact failed",
                            }));
                        }
                    }
                }
            }

            // Note: vault, blobs, events, and entities all share the main database connection,
            // so the single compact above covers all of them.

            to_c_string(&serde_json::Value::Object(results).to_string())
        }
        None => to_c_string("{}"),
    }
}

/// Helper: allocate a C string from a Rust &str. Caller must free with `privstack_free_string`.
pub fn to_c_string(s: &str) -> *mut c_char {
    CString::new(s).unwrap_or_default().into_raw()
}

/// Helper: convert a nullable C string to a &str, returning None if null or invalid.
pub unsafe fn nullable_cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

