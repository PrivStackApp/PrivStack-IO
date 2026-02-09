//! Cloud storage abstraction trait.
//!
//! Defines a common interface for cloud storage providers.

use crate::error::SyncResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Configuration for cloud storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudStorageConfig {
    /// The folder path within the cloud storage for sync files.
    pub sync_folder: String,
    /// How often to poll for changes (in seconds).
    pub poll_interval_secs: u64,
    /// Maximum file size for sync (in bytes).
    pub max_file_size: u64,
}

impl Default for CloudStorageConfig {
    fn default() -> Self {
        Self {
            sync_folder: "PrivStack/sync".to_string(),
            poll_interval_secs: 30,
            max_file_size: 50 * 1024 * 1024, // 50 MB
        }
    }
}

/// Metadata about a file in cloud storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudFile {
    /// The file's unique identifier in the cloud storage.
    pub id: String,
    /// The file name.
    pub name: String,
    /// The full path within the sync folder.
    pub path: String,
    /// File size in bytes.
    pub size: u64,
    /// Last modified time.
    pub modified_at: SystemTime,
    /// Content hash (if available).
    pub content_hash: Option<String>,
}

/// Result of a change detection operation.
#[derive(Debug, Clone)]
pub struct ChangeSet {
    /// Files that are new or modified since last sync.
    pub changed: Vec<CloudFile>,
    /// Files that were deleted.
    pub deleted: Vec<String>,
    /// A token/cursor to use for the next change detection.
    pub next_cursor: Option<String>,
}

/// Abstract cloud storage interface.
#[async_trait]
pub trait CloudStorage: Send + Sync {
    /// Returns the name of the cloud storage provider.
    fn provider_name(&self) -> &'static str;

    /// Returns whether the storage is authenticated and ready.
    fn is_authenticated(&self) -> bool;

    /// Authenticates with the cloud storage.
    /// Returns an authentication URL for OAuth flow if user interaction is needed.
    async fn authenticate(&mut self) -> SyncResult<Option<String>>;

    /// Completes OAuth authentication with an authorization code.
    async fn complete_auth(&mut self, auth_code: &str) -> SyncResult<()>;

    /// Lists all files in the sync folder.
    async fn list_files(&self) -> SyncResult<Vec<CloudFile>>;

    /// Gets changes since the last sync using a change cursor.
    async fn get_changes(&self, cursor: Option<&str>) -> SyncResult<ChangeSet>;

    /// Uploads a file to the sync folder.
    async fn upload(&self, name: &str, content: &[u8]) -> SyncResult<CloudFile>;

    /// Downloads a file's content.
    async fn download(&self, file_id: &str) -> SyncResult<Vec<u8>>;

    /// Deletes a file.
    async fn delete(&self, file_id: &str) -> SyncResult<()>;

    /// Creates the sync folder if it doesn't exist.
    async fn ensure_sync_folder(&self) -> SyncResult<()>;
}
