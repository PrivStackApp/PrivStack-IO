//! Database error types.

pub type DbResult<T> = Result<T, DbError>;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid database path: {0}")]
    InvalidPath(String),

    #[error("Wrong encryption key")]
    WrongKey,
}
