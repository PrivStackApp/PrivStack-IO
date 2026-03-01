//! Error types for the datasets module.

use thiserror::Error;

/// All errors that can occur in dataset operations.
#[derive(Debug, Error)]
pub enum DatasetError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] privstack_db::rusqlite::Error),

    #[error("Database error: {0}")]
    Db(#[from] privstack_db::DbError),

    #[error("Dataset not found: {0}")]
    NotFound(String),

    #[error("Import failed: {0}")]
    ImportFailed(String),

    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),
}

pub type DatasetResult<T> = Result<T, DatasetError>;
