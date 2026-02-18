//! Cloud sync error types.

use thiserror::Error;

/// Result type for cloud operations.
pub type CloudResult<T> = Result<T, CloudError>;

/// Errors that can occur in cloud sync operations.
#[derive(Debug, Error)]
pub enum CloudError {
    #[error("S3 operation failed: {0}")]
    S3(String),

    #[error("API request failed: {0}")]
    Api(String),

    #[error("storage quota exceeded: used {used} of {quota} bytes")]
    QuotaExceeded { used: u64, quota: u64 },

    #[error("STS credentials expired or invalid")]
    CredentialExpired,

    #[error("entity lock contention: {0}")]
    LockContention(String),

    #[error("share operation denied: {0}")]
    ShareDenied(String),

    #[error("envelope encryption error: {0}")]
    Envelope(String),

    #[error("authentication required")]
    AuthRequired,

    #[error("authentication failed: {0}")]
    AuthFailed(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("crypto error: {0}")]
    Crypto(#[from] privstack_crypto::CryptoError),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("rate limited: retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    #[error("invalid configuration: {0}")]
    Config(String),
}

impl CloudError {
    /// Returns true if this error represents a 429 rate-limit response.
    pub fn is_rate_limited(&self) -> bool {
        match self {
            CloudError::RateLimited { .. } => true,
            CloudError::Api(msg) => msg.contains("429"),
            CloudError::Http(e) => e.status().is_some_and(|s| s.as_u16() == 429),
            _ => false,
        }
    }

    /// Returns the retry-after duration if this is a rate-limit error.
    pub fn retry_after(&self) -> Option<std::time::Duration> {
        match self {
            CloudError::RateLimited { retry_after_secs } => {
                Some(std::time::Duration::from_secs(*retry_after_secs))
            }
            _ => None,
        }
    }
}
