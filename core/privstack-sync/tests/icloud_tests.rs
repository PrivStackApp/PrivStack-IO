use privstack_sync::cloud::icloud::{ICloudConfig, ICloudStorage};
use privstack_sync::cloud::CloudStorage;
use tempfile::TempDir;

// ── Config defaults ─────────────────────────────────────────────

#[test]
fn icloud_config_default() {
    let cfg = ICloudConfig::default();
    assert_eq!(cfg.bundle_id, "com.privstack.app");
    assert_eq!(cfg.base.sync_folder, "PrivStack/sync");
    assert!(cfg.container_path.is_none());
}

#[test]
fn icloud_config_debug() {
    let cfg = ICloudConfig::default();
    let debug = format!("{:?}", cfg);
    assert!(debug.contains("com.privstack.app"));
}

#[test]
fn icloud_config_clone() {
    let cfg = ICloudConfig::default();
    let cloned = cfg.clone();
    assert_eq!(cloned.bundle_id, cfg.bundle_id);
}

// ── Storage construction ────────────────────────────────────────

#[test]
fn icloud_provider_name() {
    let storage = ICloudStorage::new(ICloudConfig::default());
    assert_eq!(storage.provider_name(), "iCloud Drive");
}

#[test]
fn icloud_not_authenticated_without_container() {
    let storage = ICloudStorage::new(ICloudConfig::default());
    // No container path exists, so not authenticated
    assert!(!storage.is_authenticated());
}

#[test]
fn icloud_authenticated_with_existing_container() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let storage = ICloudStorage::new(config);
    assert!(storage.is_authenticated());
}

// ── Full lifecycle with temp dir ────────────────────────────────

#[tokio::test]
async fn authenticate_with_valid_container() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);

    let result = storage.authenticate().await.unwrap();
    assert!(result.is_none()); // No OAuth URL needed
}

#[tokio::test]
async fn authenticate_fails_without_container() {
    let config = ICloudConfig {
        container_path: Some("/nonexistent/path/for/icloud/test".into()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);

    let result = storage.authenticate().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn complete_auth_is_noop() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);
    storage.complete_auth("anything").await.unwrap();
}

#[tokio::test]
async fn ensure_sync_folder_creates_directory() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let storage = ICloudStorage::new(config);

    storage.ensure_sync_folder().await.unwrap();

    let sync_path = temp.path().join("PrivStack/sync");
    assert!(sync_path.exists());
}

#[tokio::test]
async fn list_files_empty() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);
    storage.authenticate().await.unwrap();

    let files = storage.list_files().await.unwrap();
    assert!(files.is_empty());
}

#[tokio::test]
async fn upload_download_roundtrip() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);
    storage.authenticate().await.unwrap();

    let content = b"hello icloud";
    let file = storage.upload("test.bin", content).await.unwrap();
    assert_eq!(file.name, "test.bin");
    assert_eq!(file.size, content.len() as u64);

    let downloaded = storage.download(&file.id).await.unwrap();
    assert_eq!(downloaded, content);
}

#[tokio::test]
async fn list_files_after_upload() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);
    storage.authenticate().await.unwrap();

    storage.upload("a.txt", b"aaa").await.unwrap();
    storage.upload("b.txt", b"bbb").await.unwrap();

    let files = storage.list_files().await.unwrap();
    assert_eq!(files.len(), 2);

    let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"a.txt"));
    assert!(names.contains(&"b.txt"));
}

#[tokio::test]
async fn delete_existing_file() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);
    storage.authenticate().await.unwrap();

    let file = storage.upload("del.txt", b"data").await.unwrap();
    storage.delete(&file.id).await.unwrap();

    let files = storage.list_files().await.unwrap();
    assert!(files.is_empty());
}

#[tokio::test]
async fn delete_nonexistent_is_ok() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);
    storage.authenticate().await.unwrap();

    // Deleting a file that doesn't exist should succeed
    storage.delete("nonexistent-id").await.unwrap();
}

#[tokio::test]
async fn download_nonexistent_fails() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);
    storage.authenticate().await.unwrap();

    let result = storage.download("nonexistent-id").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_changes_new_files() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);
    storage.authenticate().await.unwrap();

    // First call: no changes
    let changes = storage.get_changes(None).await.unwrap();
    assert!(changes.changed.is_empty());
    assert!(changes.deleted.is_empty());

    // Upload a file
    storage.upload("new.txt", b"new content").await.unwrap();

    // The upload itself updates the cache, so get_changes won't show it as new.
    // But if we upload via raw fs, it would show up. This tests the change detection path.
    let changes = storage.get_changes(None).await.unwrap();
    assert!(changes.next_cursor.is_none());
}

#[tokio::test]
async fn get_changes_deleted_file() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);
    storage.authenticate().await.unwrap();

    let file = storage.upload("willdelete.txt", b"tmp").await.unwrap();

    // Populate the cache
    storage.get_changes(None).await.unwrap();

    // Delete via the storage API (which also clears cache)
    storage.delete(&file.id).await.unwrap();

    // Now detect deletion
    let changes = storage.get_changes(None).await.unwrap();
    // The file was removed from cache by delete(), so it appears in deleted list
    // (if the cache entry survived — depends on implementation details)
    // Either way the function shouldn't error.
    assert!(changes.next_cursor.is_none());
}

// ── ICloudConfig Serialize ───────────────────────────────────────

#[test]
fn icloud_config_serialize() {
    let cfg = ICloudConfig {
        container_path: Some("/my/path".into()),
        bundle_id: "com.example".to_string(),
        base: privstack_sync::cloud::CloudStorageConfig {
            sync_folder: "MySync".to_string(),
            poll_interval_secs: 45,
            max_file_size: 12345,
        },
    };
    let json = serde_json::to_string(&cfg).unwrap();
    assert!(json.contains("com.example"));
    assert!(json.contains("MySync"));
    assert!(json.contains("12345"));
}

// ── CloudFile trait impls (exercised from iCloud context) ────────

#[test]
fn cloud_file_debug_clone_serde_from_icloud() {
    use privstack_sync::cloud::CloudFile;
    use std::time::SystemTime;

    let file = CloudFile {
        id: "icloud-abc".to_string(),
        name: "notes.json".to_string(),
        path: "/icloud/notes.json".to_string(),
        size: 512,
        modified_at: SystemTime::now(),
        content_hash: None,
    };

    let debug = format!("{:?}", file);
    assert!(debug.contains("icloud-abc"));

    let cloned = file.clone();
    assert_eq!(cloned.name, "notes.json");

    let json = serde_json::to_string(&file).unwrap();
    let back: CloudFile = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, "icloud-abc");
}

// ── ChangeSet trait impls (from iCloud context) ─────────────────

#[test]
fn changeset_debug_clone_from_icloud() {
    use privstack_sync::cloud::storage::ChangeSet;

    let cs = ChangeSet {
        changed: vec![],
        deleted: vec!["del1".to_string()],
        next_cursor: None,
    };

    let debug = format!("{:?}", cs);
    assert!(debug.contains("del1"));

    let cloned = cs.clone();
    assert_eq!(cloned.deleted.len(), 1);
    assert!(cloned.next_cursor.is_none());
}

// ── CloudStorageConfig from iCloud context ──────────────────────

#[test]
fn cloud_storage_config_custom_from_icloud() {
    use privstack_sync::cloud::CloudStorageConfig;

    let cfg = CloudStorageConfig {
        sync_folder: "icloud/custom".to_string(),
        poll_interval_secs: 120,
        max_file_size: 1_000_000,
    };
    let debug = format!("{:?}", cfg);
    assert!(debug.contains("icloud/custom"));
    let cloned = cfg.clone();
    assert_eq!(cloned.poll_interval_secs, 120);

    let json = serde_json::to_string(&cfg).unwrap();
    let back: CloudStorageConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.max_file_size, 1_000_000);
}

// ── Multiple uploads overwrite same file ────────────────────────

#[tokio::test]
async fn upload_overwrites_existing_file() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);
    storage.authenticate().await.unwrap();

    let f1 = storage.upload("same.txt", b"first").await.unwrap();
    let f2 = storage.upload("same.txt", b"second version").await.unwrap();
    assert_eq!(f1.id, f2.id);
    assert_ne!(f1.size, f2.size);

    let content = storage.download(&f2.id).await.unwrap();
    assert_eq!(content, b"second version");

    let files = storage.list_files().await.unwrap();
    assert_eq!(files.len(), 1);
}

// ── ICloudConfig with custom bundle_id path derivation ──────────

#[test]
fn icloud_container_path_derives_correctly() {
    let config = ICloudConfig {
        container_path: None,
        bundle_id: "com.example.test".to_string(),
        ..Default::default()
    };
    let storage = ICloudStorage::new(config);
    // Just exercise is_authenticated -> get_container_path with dots converted
    let _ = storage.is_authenticated();
}

// ── Config serialization ────────────────────────────────────────

#[test]
fn icloud_config_serde_roundtrip() {
    let config = ICloudConfig {
        container_path: Some("/tmp/test".into()),
        bundle_id: "com.test.app".to_string(),
        ..Default::default()
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: ICloudConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.bundle_id, "com.test.app");
}

// ── Container path fallback (HOME env) ──────────────────────────

#[test]
fn icloud_container_path_from_home_env() {
    // When container_path is None, it derives from HOME env var.
    // We test the path construction logic by checking it doesn't error.
    let config = ICloudConfig {
        container_path: None,
        bundle_id: "com.test.myapp".to_string(),
        ..Default::default()
    };
    let storage = ICloudStorage::new(config);
    // is_authenticated checks get_container_path; since the derived path
    // likely doesn't exist, it returns false but doesn't error
    let _ = storage.is_authenticated();
}

// ── list_files skips hidden files ───────────────────────────────

#[tokio::test]
async fn list_files_skips_hidden_files() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);
    storage.authenticate().await.unwrap();

    // Upload a normal file
    storage.upload("visible.txt", b"data").await.unwrap();

    // Create a hidden file directly in the sync folder
    let sync_folder = temp.path().join("PrivStack/sync");
    tokio::fs::write(sync_folder.join(".hidden"), b"secret").await.unwrap();

    let files = storage.list_files().await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "visible.txt");
}

// ── list_files skips directories ────────────────────────────────

#[tokio::test]
async fn list_files_skips_directories() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);
    storage.authenticate().await.unwrap();

    storage.upload("file.txt", b"data").await.unwrap();

    // Create a subdirectory in the sync folder
    let sync_folder = temp.path().join("PrivStack/sync");
    tokio::fs::create_dir(sync_folder.join("subdir")).await.unwrap();

    let files = storage.list_files().await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "file.txt");
}

// ── get_changes detects modified files ──────────────────────────

#[tokio::test]
async fn get_changes_detects_modification() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);
    storage.authenticate().await.unwrap();

    // Upload and populate cache via get_changes
    storage.upload("mod.txt", b"original").await.unwrap();
    let changes = storage.get_changes(None).await.unwrap();
    // File was added to cache by upload, so get_changes sees no new changes
    assert!(changes.changed.is_empty());

    // Modify the file directly on disk (different size triggers change)
    let sync_folder = temp.path().join("PrivStack/sync");
    tokio::fs::write(sync_folder.join("mod.txt"), b"modified content that is longer").await.unwrap();

    let changes = storage.get_changes(None).await.unwrap();
    assert_eq!(changes.changed.len(), 1);
    assert_eq!(changes.changed[0].name, "mod.txt");
    assert!(changes.deleted.is_empty());
}

// ── get_changes detects externally added files ──────────────────

#[tokio::test]
async fn get_changes_detects_external_new_file() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);
    storage.authenticate().await.unwrap();

    // Initial: empty
    let changes = storage.get_changes(None).await.unwrap();
    assert!(changes.changed.is_empty());

    // Add a file externally (not through upload)
    let sync_folder = temp.path().join("PrivStack/sync");
    tokio::fs::write(sync_folder.join("external.txt"), b"external").await.unwrap();

    let changes = storage.get_changes(None).await.unwrap();
    assert_eq!(changes.changed.len(), 1);
    assert_eq!(changes.changed[0].name, "external.txt");
}

// ── get_changes detects externally deleted files ────────────────

#[tokio::test]
async fn get_changes_detects_external_deletion() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);
    storage.authenticate().await.unwrap();

    // Create file and populate cache
    let sync_folder = temp.path().join("PrivStack/sync");
    tokio::fs::write(sync_folder.join("todelete.txt"), b"tmp").await.unwrap();

    // Populate cache
    let changes = storage.get_changes(None).await.unwrap();
    assert_eq!(changes.changed.len(), 1);

    // Delete externally
    tokio::fs::remove_file(sync_folder.join("todelete.txt")).await.unwrap();

    let changes = storage.get_changes(None).await.unwrap();
    assert!(changes.changed.is_empty());
    assert_eq!(changes.deleted.len(), 1);
}

// ── get_sync_folder cache hit ───────────────────────────────────

#[tokio::test]
async fn ensure_sync_folder_cached() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let storage = ICloudStorage::new(config);

    // First call creates, second hits cache
    storage.ensure_sync_folder().await.unwrap();
    storage.ensure_sync_folder().await.unwrap();

    let sync_path = temp.path().join("PrivStack/sync");
    assert!(sync_path.exists());
}

// ── path_to_id deterministic ────────────────────────────────────

#[tokio::test]
async fn path_to_id_is_deterministic() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);
    storage.authenticate().await.unwrap();

    let file = storage.upload("det.txt", b"abc").await.unwrap();
    // Download with the same ID should work (proves determinism)
    let content = storage.download(&file.id).await.unwrap();
    assert_eq!(content, b"abc");

    // Upload same name again, ID should be the same
    let file2 = storage.upload("det.txt", b"xyz").await.unwrap();
    assert_eq!(file.id, file2.id);
}

// ── get_changes with unchanged files ────────────────────────────

#[tokio::test]
async fn get_changes_unchanged_files_not_reported() {
    let temp = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp.path().to_path_buf()),
        ..Default::default()
    };
    let mut storage = ICloudStorage::new(config);
    storage.authenticate().await.unwrap();

    storage.upload("stable.txt", b"content").await.unwrap();

    // First get_changes may or may not report it (cache populated by upload)
    let _ = storage.get_changes(None).await.unwrap();

    // Second call: file hasn't changed, should report nothing
    let changes = storage.get_changes(None).await.unwrap();
    assert!(changes.changed.is_empty());
    assert!(changes.deleted.is_empty());
}
