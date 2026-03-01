//! SQLite storage layer for PrivStack.
//!
//! Provides persistent storage for generic entities using SQLite (via privstack-db).
//! SQLite with custom functions supports AI features like semantic search
//! and embeddings via the `cosine_similarity()` function.
//!
//! # Architecture
//!
//! - Entities are stored as typed JSON blobs with schema-driven field extraction
//! - Events are stored for sync protocol replication
//! - Entity links support cross-plugin references
//! - Schema migrations are handled automatically on startup

mod error;
pub mod entity_store;
mod event_store;

pub use entity_store::{EntityStore, scan_db_file, scan_db_connection, compact_db_file};
pub use event_store::EventStore;
pub use error::{StorageError, StorageResult};
