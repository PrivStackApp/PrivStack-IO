use privstack_sync::cloud::google_drive::{GoogleDriveConfig, GoogleDriveStorage};
use privstack_sync::cloud::CloudStorage;
use wiremock::matchers::{method, path, path_regex, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ── Config defaults ─────────────────────────────────────────────

#[test]
fn google_drive_config_default() {
    let cfg = GoogleDriveConfig::default();
    assert_eq!(cfg.base.sync_folder, "PrivStack/sync");
    assert_eq!(cfg.redirect_uri, "urn:ietf:wg:oauth:2.0:oob");
    assert!(cfg.client_id.is_empty());
    assert!(cfg.client_secret.is_empty());
    assert_eq!(cfg.api_base_url, "https://www.googleapis.com");
    assert_eq!(cfg.oauth_base_url, "https://oauth2.googleapis.com");
    assert_eq!(cfg.auth_base_url, "https://accounts.google.com");
}

#[test]
fn google_drive_config_debug() {
    let cfg = GoogleDriveConfig::default();
    let debug = format!("{:?}", cfg);
    assert!(debug.contains("redirect_uri"));
    assert!(debug.contains("api_base_url"));
}

#[test]
fn google_drive_config_clone() {
    let cfg = GoogleDriveConfig {
        client_id: "test_id".to_string(),
        client_secret: "test_secret".to_string(),
        ..Default::default()
    };
    let cloned = cfg.clone();
    assert_eq!(cloned.client_id, "test_id");
    assert_eq!(cloned.client_secret, "test_secret");
}

#[test]
fn google_drive_config_serde_roundtrip() {
    let cfg = GoogleDriveConfig {
        client_id: "my_id".to_string(),
        client_secret: "my_secret".to_string(),
        redirect_uri: "http://localhost".to_string(),
        ..Default::default()
    };
    let json = serde_json::to_string(&cfg).unwrap();
    let deserialized: GoogleDriveConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.client_id, "my_id");
    assert_eq!(deserialized.redirect_uri, "http://localhost");
    assert_eq!(deserialized.api_base_url, "https://www.googleapis.com");
}

// ── Storage construction ────────────────────────────────────────

#[test]
fn google_drive_provider_name() {
    let storage = GoogleDriveStorage::new(GoogleDriveConfig::default());
    assert_eq!(storage.provider_name(), "Google Drive");
}

#[test]
fn google_drive_not_authenticated_by_default() {
    let storage = GoogleDriveStorage::new(GoogleDriveConfig::default());
    assert!(!storage.is_authenticated());
}

#[test]
fn google_drive_with_credentials() {
    let config = GoogleDriveConfig {
        client_id: "test_client_id".to_string(),
        client_secret: "test_client_secret".to_string(),
        ..Default::default()
    };
    let storage = GoogleDriveStorage::new(config);
    assert_eq!(storage.provider_name(), "Google Drive");
    assert!(!storage.is_authenticated());
}

// ── authenticate returns OAuth URL ──────────────────────────────

#[tokio::test]
async fn google_drive_authenticate_returns_url() {
    let config = GoogleDriveConfig {
        client_id: "test_client_id".to_string(),
        client_secret: "test_secret".to_string(),
        ..Default::default()
    };
    let mut storage = GoogleDriveStorage::new(config);

    let result = storage.authenticate().await.unwrap();
    assert!(result.is_some());
    let url = result.unwrap();
    assert!(url.contains("test_client_id"));
    assert!(url.contains("accounts.google.com"));
}

#[tokio::test]
async fn google_drive_authenticate_already_authenticated() {
    let config = GoogleDriveConfig::default();
    let mut storage = GoogleDriveStorage::new(config);

    storage
        .set_tokens("token".to_string(), None)
        .await;

    let result = storage.authenticate().await.unwrap();
    assert!(result.is_none()); // already authenticated
}

// ── set_tokens ──────────────────────────────────────────────────

#[tokio::test]
async fn google_drive_set_tokens_makes_authenticated() {
    let config = GoogleDriveConfig::default();
    let storage = GoogleDriveStorage::new(config);

    storage
        .set_tokens("access_token_123".to_string(), Some("refresh_456".to_string()))
        .await;

    assert!(storage.is_authenticated());
}

#[tokio::test]
async fn google_drive_set_tokens_without_refresh() {
    let config = GoogleDriveConfig::default();
    let storage = GoogleDriveStorage::new(config);

    storage
        .set_tokens("access_only".to_string(), None)
        .await;

    assert!(storage.is_authenticated());
}

// ── CloudStorageConfig defaults ─────────────────────────────────

#[test]
fn cloud_storage_config_defaults() {
    let cfg = privstack_sync::cloud::CloudStorageConfig::default();
    assert_eq!(cfg.sync_folder, "PrivStack/sync");
    assert_eq!(cfg.poll_interval_secs, 30);
    assert_eq!(cfg.max_file_size, 50 * 1024 * 1024);
}

// ── Wiremock-based integration tests ────────────────────────────

fn mock_config(server: &MockServer) -> GoogleDriveConfig {
    GoogleDriveConfig {
        client_id: "test_client".to_string(),
        client_secret: "test_secret".to_string(),
        api_base_url: server.uri(),
        oauth_base_url: server.uri(),
        auth_base_url: server.uri(),
        ..Default::default()
    }
}

#[tokio::test]
async fn complete_auth_exchanges_code_for_tokens() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "new_access_token",
            "refresh_token": "new_refresh_token",
            "expires_in": 3600
        })))
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let mut storage = GoogleDriveStorage::new(config);

    storage.complete_auth("auth_code_123").await.unwrap();
    assert!(storage.is_authenticated());
}

#[tokio::test]
async fn complete_auth_failure_returns_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": "invalid_grant"
        })))
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let mut storage = GoogleDriveStorage::new(config);

    let result = storage.complete_auth("bad_code").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn list_files_empty() {
    let server = MockServer::start().await;

    // get_or_create_sync_folder: search for "PrivStack" folder
    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .and(query_param("fields", "files(id,name)"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files": [{"id": "folder_privstack", "name": "PrivStack", "mimeType": "application/vnd.google-apps.folder"}]
        })))
        .expect(1..)
        .mount(&server)
        .await;

    // list_files call with full fields
    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .and(query_param("pageSize", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files": []
        })))
        .expect(1..)
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    let files = storage.list_files().await.unwrap();
    assert!(files.is_empty());
}

#[tokio::test]
async fn list_files_not_authenticated() {
    let config = GoogleDriveConfig::default();
    let storage = GoogleDriveStorage::new(config);

    let result = storage.list_files().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn upload_and_download_roundtrip() {
    let server = MockServer::start().await;

    // Folder lookup
    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files": [{"id": "folder_id", "name": "PrivStack", "mimeType": "application/vnd.google-apps.folder"}]
        })))
        .expect(1..)
        .mount(&server)
        .await;

    // Upload
    Mock::given(method("POST"))
        .and(path_regex("/upload/drive/v3/files.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "file_123",
            "name": "test.bin",
            "mimeType": "application/octet-stream",
            "size": "5",
            "modifiedTime": "2024-01-01T00:00:00Z",
            "md5Checksum": "abc123"
        })))
        .mount(&server)
        .await;

    // Download
    Mock::given(method("GET"))
        .and(path("/drive/v3/files/file_123"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello".to_vec()))
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    let uploaded = storage.upload("test.bin", b"hello").await.unwrap();
    assert_eq!(uploaded.id, "file_123");
    assert_eq!(uploaded.name, "test.bin");

    let downloaded = storage.download("file_123").await.unwrap();
    assert_eq!(downloaded, b"hello");
}

#[tokio::test]
async fn upload_failure() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files": [{"id": "folder_id", "name": "PrivStack", "mimeType": "application/vnd.google-apps.folder"}]
        })))
        .expect(1..)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path_regex("/upload/drive/v3/files.*"))
        .respond_with(ResponseTemplate::new(500).set_body_string("server error"))
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    let result = storage.upload("test.bin", b"data").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn download_failure() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/drive/v3/files/bad_id"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    let result = storage.download("bad_id").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn delete_file() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/drive/v3/files/file_to_delete"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    storage.delete("file_to_delete").await.unwrap();
}

#[tokio::test]
async fn delete_already_gone_is_ok() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/drive/v3/files/already_gone"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    // 404 on delete should be treated as success
    storage.delete("already_gone").await.unwrap();
}

#[tokio::test]
async fn delete_server_error() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/drive/v3/files/fail"))
        .respond_with(ResponseTemplate::new(500).set_body_string("error"))
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    let result = storage.delete("fail").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn ensure_sync_folder_creates_missing_folders() {
    let server = MockServer::start().await;

    // First folder search (PrivStack) — not found
    // Second folder search (sync) — not found
    let search_counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let counter_clone = search_counter.clone();

    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .and(query_param("fields", "files(id,name)"))
        .respond_with(move |_req: &wiremock::Request| {
            let count = counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count == 0 {
                // First search: PrivStack not found
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"files": []}))
            } else {
                // Second search: sync not found
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"files": []}))
            }
        })
        .mount(&server)
        .await;

    // Create folder (called twice: PrivStack, then sync)
    let create_counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let create_clone = create_counter.clone();

    Mock::given(method("POST"))
        .and(path("/drive/v3/files"))
        .respond_with(move |_req: &wiremock::Request| {
            let count = create_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count == 0 {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "id": "created_privstack",
                    "name": "PrivStack",
                    "mimeType": "application/vnd.google-apps.folder"
                }))
            } else {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "id": "created_sync",
                    "name": "sync",
                    "mimeType": "application/vnd.google-apps.folder"
                }))
            }
        })
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    storage.ensure_sync_folder().await.unwrap();
}

#[tokio::test]
async fn ensure_sync_folder_create_fails() {
    let server = MockServer::start().await;

    // Folder not found
    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .and(query_param("fields", "files(id,name)"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"files": []})))
        .mount(&server)
        .await;

    // Create fails
    Mock::given(method("POST"))
        .and(path("/drive/v3/files"))
        .respond_with(ResponseTemplate::new(403).set_body_string("forbidden"))
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    let result = storage.ensure_sync_folder().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_changes_with_no_cursor() {
    let server = MockServer::start().await;

    // startPageToken endpoint
    Mock::given(method("GET"))
        .and(path("/drive/v3/changes/startPageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "startPageToken": "page_token_1"
        })))
        .mount(&server)
        .await;

    // changes endpoint
    Mock::given(method("GET"))
        .and(path("/drive/v3/changes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "changes": [
                {
                    "fileId": "file_1",
                    "removed": false,
                    "file": {
                        "id": "file_1",
                        "name": "doc.txt",
                        "mimeType": "text/plain",
                        "size": "100",
                        "modifiedTime": "2024-06-15T10:00:00Z",
                        "md5Checksum": "hash1"
                    }
                },
                {
                    "fileId": "file_2",
                    "removed": true
                }
            ],
            "newStartPageToken": "page_token_2"
        })))
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    let changes = storage.get_changes(None).await.unwrap();
    assert_eq!(changes.changed.len(), 1);
    assert_eq!(changes.changed[0].name, "doc.txt");
    assert_eq!(changes.deleted.len(), 1);
    assert_eq!(changes.deleted[0], "file_2");
    assert_eq!(changes.next_cursor, Some("page_token_2".to_string()));
}

#[tokio::test]
async fn get_changes_with_cursor() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/drive/v3/changes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "changes": [],
            "newStartPageToken": "next_cursor"
        })))
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    let changes = storage.get_changes(Some("existing_cursor")).await.unwrap();
    assert!(changes.changed.is_empty());
    assert!(changes.deleted.is_empty());
    assert_eq!(changes.next_cursor, Some("next_cursor".to_string()));
}

#[tokio::test]
async fn get_changes_failure() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/drive/v3/changes"))
        .respond_with(ResponseTemplate::new(500).set_body_string("fail"))
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    let result = storage.get_changes(Some("cursor")).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn list_files_server_error() {
    let server = MockServer::start().await;

    // Folder lookup succeeds
    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .and(query_param("fields", "files(id,name)"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files": [{"id": "folder", "name": "PrivStack", "mimeType": "application/vnd.google-apps.folder"}]
        })))
        .mount(&server)
        .await;

    // File listing fails
    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .and(query_param("pageSize", "100"))
        .respond_with(ResponseTemplate::new(500).set_body_string("error"))
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    let result = storage.list_files().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn list_files_with_results() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files": [
                {
                    "id": "f1",
                    "name": "notes.json",
                    "mimeType": "application/json",
                    "size": "256",
                    "modifiedTime": "2024-03-01T12:00:00Z",
                    "md5Checksum": "md5abc"
                }
            ]
        })))
        .expect(1..)
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    let files = storage.list_files().await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "notes.json");
    assert_eq!(files[0].id, "f1");
    assert_eq!(files[0].size, 256);
    assert_eq!(files[0].content_hash, Some("md5abc".to_string()));
}

// ── Token refresh flow ──────────────────────────────────────────

#[tokio::test]
async fn token_refresh_on_expired_token() {
    let server = MockServer::start().await;

    // complete_auth gives us an already-expired token (expires_in < 60s buffer)
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "expired_token",
            "refresh_token": "my_refresh",
            "expires_in": 1  // minus 60s buffer = already expired
        })))
        .expect(1..)
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let mut storage = GoogleDriveStorage::new(config);

    // complete_auth sets token with expires_at = now (expires_in:0 saturating_sub 60 = 0)
    storage.complete_auth("code").await.unwrap();
    assert!(storage.is_authenticated());

    // Wait so SystemTime::now() > expires_at
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // File list endpoint
    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files": [{"id": "folder", "name": "PrivStack", "mimeType": "application/vnd.google-apps.folder"}]
        })))
        .expect(1..)
        .mount(&server)
        .await;

    // This triggers get_access_token -> sees expired -> calls refresh_token
    let result = storage.list_files().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn token_refresh_failure() {
    let server = MockServer::start().await;

    // complete_auth gives expired token
    let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let counter = call_count.clone();

    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(move |_req: &wiremock::Request| {
            let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n == 0 {
                // First call: complete_auth
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "access_token": "expired",
                    "refresh_token": "my_refresh",
                    "expires_in": 1
                }))
            } else {
                // Second call: refresh fails
                ResponseTemplate::new(401).set_body_json(serde_json::json!({
                    "error": "invalid_grant"
                }))
            }
        })
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let mut storage = GoogleDriveStorage::new(config);
    storage.complete_auth("code").await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"files": []})))
        .mount(&server)
        .await;

    let result = storage.list_files().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn token_refresh_no_refresh_token() {
    let server = MockServer::start().await;

    // complete_auth gives expired token WITHOUT refresh_token
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "expired",
            "expires_in": 1
            // no refresh_token!
        })))
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let mut storage = GoogleDriveStorage::new(config);
    storage.complete_auth("code").await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"files": []})))
        .mount(&server)
        .await;

    let result = storage.list_files().await;
    assert!(result.is_err());
}

// ── Folder cache hit ────────────────────────────────────────────

#[tokio::test]
async fn ensure_sync_folder_caches_result() {
    let server = MockServer::start().await;

    // Folder exists
    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files": [{"id": "cached_folder", "name": "PrivStack", "mimeType": "application/vnd.google-apps.folder"}]
        })))
        .expect(1..)
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    // First call populates cache
    storage.ensure_sync_folder().await.unwrap();
    // Second call should hit cache (folder ID already stored)
    storage.ensure_sync_folder().await.unwrap();
}

// ── Upload with no size/modifiedTime in response ────────────────

#[tokio::test]
async fn upload_response_missing_optional_fields() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files": [{"id": "folder", "name": "PrivStack", "mimeType": "application/vnd.google-apps.folder"}]
        })))
        .expect(1..)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path_regex("/upload/drive/v3/files.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "f_new",
            "name": "data.bin",
            "mimeType": "application/octet-stream"
            // no size, modifiedTime, md5Checksum
        })))
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    let file = storage.upload("data.bin", b"content").await.unwrap();
    assert_eq!(file.id, "f_new");
    assert_eq!(file.size, 0); // default when missing
    assert!(file.content_hash.is_none());
}

// ── Pagination for list_files ───────────────────────────────────

#[tokio::test]
async fn list_files_pagination() {
    let server = MockServer::start().await;

    // Folder lookup
    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .and(query_param("fields", "files(id,name)"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files": [{"id": "folder", "name": "PrivStack", "mimeType": "application/vnd.google-apps.folder"}]
        })))
        .mount(&server)
        .await;

    let page_counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let counter_clone = page_counter.clone();

    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .and(query_param("pageSize", "100"))
        .respond_with(move |_req: &wiremock::Request| {
            let n = counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n == 0 {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "files": [{
                        "id": "f1", "name": "a.txt", "mimeType": "text/plain",
                        "size": "10", "modifiedTime": "2024-01-01T00:00:00Z"
                    }],
                    "nextPageToken": "page2"
                }))
            } else {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "files": [{
                        "id": "f2", "name": "b.txt", "mimeType": "text/plain",
                        "size": "20", "modifiedTime": "2024-02-01T00:00:00Z"
                    }]
                }))
            }
        })
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    let files = storage.list_files().await.unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].name, "a.txt");
    assert_eq!(files[1].name, "b.txt");
}

// ── Pagination for get_changes ──────────────────────────────────

#[tokio::test]
async fn get_changes_pagination() {
    let server = MockServer::start().await;

    let page_counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let counter_clone = page_counter.clone();

    Mock::given(method("GET"))
        .and(path("/drive/v3/changes"))
        .respond_with(move |_req: &wiremock::Request| {
            let n = counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n == 0 {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "changes": [{
                        "fileId": "f1",
                        "removed": false,
                        "file": {
                            "id": "f1", "name": "page1.txt", "mimeType": "text/plain",
                            "size": "5", "modifiedTime": "2024-01-01T00:00:00Z"
                        }
                    }],
                    "nextPageToken": "page2"
                }))
            } else {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "changes": [{
                        "fileId": "f2",
                        "removed": true
                    }],
                    "newStartPageToken": "final_cursor"
                }))
            }
        })
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    let changes = storage.get_changes(Some("start")).await.unwrap();
    assert_eq!(changes.changed.len(), 1);
    assert_eq!(changes.changed[0].name, "page1.txt");
    assert_eq!(changes.deleted.len(), 1);
    assert_eq!(changes.deleted[0], "f2");
    assert_eq!(changes.next_cursor, Some("final_cursor".to_string()));
}

// ── complete_auth without expires_in ────────────────────────────

#[tokio::test]
async fn complete_auth_no_expires_in() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "token_no_expiry",
            "refresh_token": "refresh_no_expiry"
            // no expires_in
        })))
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let mut storage = GoogleDriveStorage::new(config);
    storage.complete_auth("code").await.unwrap();
    assert!(storage.is_authenticated());
}

// ── refresh preserves old refresh_token if new one absent ───────

#[tokio::test]
async fn refresh_preserves_refresh_token() {
    let server = MockServer::start().await;

    let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let counter = call_count.clone();

    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(move |_req: &wiremock::Request| {
            let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n == 0 {
                // complete_auth: gives expired token with refresh
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "access_token": "expired",
                    "refresh_token": "original_refresh",
                    "expires_in": 1
                }))
            } else {
                // refresh: returns new access token WITHOUT new refresh_token, and no expires_in
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "access_token": "new_access"
                    // no refresh_token, no expires_in
                }))
            }
        })
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files": [{"id": "folder", "name": "PrivStack", "mimeType": "application/vnd.google-apps.folder"}]
        })))
        .expect(1..)
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let mut storage = GoogleDriveStorage::new(config);
    storage.complete_auth("code").await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Triggers refresh flow
    let result = storage.list_files().await;
    assert!(result.is_ok());
}

// ── get_or_create_sync_folder with empty path segments ──────────

#[tokio::test]
async fn sync_folder_with_leading_slash() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files": [{"id": "folder_id", "name": "sync", "mimeType": "application/vnd.google-apps.folder"}]
        })))
        .expect(1..)
        .mount(&server)
        .await;

    let mut config = mock_config(&server);
    config.base.sync_folder = "/sync".to_string(); // leading slash -> empty first segment
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    storage.ensure_sync_folder().await.unwrap();
}

// ── drive_file_to_cloud_file with unparseable size ──────────────

#[tokio::test]
async fn list_files_with_unparseable_size() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files": [{
                "id": "f_bad_size",
                "name": "bad.txt",
                "mimeType": "text/plain",
                "size": "not_a_number",
                "modifiedTime": "2024-01-01T00:00:00Z"
            }]
        })))
        .expect(1..)
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    let files = storage.list_files().await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].size, 0); // unparseable falls back to 0
}

// ── CloudFile trait impls ────────────────────────────────────────

#[tokio::test]
async fn cloud_file_debug_clone_serde() {
    use privstack_sync::cloud::CloudFile;
    use std::time::SystemTime;

    let file = CloudFile {
        id: "abc".to_string(),
        name: "test.txt".to_string(),
        path: "/sync/test.txt".to_string(),
        size: 1024,
        modified_at: SystemTime::now(),
        content_hash: Some("md5hash".to_string()),
    };

    // Debug
    let debug = format!("{:?}", file);
    assert!(debug.contains("abc"));
    assert!(debug.contains("test.txt"));

    // Clone
    let cloned = file.clone();
    assert_eq!(cloned.id, "abc");
    assert_eq!(cloned.name, "test.txt");
    assert_eq!(cloned.path, "/sync/test.txt");
    assert_eq!(cloned.size, 1024);
    assert_eq!(cloned.content_hash, Some("md5hash".to_string()));

    // Serialize
    let json = serde_json::to_string(&file).unwrap();
    assert!(json.contains("abc"));

    // Deserialize
    let deserialized: CloudFile = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, "abc");
    assert_eq!(deserialized.size, 1024);
}

#[test]
fn cloud_file_without_content_hash() {
    use privstack_sync::cloud::CloudFile;
    use std::time::SystemTime;

    let file = CloudFile {
        id: "x".to_string(),
        name: "n".to_string(),
        path: "p".to_string(),
        size: 0,
        modified_at: SystemTime::now(),
        content_hash: None,
    };
    let json = serde_json::to_string(&file).unwrap();
    let back: CloudFile = serde_json::from_str(&json).unwrap();
    assert!(back.content_hash.is_none());
}

// ── ChangeSet trait impls ───────────────────────────────────────

#[test]
fn changeset_debug_clone() {
    use privstack_sync::cloud::storage::ChangeSet;
    use privstack_sync::cloud::CloudFile;
    use std::time::SystemTime;

    let cs = ChangeSet {
        changed: vec![CloudFile {
            id: "f1".to_string(),
            name: "a.txt".to_string(),
            path: "/a.txt".to_string(),
            size: 10,
            modified_at: SystemTime::now(),
            content_hash: None,
        }],
        deleted: vec!["d1".to_string()],
        next_cursor: Some("cursor".to_string()),
    };

    let debug = format!("{:?}", cs);
    assert!(debug.contains("f1"));
    assert!(debug.contains("d1"));
    assert!(debug.contains("cursor"));

    let cloned = cs.clone();
    assert_eq!(cloned.changed.len(), 1);
    assert_eq!(cloned.deleted.len(), 1);
    assert_eq!(cloned.next_cursor, Some("cursor".to_string()));
}

#[test]
fn changeset_empty() {
    use privstack_sync::cloud::storage::ChangeSet;

    let cs = ChangeSet {
        changed: vec![],
        deleted: vec![],
        next_cursor: None,
    };
    let debug = format!("{:?}", cs);
    assert!(debug.contains("ChangeSet"));
    let cloned = cs.clone();
    assert!(cloned.changed.is_empty());
}

// ── CloudStorageConfig trait impls ──────────────────────────────

#[test]
fn cloud_storage_config_debug_clone_serde() {
    use privstack_sync::cloud::CloudStorageConfig;

    let cfg = CloudStorageConfig {
        sync_folder: "custom/folder".to_string(),
        poll_interval_secs: 60,
        max_file_size: 100,
    };

    // Debug
    let debug = format!("{:?}", cfg);
    assert!(debug.contains("custom/folder"));

    // Clone
    let cloned = cfg.clone();
    assert_eq!(cloned.sync_folder, "custom/folder");
    assert_eq!(cloned.poll_interval_secs, 60);
    assert_eq!(cloned.max_file_size, 100);

    // Serialize round-trip
    let json = serde_json::to_string(&cfg).unwrap();
    let back: CloudStorageConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.sync_folder, "custom/folder");
    assert_eq!(back.poll_interval_secs, 60);
}

// ── GoogleDriveConfig Serialize ─────────────────────────────────

#[test]
fn google_drive_config_serialize() {
    let cfg = GoogleDriveConfig {
        client_id: "id".to_string(),
        client_secret: "secret".to_string(),
        redirect_uri: "http://redir".to_string(),
        base: privstack_sync::cloud::CloudStorageConfig {
            sync_folder: "test/sync".to_string(),
            poll_interval_secs: 10,
            max_file_size: 999,
        },
        api_base_url: "http://api".to_string(),
        oauth_base_url: "http://oauth".to_string(),
        auth_base_url: "http://auth".to_string(),
    };
    let json = serde_json::to_string(&cfg).unwrap();
    assert!(json.contains("test/sync"));
    assert!(json.contains("http://api"));
}

// ── authenticate URL contains encoded params ────────────────────

#[tokio::test]
async fn authenticate_url_contains_scope_and_redirect() {
    let config = GoogleDriveConfig {
        client_id: "my+special&id".to_string(),
        client_secret: "secret".to_string(),
        redirect_uri: "http://localhost:8080/callback".to_string(),
        ..Default::default()
    };
    let mut storage = GoogleDriveStorage::new(config);
    let url = storage.authenticate().await.unwrap().unwrap();
    // URL-encoded client_id
    assert!(url.contains("my%2Bspecial%26id"));
    assert!(url.contains("drive.file"));
    assert!(url.contains("access_type=offline"));
    assert!(url.contains("prompt=consent"));
    assert!(url.contains("response_type=code"));
}

// ── drive_file_to_cloud_file with invalid modifiedTime ──────────

#[tokio::test]
async fn list_files_with_invalid_modified_time() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files": [{
                "id": "f_bad_time",
                "name": "badtime.txt",
                "mimeType": "text/plain",
                "size": "42",
                "modifiedTime": "not-a-date"
            }]
        })))
        .expect(1..)
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    let files = storage.list_files().await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "badtime.txt");
    assert_eq!(files[0].size, 42);
    // modified_at falls back to SystemTime::now(), just ensure it doesn't panic
}

// ── get_changes: change with removed=None and no file ───────────

#[tokio::test]
async fn get_changes_with_no_file_and_not_removed() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/drive/v3/changes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "changes": [{
                "fileId": "orphan",
                "removed": false
                // no "file" field
            }],
            "newStartPageToken": "cursor_2"
        })))
        .mount(&server)
        .await;

    let config = mock_config(&server);
    let storage = GoogleDriveStorage::new(config);
    storage.set_tokens("token".to_string(), None).await;

    let changes = storage.get_changes(Some("cursor_1")).await.unwrap();
    // Change has removed=false but no file -> skipped (neither changed nor deleted)
    assert!(changes.changed.is_empty());
    assert!(changes.deleted.is_empty());
    assert_eq!(changes.next_cursor, Some("cursor_2".to_string()));
}
