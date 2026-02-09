//! Error types for the PPK crate.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PpkError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("TOML serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("TOML deserialization error: {0}")]
    TomlDeserialize(#[from] toml::de::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("missing required entry: {0}")]
    MissingEntry(String),

    #[error("signature verification failed")]
    SignatureInvalid,

    #[error("package is not signed")]
    NotSigned,

    #[error("invalid public key")]
    InvalidPublicKey,

    #[error("manifest validation error: {0}")]
    ManifestInvalid(String),
}
