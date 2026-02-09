//! Error types for the sync layer.

use thiserror::Error;

/// Result type for sync operations.
pub type SyncResult<T> = Result<T, SyncError>;

/// Errors that can occur in sync operations.
#[derive(Debug, Error)]
pub enum SyncError {
    /// Network error.
    #[error("network error: {0}")]
    Network(String),

    /// Protocol error (invalid message format).
    #[error("protocol error: {0}")]
    Protocol(String),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Storage error.
    #[error("storage error: {0}")]
    Storage(String),

    /// Authentication error.
    #[error("authentication error: {0}")]
    Auth(String),

    /// Peer not found.
    #[error("peer not found: {0}")]
    PeerNotFound(String),

    /// Connection refused.
    #[error("connection refused: {0}")]
    ConnectionRefused(String),

    /// Timeout.
    #[error("operation timed out")]
    Timeout,

    /// Channel closed.
    #[error("channel closed")]
    ChannelClosed,

    /// Policy denied the operation.
    #[error("policy denied: {reason}")]
    PolicyDenied { reason: String },
}
