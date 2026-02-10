//! Error types for the licensing module.

use thiserror::Error;

/// Licensing-specific errors.
#[derive(Debug, Error)]
pub enum LicenseError {
    /// Invalid license key format.
    #[error("invalid license key format: {0}")]
    InvalidKeyFormat(String),

    /// Ed25519 signature verification failed.
    #[error("license key signature invalid")]
    InvalidSignature,

    /// Payload JSON is malformed or missing required fields.
    #[error("invalid license payload: {0}")]
    InvalidPayload(String),

    /// License has expired.
    #[error("license expired on {0}")]
    Expired(String),

    /// License not activated.
    #[error("license not activated")]
    NotActivated,

    /// Activation failed.
    #[error("activation failed: {0}")]
    ActivationFailed(String),

    /// Device limit exceeded.
    #[error("device limit exceeded (max {0} devices)")]
    DeviceLimitExceeded(u32),

    /// License revoked.
    #[error("license has been revoked")]
    Revoked,

    /// Network error during activation.
    #[error("network error: {0}")]
    Network(String),

    /// Storage error.
    #[error("storage error: {0}")]
    Storage(String),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Result type for license operations.
pub type LicenseResult<T> = Result<T, LicenseError>;
