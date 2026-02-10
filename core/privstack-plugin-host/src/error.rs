//! Error types for the plugin host.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginHostError {
    #[error("plugin not found: {0}")]
    PluginNotFound(String),

    #[error("plugin already loaded: {0}")]
    PluginAlreadyLoaded(String),

    #[error("wasm compilation error: {0}")]
    Compilation(#[from] wasmtime::Error),

    #[error("permission denied: plugin '{plugin_id}' lacks '{permission}' capability")]
    PermissionDenied {
        plugin_id: String,
        permission: String,
    },

    #[error("entity type '{entity_type}' not declared by plugin '{plugin_id}'")]
    EntityTypeNotDeclared {
        plugin_id: String,
        entity_type: String,
    },

    #[error("plugin crashed: {plugin_id}: {message}")]
    PluginCrashed {
        plugin_id: String,
        message: String,
    },

    #[error("resource limit exceeded: {plugin_id}: {detail}")]
    ResourceLimitExceeded {
        plugin_id: String,
        detail: String,
    },

    #[error("plugin initialization failed: {0}")]
    InitializationFailed(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("network error: {0}")]
    NetworkError(String),

    #[error("policy denied: {0}")]
    PolicyDenied(String),

    #[error("invalid WIT schema: {0}")]
    InvalidSchema(String),

    #[error("timeout: plugin '{plugin_id}' exceeded {timeout_ms}ms deadline")]
    Timeout {
        plugin_id: String,
        timeout_ms: u64,
    },

    #[error("capability '{capability}' not supported by plugin '{plugin_id}'")]
    CapabilityNotSupported {
        plugin_id: String,
        capability: String,
    },
}
