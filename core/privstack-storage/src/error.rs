//! Error types for the storage layer.

use thiserror::Error;

/// Result type for storage operations.
pub type StorageResult<T> = Result<T, StorageError>;

/// Errors that can occur in storage operations.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Database error from SQLite.
    #[error("database error: {0}")]
    Database(#[from] privstack_db::rusqlite::Error),

    /// Database layer error from privstack-db.
    #[error("db error: {0}")]
    Db(#[from] privstack_db::DbError),

    /// Serialization/deserialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Entity not found.
    #[error("entity not found: {0}")]
    NotFound(String),

    /// IO error (file system).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Migration error.
    #[error("migration error: {0}")]
    Migration(String),

    /// Invalid data.
    #[error("invalid data: {0}")]
    InvalidData(String),
}
