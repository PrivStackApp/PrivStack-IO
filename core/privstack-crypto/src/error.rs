//! Error types for the encryption layer.

use thiserror::Error;

/// Result type for crypto operations.
pub type CryptoResult<T> = Result<T, CryptoError>;

/// Errors that can occur in cryptographic operations.
#[derive(Debug, Error)]
pub enum CryptoError {
    /// Key derivation failed.
    #[error("key derivation failed: {0}")]
    KeyDerivation(String),

    /// Encryption failed.
    #[error("encryption failed: {0}")]
    Encryption(String),

    /// Decryption failed (wrong key or tampered data).
    #[error("decryption failed: {0}")]
    Decryption(String),

    /// Invalid key length.
    #[error("invalid key length: expected {expected}, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },

    /// Invalid nonce length.
    #[error("invalid nonce length: expected {expected}, got {actual}")]
    InvalidNonceLength { expected: usize, actual: usize },

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
