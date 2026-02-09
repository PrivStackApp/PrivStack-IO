//! iCloud Drive storage implementation.
//!
//! Uses file system access to iCloud Drive folder.
//! This works on macOS/iOS where iCloud Drive is mounted as a folder.

use super::storage::{ChangeSet, CloudFile, CloudStorage, CloudStorageConfig};
use crate::error::{SyncError, SyncResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// iCloud specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ICloudConfig {
    /// The iCloud Drive container path.
    /// On macOS: ~/Library/Mobile Documents/iCloud~com~privstack/
    /// Can be overridden for testing or custom setups.
    pub container_path: Option<PathBuf>,
    /// App bundle identifier for iCloud container.
    pub bundle_id: String,
    /// Base cloud storage config.
    #[serde(flatten)]
    pub base: CloudStorageConfig,
}

impl Default for ICloudConfig {
    fn default() -> Self {
        Self {
            container_path: None,
            bundle_id: "com.privstack.app".to_string(),
            base: CloudStorageConfig::default(),
        }
    }
}

/// Tracks file state for change detection.
#[derive(Debug, Clone)]
struct FileState {
    modified_at: SystemTime,
    size: u64,
}

/// iCloud Drive storage implementation.
pub struct ICloudStorage {
    config: ICloudConfig,
    sync_folder: Arc<RwLock<Option<PathBuf>>>,
    /// Cached file states for change detection.
    file_states: Arc<RwLock<HashMap<String, FileState>>>,
}

impl ICloudStorage {
    /// Creates a new iCloud storage instance.
    pub fn new(config: ICloudConfig) -> Self {
        Self {
            config,
            sync_folder: Arc::new(RwLock::new(None)),
            file_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Gets the iCloud Drive container path.
    fn get_container_path(&self) -> SyncResult<PathBuf> {
        if let Some(path) = &self.config.container_path {
            return Ok(path.clone());
        }

        // Standard macOS iCloud Drive path
        let home = std::env::var("HOME")
            .map_err(|_| SyncError::Storage("HOME environment variable not set".to_string()))?;

        // iCloud container format: ~/Library/Mobile Documents/iCloud~<bundle_id>/
        let container_name = format!("iCloud~{}", self.config.bundle_id.replace('.', "~"));
        let path = PathBuf::from(home)
            .join("Library")
            .join("Mobile Documents")
            .join(container_name);

        Ok(path)
    }

    /// Gets the sync folder path, creating it if necessary.
    async fn get_sync_folder(&self) -> SyncResult<PathBuf> {
        if let Some(path) = self.sync_folder.read().await.as_ref() {
            return Ok(path.clone());
        }

        let container = self.get_container_path()?;
        let sync_folder = container.join(&self.config.base.sync_folder);

        // Create sync folder if it doesn't exist
        if !sync_folder.exists() {
            fs::create_dir_all(&sync_folder)
                .await
                .map_err(|e| SyncError::Storage(format!("failed to create sync folder: {e}")))?;
            info!("Created iCloud sync folder: {:?}", sync_folder);
        }

        *self.sync_folder.write().await = Some(sync_folder.clone());
        Ok(sync_folder)
    }

    /// Generates a deterministic file ID from path.
    fn path_to_id(path: &std::path::Path) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        path.to_string_lossy().hash(&mut hasher);
        format!("icloud-{:x}", hasher.finish())
    }

    /// Converts a file path to CloudFile.
    async fn path_to_cloud_file(&self, path: PathBuf) -> SyncResult<CloudFile> {
        let metadata = fs::metadata(&path)
            .await
            .map_err(|e| SyncError::Storage(format!("failed to get file metadata: {e}")))?;

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let modified_at = metadata.modified().unwrap_or(SystemTime::now());

        Ok(CloudFile {
            id: Self::path_to_id(&path),
            name,
            path: path.to_string_lossy().to_string(),
            size: metadata.len(),
            modified_at,
            content_hash: None, // Could compute if needed
        })
    }
}

#[async_trait]
impl CloudStorage for ICloudStorage {
    fn provider_name(&self) -> &'static str {
        "iCloud Drive"
    }

    fn is_authenticated(&self) -> bool {
        // iCloud is "authenticated" if the container folder exists
        self.get_container_path()
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    async fn authenticate(&mut self) -> SyncResult<Option<String>> {
        let container = self.get_container_path()?;

        if !container.exists() {
            // iCloud container doesn't exist - user needs to enable iCloud Drive
            return Err(SyncError::Auth(format!(
                "iCloud Drive container not found at {:?}. \
                Please ensure iCloud Drive is enabled and the app has iCloud entitlements.",
                container
            )));
        }

        // Create sync folder
        self.get_sync_folder().await?;

        info!("iCloud Drive authenticated via container: {:?}", container);
        Ok(None) // No OAuth URL needed
    }

    async fn complete_auth(&mut self, _auth_code: &str) -> SyncResult<()> {
        // No-op for iCloud - auth is implicit via file system access
        Ok(())
    }

    async fn list_files(&self) -> SyncResult<Vec<CloudFile>> {
        let sync_folder = self.get_sync_folder().await?;

        let mut files = Vec::new();
        let mut read_dir = fs::read_dir(&sync_folder)
            .await
            .map_err(|e| SyncError::Storage(format!("failed to read sync folder: {e}")))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| SyncError::Storage(format!("failed to read directory entry: {e}")))?
        {
            let path = entry.path();

            // Skip directories and hidden files
            if path.is_dir()
                || path
                    .file_name()
                    .map(|n| n.to_string_lossy().starts_with('.'))
                    .unwrap_or(false)
            {
                continue;
            }

            match self.path_to_cloud_file(path).await {
                Ok(file) => files.push(file),
                Err(e) => warn!("Skipping file due to error: {e}"),
            }
        }

        Ok(files)
    }

    async fn get_changes(&self, _cursor: Option<&str>) -> SyncResult<ChangeSet> {
        let current_files = self.list_files().await?;
        let mut file_states = self.file_states.write().await;

        let mut changed = Vec::new();
        let mut deleted: Vec<String> = file_states.keys().cloned().collect();

        for file in current_files {
            // Remove from deleted list (file still exists)
            deleted.retain(|id| id != &file.id);

            // Check if file is new or modified
            let is_changed = match file_states.get(&file.id) {
                Some(state) => file.modified_at > state.modified_at || file.size != state.size,
                None => true, // New file
            };

            if is_changed {
                // Update cached state
                file_states.insert(
                    file.id.clone(),
                    FileState {
                        modified_at: file.modified_at,
                        size: file.size,
                    },
                );
                changed.push(file);
            }
        }

        // Remove deleted files from cache
        for id in &deleted {
            file_states.remove(id);
        }

        Ok(ChangeSet {
            changed,
            deleted,
            next_cursor: None, // File-based doesn't use cursors
        })
    }

    async fn upload(&self, name: &str, content: &[u8]) -> SyncResult<CloudFile> {
        let sync_folder = self.get_sync_folder().await?;
        let file_path = sync_folder.join(name);

        debug!(
            "Uploading to iCloud: {:?} ({} bytes)",
            file_path,
            content.len()
        );

        fs::write(&file_path, content)
            .await
            .map_err(|e| SyncError::Storage(format!("failed to write file: {e}")))?;

        let file = self.path_to_cloud_file(file_path).await?;
        info!("Uploaded file to iCloud: {}", name);

        // Update cache
        let mut file_states = self.file_states.write().await;
        file_states.insert(
            file.id.clone(),
            FileState {
                modified_at: file.modified_at,
                size: file.size,
            },
        );

        Ok(file)
    }

    async fn download(&self, file_id: &str) -> SyncResult<Vec<u8>> {
        let sync_folder = self.get_sync_folder().await?;

        // Find file by ID (need to scan folder)
        let mut read_dir = fs::read_dir(&sync_folder)
            .await
            .map_err(|e| SyncError::Storage(format!("failed to read sync folder: {e}")))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| SyncError::Storage(format!("failed to read directory entry: {e}")))?
        {
            let path = entry.path();
            if Self::path_to_id(&path) == file_id {
                debug!("Downloading from iCloud: {:?}", path);
                let content = fs::read(&path)
                    .await
                    .map_err(|e| SyncError::Storage(format!("failed to read file: {e}")))?;
                return Ok(content);
            }
        }

        Err(SyncError::Storage(format!("file not found: {file_id}")))
    }

    async fn delete(&self, file_id: &str) -> SyncResult<()> {
        let sync_folder = self.get_sync_folder().await?;

        // Find file by ID
        let mut read_dir = fs::read_dir(&sync_folder)
            .await
            .map_err(|e| SyncError::Storage(format!("failed to read sync folder: {e}")))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| SyncError::Storage(format!("failed to read directory entry: {e}")))?
        {
            let path = entry.path();
            if Self::path_to_id(&path) == file_id {
                debug!("Deleting from iCloud: {:?}", path);
                fs::remove_file(&path)
                    .await
                    .map_err(|e| SyncError::Storage(format!("failed to delete file: {e}")))?;

                // Remove from cache
                let mut file_states = self.file_states.write().await;
                file_states.remove(file_id);

                info!("Deleted file from iCloud: {:?}", path);
                return Ok(());
            }
        }

        // File doesn't exist - that's fine for delete
        Ok(())
    }

    async fn ensure_sync_folder(&self) -> SyncResult<()> {
        self.get_sync_folder().await?;
        Ok(())
    }
}
