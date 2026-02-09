//! DuckDB storage layer for PrivStack.
//!
//! Provides persistent storage for generic entities using DuckDB.
//! DuckDB is chosen for its analytical query capabilities which support
//! AI features like semantic search and embeddings.
//!
//! # Architecture
//!
//! - Entities are stored as typed JSON blobs with schema-driven field extraction
//! - Events are stored for sync protocol replication
//! - Entity links support cross-plugin references
//! - Schema migrations are handled automatically on startup

mod error;
mod entity_store;
mod event_store;

pub use entity_store::EntityStore;
pub use event_store::EventStore;
pub use error::{StorageError, StorageResult};

/// Open a DuckDB connection with stale WAL recovery.
///
/// If the initial open fails and a `.wal` file exists alongside the database,
/// it is removed and the open is retried once. This handles the common case
/// where an unclean shutdown leaves a WAL file that prevents reopening.
pub fn open_duckdb_with_wal_recovery(path: &std::path::Path) -> StorageResult<duckdb::Connection> {
    match duckdb::Connection::open(path) {
        Ok(conn) => Ok(conn),
        Err(first_err) => {
            let wal_path = path.with_extension(
                path.extension()
                    .map(|ext| format!("{}.wal", ext.to_string_lossy()))
                    .unwrap_or_else(|| "wal".to_string()),
            );
            if wal_path.exists() {
                eprintln!(
                    "[WARN] DuckDB open failed, removing stale WAL and retrying: {}",
                    wal_path.display()
                );
                if std::fs::remove_file(&wal_path).is_ok() {
                    return duckdb::Connection::open(path).map_err(Into::into);
                }
            }
            Err(first_err.into())
        }
    }
}
