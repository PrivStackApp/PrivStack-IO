use privstack_sync::cloud::google_drive::GoogleDriveConfig;
use privstack_sync::cloud::google_drive::GoogleDriveStorage;
use privstack_sync::cloud::icloud::{ICloudConfig, ICloudStorage};
use privstack_sync::cloud::CloudStorage;
use tempfile::TempDir;

#[test]
fn google_drive_default_config() {
    let config = GoogleDriveConfig::default();
    assert_eq!(config.base.sync_folder, "PrivStack/sync");
    assert_eq!(config.redirect_uri, "urn:ietf:wg:oauth:2.0:oob");
}

#[test]
fn google_drive_create_storage() {
    let config = GoogleDriveConfig::default();
    let storage = GoogleDriveStorage::new(config);
    assert_eq!(storage.provider_name(), "Google Drive");
    assert!(!storage.is_authenticated());
}

#[test]
fn icloud_default_config() {
    let config = ICloudConfig::default();
    assert_eq!(config.bundle_id, "com.privstack.app");
    assert_eq!(config.base.sync_folder, "PrivStack/sync");
}

#[tokio::test]
async fn icloud_test_with_temp_dir() {
    let temp_dir = TempDir::new().unwrap();
    let config = ICloudConfig {
        container_path: Some(temp_dir.path().to_path_buf()),
        ..Default::default()
    };

    let mut storage = ICloudStorage::new(config);

    let result = storage.authenticate().await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());

    let files = storage.list_files().await.unwrap();
    assert!(files.is_empty());

    let content = b"test content";
    let file = storage.upload("test.txt", content).await.unwrap();
    assert_eq!(file.name, "test.txt");
    assert_eq!(file.size, content.len() as u64);

    let files = storage.list_files().await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "test.txt");

    let downloaded = storage.download(&file.id).await.unwrap();
    assert_eq!(downloaded, content);

    storage.delete(&file.id).await.unwrap();
    let files = storage.list_files().await.unwrap();
    assert!(files.is_empty());
}
