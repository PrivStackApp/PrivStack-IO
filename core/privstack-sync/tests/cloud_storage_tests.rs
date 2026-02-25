//! Tests for cloud storage DTOs and configuration defaults.

use privstack_sync::cloud::storage::{CloudStorageConfig, CloudFile, ChangeSet};
use std::time::SystemTime;

#[test]
fn cloud_storage_config_default() {
    let config = CloudStorageConfig::default();
    assert_eq!(config.sync_folder, "PrivStack/sync");
    assert_eq!(config.poll_interval_secs, 30);
    assert_eq!(config.max_file_size, 50 * 1024 * 1024);
}

#[test]
fn cloud_storage_config_custom() {
    let config = CloudStorageConfig {
        sync_folder: "custom/path".to_string(),
        poll_interval_secs: 60,
        max_file_size: 100 * 1024 * 1024,
    };
    assert_eq!(config.sync_folder, "custom/path");
    assert_eq!(config.poll_interval_secs, 60);
    assert_eq!(config.max_file_size, 100 * 1024 * 1024);
}

#[test]
fn cloud_storage_config_serialize_roundtrip() {
    let config = CloudStorageConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: CloudStorageConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.sync_folder, config.sync_folder);
    assert_eq!(deserialized.poll_interval_secs, config.poll_interval_secs);
    assert_eq!(deserialized.max_file_size, config.max_file_size);
}

#[test]
fn cloud_file_create_and_clone() {
    let file = CloudFile {
        id: "file-123".to_string(),
        name: "test.json".to_string(),
        path: "PrivStack/sync/test.json".to_string(),
        size: 1024,
        modified_at: SystemTime::now(),
        content_hash: Some("abc123".to_string()),
    };
    let cloned = file.clone();
    assert_eq!(cloned.id, "file-123");
    assert_eq!(cloned.name, "test.json");
    assert_eq!(cloned.size, 1024);
    assert_eq!(cloned.content_hash, Some("abc123".to_string()));
}

#[test]
fn cloud_file_no_content_hash() {
    let file = CloudFile {
        id: "f1".to_string(),
        name: "nodata.bin".to_string(),
        path: "PrivStack/sync/nodata.bin".to_string(),
        size: 0,
        modified_at: SystemTime::now(),
        content_hash: None,
    };
    assert!(file.content_hash.is_none());
    assert_eq!(file.size, 0);
}

#[test]
fn cloud_file_serialize_roundtrip() {
    let file = CloudFile {
        id: "file-456".to_string(),
        name: "data.json".to_string(),
        path: "PrivStack/sync/data.json".to_string(),
        size: 2048,
        modified_at: SystemTime::UNIX_EPOCH,
        content_hash: Some("sha256:abcdef".to_string()),
    };
    let json = serde_json::to_string(&file).unwrap();
    let deserialized: CloudFile = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, file.id);
    assert_eq!(deserialized.name, file.name);
    assert_eq!(deserialized.path, file.path);
    assert_eq!(deserialized.size, file.size);
    assert_eq!(deserialized.content_hash, file.content_hash);
}

#[test]
fn change_set_empty() {
    let cs = ChangeSet {
        changed: vec![],
        deleted: vec![],
        next_cursor: None,
    };
    assert!(cs.changed.is_empty());
    assert!(cs.deleted.is_empty());
    assert!(cs.next_cursor.is_none());
}

#[test]
fn change_set_with_data() {
    let file = CloudFile {
        id: "f1".to_string(),
        name: "new.txt".to_string(),
        path: "PrivStack/sync/new.txt".to_string(),
        size: 100,
        modified_at: SystemTime::now(),
        content_hash: None,
    };
    let cs = ChangeSet {
        changed: vec![file],
        deleted: vec!["old-file-id".to_string()],
        next_cursor: Some("cursor-abc".to_string()),
    };
    assert_eq!(cs.changed.len(), 1);
    assert_eq!(cs.changed[0].name, "new.txt");
    assert_eq!(cs.deleted, vec!["old-file-id"]);
    assert_eq!(cs.next_cursor, Some("cursor-abc".to_string()));
}

#[test]
fn cloud_file_debug_format() {
    let file = CloudFile {
        id: "f1".to_string(),
        name: "test.txt".to_string(),
        path: "PrivStack/sync/test.txt".to_string(),
        size: 42,
        modified_at: SystemTime::UNIX_EPOCH,
        content_hash: None,
    };
    let debug = format!("{:?}", file);
    assert!(debug.contains("test.txt"));
    assert!(debug.contains("42"));
}

#[test]
fn cloud_storage_config_debug_format() {
    let config = CloudStorageConfig::default();
    let debug = format!("{:?}", config);
    assert!(debug.contains("PrivStack/sync"));
}

#[test]
fn cloud_storage_config_clone() {
    let config = CloudStorageConfig {
        sync_folder: "test/folder".to_string(),
        poll_interval_secs: 10,
        max_file_size: 1024,
    };
    let cloned = config.clone();
    assert_eq!(cloned.sync_folder, "test/folder");
    assert_eq!(cloned.poll_interval_secs, 10);
}

#[test]
fn change_set_clone() {
    let cs = ChangeSet {
        changed: vec![],
        deleted: vec!["id1".to_string(), "id2".to_string()],
        next_cursor: Some("token".to_string()),
    };
    let cloned = cs.clone();
    assert_eq!(cloned.deleted.len(), 2);
    assert_eq!(cloned.next_cursor, Some("token".to_string()));
}
