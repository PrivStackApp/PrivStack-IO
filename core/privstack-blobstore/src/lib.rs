//! Namespace-scoped blob storage backed by SQLite.
//!
//! Data is stored as plaintext in the database — at-rest encryption is
//! handled by SQLCipher at the database file level. Content hashes are
//! SHA-256 of the plaintext for dedup checks.

use chrono::Utc;
use privstack_db::rusqlite::{self, params, Connection};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::{Arc, Mutex};

// ============================================================================
// Error types
// ============================================================================

#[derive(Debug, thiserror::Error)]
pub enum BlobStoreError {
    #[error("blob not found: {0}/{1}")]
    NotFound(String, String),
    #[error("storage error: {0}")]
    Storage(String),
}

pub type BlobStoreResult<T> = Result<T, BlobStoreError>;

// ============================================================================
// BlobMetadata
// ============================================================================

#[derive(Debug, Serialize)]
pub struct BlobMetadata {
    pub namespace: String,
    pub blob_id: String,
    pub size: i64,
    pub content_hash: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: i64,
    pub modified_at: i64,
}

// ============================================================================
// BlobStore
// ============================================================================

pub struct BlobStore {
    conn: Arc<Mutex<Connection>>,
}

impl BlobStore {
    /// Open a blob store backed by a SQLite file (no encryption).
    pub fn open(db_path: &Path) -> BlobStoreResult<Self> {
        let conn = privstack_db::open_db_unencrypted(db_path)
            .map_err(|e| BlobStoreError::Storage(e.to_string()))?;

        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.ensure_tables()?;
        Ok(store)
    }

    /// Open with an existing shared connection.
    pub fn open_with_conn(conn: Arc<Mutex<Connection>>) -> BlobStoreResult<Self> {
        let store = Self { conn };
        store.ensure_tables()?;
        Ok(store)
    }

    /// Open in-memory (for testing).
    pub fn open_in_memory() -> BlobStoreResult<Self> {
        let conn =
            privstack_db::open_in_memory().map_err(|e| BlobStoreError::Storage(e.to_string()))?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.ensure_tables()?;
        Ok(store)
    }

    /// Flush the WAL to the main database file.
    pub fn checkpoint(&self) -> BlobStoreResult<()> {
        let conn = self.conn.lock().map_err(|e| BlobStoreError::Storage(e.to_string()))?;
        privstack_db::checkpoint(&conn).map_err(|e| BlobStoreError::Storage(e.to_string()))?;
        Ok(())
    }

    fn ensure_tables(&self) -> BlobStoreResult<()> {
        let conn = self.conn.lock().map_err(|e| BlobStoreError::Storage(e.to_string()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS blobs (
                namespace TEXT NOT NULL,
                blob_id TEXT NOT NULL,
                data BLOB NOT NULL,
                size INTEGER NOT NULL DEFAULT 0,
                content_hash TEXT,
                metadata_json TEXT,
                created_at INTEGER NOT NULL,
                modified_at INTEGER NOT NULL,
                PRIMARY KEY (namespace, blob_id)
            );",
        )
        .map_err(|e| BlobStoreError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Store a blob. Data is stored as-is (plaintext).
    pub fn store(
        &self,
        namespace: &str,
        id: &str,
        data: &[u8],
        metadata_json: Option<&str>,
    ) -> BlobStoreResult<()> {
        let content_hash = hex_encode(Sha256::digest(data));
        let now = Utc::now().timestamp_millis();

        let conn = self.conn.lock().map_err(|e| BlobStoreError::Storage(e.to_string()))?;
        conn.execute(
            "INSERT OR REPLACE INTO blobs (namespace, blob_id, data, size, content_hash, metadata_json, created_at, modified_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, COALESCE((SELECT created_at FROM blobs WHERE namespace = ?1 AND blob_id = ?2), ?7), ?7)",
            params![namespace, id, data, data.len() as i64, content_hash, metadata_json, now],
        )
        .map_err(|e| BlobStoreError::Storage(e.to_string()))?;

        Ok(())
    }

    /// Read a blob's data.
    pub fn read(&self, namespace: &str, id: &str) -> BlobStoreResult<Vec<u8>> {
        let conn = self.conn.lock().map_err(|e| BlobStoreError::Storage(e.to_string()))?;
        conn.query_row(
            "SELECT data FROM blobs WHERE namespace = ?1 AND blob_id = ?2",
            params![namespace, id],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                BlobStoreError::NotFound(namespace.to_string(), id.to_string())
            }
            _ => BlobStoreError::Storage(e.to_string()),
        })
    }

    /// Delete a blob.
    pub fn delete(&self, namespace: &str, id: &str) -> BlobStoreResult<()> {
        let conn = self.conn.lock().map_err(|e| BlobStoreError::Storage(e.to_string()))?;
        let affected = conn
            .execute(
                "DELETE FROM blobs WHERE namespace = ?1 AND blob_id = ?2",
                params![namespace, id],
            )
            .map_err(|e| BlobStoreError::Storage(e.to_string()))?;

        if affected == 0 {
            return Err(BlobStoreError::NotFound(
                namespace.to_string(),
                id.to_string(),
            ));
        }
        Ok(())
    }

    /// List blob metadata for a namespace.
    pub fn list(&self, namespace: &str) -> BlobStoreResult<Vec<BlobMetadata>> {
        let conn = self.conn.lock().map_err(|e| BlobStoreError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT namespace, blob_id, size, content_hash, metadata_json, created_at, modified_at
                 FROM blobs WHERE namespace = ?1 ORDER BY modified_at DESC",
            )
            .map_err(|e| BlobStoreError::Storage(e.to_string()))?;

        let items: Vec<BlobMetadata> = stmt
            .query_map(params![namespace], |row| {
                Ok(BlobMetadata {
                    namespace: row.get(0)?,
                    blob_id: row.get(1)?,
                    size: row.get(2)?,
                    content_hash: row.get(3)?,
                    metadata_json: row.get(4)?,
                    created_at: row.get(5)?,
                    modified_at: row.get(6)?,
                })
            })
            .map_err(|e| BlobStoreError::Storage(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(items)
    }

    /// Update metadata for a blob.
    pub fn update_metadata(
        &self,
        namespace: &str,
        id: &str,
        metadata_json: &str,
    ) -> BlobStoreResult<()> {
        let now = Utc::now().timestamp_millis();
        let conn = self.conn.lock().map_err(|e| BlobStoreError::Storage(e.to_string()))?;
        let affected = conn
            .execute(
                "UPDATE blobs SET metadata_json = ?1, modified_at = ?2 WHERE namespace = ?3 AND blob_id = ?4",
                params![metadata_json, now, namespace, id],
            )
            .map_err(|e| BlobStoreError::Storage(e.to_string()))?;

        if affected == 0 {
            return Err(BlobStoreError::NotFound(
                namespace.to_string(),
                id.to_string(),
            ));
        }
        Ok(())
    }
}

fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
