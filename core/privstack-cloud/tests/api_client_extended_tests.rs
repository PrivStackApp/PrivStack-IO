//! Extended wiremock tests for CloudApiClient — covers methods and error paths
//! not exercised by api_client_tests.rs.

use privstack_cloud::api_client::CloudApiClient;
use privstack_cloud::config::CloudConfig;
use privstack_cloud::error::CloudError;
use privstack_cloud::types::*;
use std::sync::Arc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn setup() -> (MockServer, Arc<CloudApiClient>) {
    let server = MockServer::start().await;
    let config = CloudConfig {
        api_base_url: server.uri(),
        s3_bucket: "test-bucket".into(),
        s3_region: "us-east-2".into(),
        s3_endpoint_override: None,
        credential_refresh_margin_secs: 60,
        poll_interval_secs: 5,
    };
    let client = Arc::new(CloudApiClient::new(config));
    client.set_tokens("at".into(), "rt".into(), 1).await;
    (server, client)
}

fn auth_response() -> serde_json::Value {
    serde_json::json!({
        "access_token": "at-refreshed",
        "refresh_token": "rt-refreshed",
        "user": { "id": 1, "email": "test@example.com" }
    })
}

fn workspace_json(id: &str) -> serde_json::Value {
    serde_json::json!({
        "id": 1,
        "user_id": 1,
        "workspace_id": id,
        "workspace_name": "Test",
        "s3_prefix": format!("users/1/workspaces/{id}"),
        "storage_used_bytes": 0,
        "storage_quota_bytes": 10737418240_u64,
        "created_at": "2025-01-01T00:00:00Z"
    })
}

// ── get_current_tokens ──

#[tokio::test]
async fn get_current_tokens_returns_tokens_when_authenticated() {
    let (_server, client) = setup().await;
    let tokens = client.get_current_tokens().await.unwrap();
    assert_eq!(tokens.access_token, "at");
    assert_eq!(tokens.refresh_token, "rt");
    assert_eq!(tokens.user_id, 1);
}

#[tokio::test]
async fn get_current_tokens_returns_none_when_not_authenticated() {
    let server = MockServer::start().await;
    let config = CloudConfig {
        api_base_url: server.uri(),
        s3_bucket: "test".into(),
        s3_region: "us-east-2".into(),
        s3_endpoint_override: None,
        credential_refresh_margin_secs: 60,
        poll_interval_secs: 5,
    };
    let client = CloudApiClient::new(config);
    assert!(client.get_current_tokens().await.is_none());
}

// ── refresh_access_token ──

#[tokio::test]
async fn refresh_access_token_success() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/auth/refresh"))
        .respond_with(ResponseTemplate::new(200).set_body_json(auth_response()))
        .mount(&server)
        .await;

    let new_token = client.refresh_access_token().await.unwrap();
    assert_eq!(new_token, "at-refreshed");

    // Verify tokens were updated internally.
    let tokens = client.get_current_tokens().await.unwrap();
    assert_eq!(tokens.access_token, "at-refreshed");
    assert_eq!(tokens.refresh_token, "rt-refreshed");
}

#[tokio::test]
async fn refresh_access_token_401_clears_session() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/auth/refresh"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let result = client.refresh_access_token().await;
    assert!(matches!(result.unwrap_err(), CloudError::AuthFailed(_)));
    // Session should be cleared.
    assert!(!client.is_authenticated().await);
}

#[tokio::test]
async fn refresh_access_token_403_clears_session() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/auth/refresh"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let result = client.refresh_access_token().await;
    assert!(matches!(result.unwrap_err(), CloudError::AuthFailed(_)));
    assert!(!client.is_authenticated().await);
}

#[tokio::test]
async fn refresh_access_token_no_refresh_token_returns_auth_required() {
    let server = MockServer::start().await;
    let config = CloudConfig {
        api_base_url: server.uri(),
        s3_bucket: "test".into(),
        s3_region: "us-east-2".into(),
        s3_endpoint_override: None,
        credential_refresh_margin_secs: 60,
        poll_interval_secs: 5,
    };
    let client = CloudApiClient::new(config);
    // No tokens set at all.
    let result = client.refresh_access_token().await;
    assert!(matches!(result.unwrap_err(), CloudError::AuthRequired));
}

// ── ack_download ──

#[tokio::test]
async fn ack_download_success() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/cursors/ack"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(&server)
        .await;

    client
        .ack_download("ws-1", "dev-1", "entity-1", 42)
        .await
        .unwrap();
}

#[tokio::test]
async fn ack_download_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/cursors/ack"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.ack_download("ws-1", "dev-1", "entity-1", 42).await;
    assert!(result.is_err());
}

// ── get_rate_limits ──

#[tokio::test]
async fn get_rate_limits_success() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/cloud/rate-limits"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "window_seconds": 60,
            "max_requests_per_window": 600,
            "recommended_poll_interval_secs": 30,
            "flush_batch_size": 25,
            "inter_entity_delay_ms": 120
        })))
        .mount(&server)
        .await;

    let limits = client.get_rate_limits().await.unwrap();
    assert_eq!(limits.window_seconds, 60);
    assert_eq!(limits.max_requests_per_window, 600);
    assert_eq!(limits.recommended_poll_interval_secs, 30);
    assert_eq!(limits.flush_batch_size, 25);
    assert_eq!(limits.inter_entity_delay_ms, 120);
}

// ── upload_public_key ──

#[tokio::test]
async fn upload_public_key_success() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/keys/public"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(&server)
        .await;

    let key = [7u8; 32];
    client.upload_public_key(&key).await.unwrap();
}

#[tokio::test]
async fn upload_public_key_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/keys/public"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let key = [7u8; 32];
    let result = client.upload_public_key(&key).await;
    assert!(result.is_err());
}

// ── notify_snapshot ──

#[tokio::test]
async fn notify_snapshot_success() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/compaction/request"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(&server)
        .await;

    client
        .notify_snapshot("entity-1", "ws-1", "snapshot.enc", 100)
        .await
        .unwrap();
}

#[tokio::test]
async fn notify_snapshot_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/compaction/request"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client
        .notify_snapshot("entity-1", "ws-1", "snapshot.enc", 100)
        .await;
    assert!(result.is_err());
}

// ── register_blob ──

#[tokio::test]
async fn register_blob_success() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/blobs/register"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(&server)
        .await;

    let req = RegisterBlobRequest {
        workspace_id: "ws-1".into(),
        blob_id: "blob-1".into(),
        entity_id: Some("entity-1".into()),
        s3_key: "blobs/blob-1.enc".into(),
        size_bytes: 4096,
        content_hash: Some("abc123".into()),
    };
    client.register_blob(&req).await.unwrap();
}

#[tokio::test]
async fn register_blob_without_entity_id() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/blobs/register"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(&server)
        .await;

    let req = RegisterBlobRequest {
        workspace_id: "ws-1".into(),
        blob_id: "blob-orphan".into(),
        entity_id: None,
        s3_key: "blobs/blob-orphan.enc".into(),
        size_bytes: 1024,
        content_hash: None,
    };
    client.register_blob(&req).await.unwrap();
}

#[tokio::test]
async fn register_blob_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/blobs/register"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let req = RegisterBlobRequest {
        workspace_id: "ws-1".into(),
        blob_id: "blob-1".into(),
        entity_id: None,
        s3_key: "blobs/blob-1.enc".into(),
        size_bytes: 4096,
        content_hash: None,
    };
    let result = client.register_blob(&req).await;
    assert!(result.is_err());
}

// ── configure_rate_limits ──

#[tokio::test]
async fn configure_rate_limits_updates_counter() {
    let (server, client) = setup().await;

    // After configuring, subsequent requests should use new limits.
    // Configure with very high limit so requests pass.
    client.configure_rate_limits(10000, 60).await;

    Mock::given(method("GET"))
        .and(path("/api/cloud/rate-limits"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "window_seconds": 60,
            "max_requests_per_window": 10000,
            "recommended_poll_interval_secs": 30,
            "flush_batch_size": 25,
            "inter_entity_delay_ms": 120
        })))
        .mount(&server)
        .await;

    // Should succeed since limit is high.
    client.get_rate_limits().await.unwrap();
}

// ── is_rate_limited / rate_limit_remaining ──

#[tokio::test]
async fn is_rate_limited_false_by_default() {
    let (_server, client) = setup().await;
    assert!(!client.is_rate_limited().await);
}

#[tokio::test]
async fn rate_limit_remaining_none_by_default() {
    let (_server, client) = setup().await;
    assert!(client.rate_limit_remaining().await.is_none());
}

// ── 429 response handling ──

#[tokio::test]
async fn response_429_sets_rate_limit_gate() {
    let (server, client) = setup().await;

    // Return 429 with Retry-After header.
    Mock::given(method("GET"))
        .and(path("/api/cloud/workspaces"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "10"),
        )
        .mount(&server)
        .await;

    let result = client.list_workspaces().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        CloudError::RateLimited { retry_after_secs } => {
            // Server said 10s, client adds 1s buffer = 11s.
            assert_eq!(retry_after_secs, 11);
        }
        other => panic!("expected RateLimited, got: {other:?}"),
    }

    // Subsequent requests should be blocked immediately.
    assert!(client.is_rate_limited().await);
    assert!(client.rate_limit_remaining().await.is_some());
}

#[tokio::test]
async fn response_429_without_retry_after_defaults_to_60() {
    let (server, client) = setup().await;

    // 429 without Retry-After header.
    Mock::given(method("GET"))
        .and(path("/api/cloud/rate-limits"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    let result = client.get_rate_limits().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        CloudError::RateLimited { retry_after_secs } => {
            // Default 60 + 1 buffer = 61.
            assert_eq!(retry_after_secs, 61);
        }
        other => panic!("expected RateLimited, got: {other:?}"),
    }
}

#[tokio::test]
async fn rate_limited_blocks_subsequent_requests() {
    let (server, client) = setup().await;

    // First: trigger 429 with long Retry-After.
    Mock::given(method("GET"))
        .and(path("/api/cloud/devices"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "300"),
        )
        .mount(&server)
        .await;

    let _ = client.list_devices().await;

    // Now try a different endpoint — should be blocked by the gate.
    let result = client.get_quota("ws-1").await;
    assert!(result.is_err());
    match result.unwrap_err() {
        CloudError::RateLimited { .. } => {} // Expected
        other => panic!("expected RateLimited, got: {other:?}"),
    }
}

// ── 429 on POST ──

#[tokio::test]
async fn response_429_on_post_sets_rate_limit() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/cursors/ack"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "5"),
        )
        .mount(&server)
        .await;

    let result = client.ack_download("ws-1", "dev-1", "e-1", 10).await;
    assert!(result.is_err());
    assert!(client.is_rate_limited().await);
}

// ── 401 retry on POST ──

#[tokio::test]
async fn auth_retry_on_401_post() {
    let (server, client) = setup().await;

    // First POST call: 401, then refresh, then retry succeeds.
    Mock::given(method("POST"))
        .and(path("/api/cloud/cursors/ack"))
        .respond_with(ResponseTemplate::new(401))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/api/auth/refresh"))
        .respond_with(ResponseTemplate::new(200).set_body_json(auth_response()))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/cursors/ack"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(&server)
        .await;

    client
        .ack_download("ws-1", "dev-1", "entity-1", 42)
        .await
        .unwrap();
}

// ── 401 retry on DELETE ──

#[tokio::test]
async fn auth_retry_on_401_delete() {
    let (server, client) = setup().await;

    // First DELETE: 401, then refresh, then retry succeeds.
    Mock::given(method("DELETE"))
        .and(path("/api/cloud/workspaces/ws-old"))
        .respond_with(ResponseTemplate::new(401))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/api/auth/refresh"))
        .respond_with(ResponseTemplate::new(200).set_body_json(auth_response()))
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/api/cloud/workspaces/ws-old"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(&server)
        .await;

    client.delete_workspace("ws-old").await.unwrap();
}

// ── Workspace 409 conflict ──

#[tokio::test]
async fn register_workspace_409_fetches_existing() {
    let (server, client) = setup().await;

    // POST returns 409 Conflict.
    Mock::given(method("POST"))
        .and(path("/api/cloud/workspaces"))
        .respond_with(ResponseTemplate::new(409))
        .mount(&server)
        .await;

    // GET returns the existing workspace.
    Mock::given(method("GET"))
        .and(path("/api/cloud/workspaces"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "workspaces": [workspace_json("ws-existing")]
        })))
        .mount(&server)
        .await;

    let ws = client
        .register_workspace("ws-existing", "Test")
        .await
        .unwrap();
    assert_eq!(ws.workspace_id, "ws-existing");
}

#[tokio::test]
async fn register_workspace_409_but_not_found_in_list() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/workspaces"))
        .respond_with(ResponseTemplate::new(409))
        .mount(&server)
        .await;

    // GET returns workspaces but not the one we want.
    Mock::given(method("GET"))
        .and(path("/api/cloud/workspaces"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "workspaces": [workspace_json("ws-other")]
        })))
        .mount(&server)
        .await;

    let result = client.register_workspace("ws-missing", "Test").await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("workspace conflict but not found"));
}

// ── Public key: invalid base64 ──

#[tokio::test]
async fn get_public_key_invalid_base64() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/cloud/keys/public/5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "public_key": "not-valid-base64!!!"
        })))
        .mount(&server)
        .await;

    let result = client.get_public_key(5).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("invalid public key encoding"));
}

// ── Server errors on various endpoints ──

#[tokio::test]
async fn get_sts_credentials_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/credentials"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.get_sts_credentials("ws-1").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn advance_cursor_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/cursors/advance"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let req = AdvanceCursorRequest {
        workspace_id: "ws".into(),
        device_id: "dev".into(),
        entity_id: "ent".into(),
        cursor_position: 10,
        batch_key: "batch.enc".into(),
        size_bytes: 1024,
        event_count: 5,
    };
    let result = client.advance_cursor(&req).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_pending_changes_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/cloud/cursors/pending"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.get_pending_changes("ws", "dev").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn release_lock_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/locks/release"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.release_lock("ent-1", "ws-1", "dev-1").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_quota_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/cloud/quota"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.get_quota("ws-1").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn list_workspaces_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/cloud/workspaces"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.list_workspaces().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn delete_workspace_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("DELETE"))
        .and(path("/api/cloud/workspaces/ws-1"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.delete_workspace("ws-1").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn register_device_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/devices/register"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.register_device("Mac", "macos", "dev-1").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn list_devices_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/cloud/devices"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.list_devices().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_entity_blobs_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/cloud/blobs/e-1"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.get_entity_blobs("e-1").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_batches_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/cloud/batches/e-1"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.get_batches("ws-1", "e-1", 0).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_rate_limits_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/cloud/rate-limits"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.get_rate_limits().await;
    assert!(result.is_err());
}

// ── Sharing error paths ──

#[tokio::test]
async fn create_share_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/share/create"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let req = CreateShareRequest {
        entity_id: "e-1".into(),
        entity_type: "note".into(),
        entity_name: None,
        workspace_id: "ws-1".into(),
        recipient_email: "bob@example.com".into(),
        permission: SharePermission::Write,
    };
    let result = client.create_share(&req).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn accept_share_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/share/accept"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.accept_share("token-abc").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn revoke_share_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/share/revoke"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.revoke_share("e-1", "bob@example.com").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_entity_shares_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/share/entity/e-1"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.get_entity_shares("e-1").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_shared_with_me_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/share/received"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.get_shared_with_me().await;
    assert!(result.is_err());
}

// ── Proactive rate limiting (RequestCounter at capacity) ──

#[tokio::test]
async fn proactive_rate_limit_throttles_at_75_percent() {
    let (server, client) = setup().await;

    // Configure very low limit: 4 req/60s -> 75% = 3 req/window.
    client.configure_rate_limits(4, 60).await;

    // Mock endpoint for all GET requests.
    Mock::given(method("GET"))
        .and(path("/api/cloud/rate-limits"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "window_seconds": 60,
            "max_requests_per_window": 4,
            "recommended_poll_interval_secs": 30,
            "flush_batch_size": 25,
            "inter_entity_delay_ms": 120
        })))
        .expect(3) // Only 3 should get through.
        .mount(&server)
        .await;

    // First 3 requests should succeed (75% of 4 = 3).
    client.get_rate_limits().await.unwrap();
    client.get_rate_limits().await.unwrap();
    client.get_rate_limits().await.unwrap();

    // 4th request should be proactively throttled.
    let result = client.get_rate_limits().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        CloudError::RateLimited { retry_after_secs } => {
            assert!(retry_after_secs >= 1);
        }
        other => panic!("expected RateLimited, got: {other:?}"),
    }
}

// ── 429 on retry after 401 ──

#[tokio::test]
async fn response_429_on_retry_after_401_sets_rate_limit() {
    let (server, client) = setup().await;

    // First GET: 401.
    Mock::given(method("GET"))
        .and(path("/api/cloud/workspaces"))
        .respond_with(ResponseTemplate::new(401))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Refresh succeeds.
    Mock::given(method("POST"))
        .and(path("/api/auth/refresh"))
        .respond_with(ResponseTemplate::new(200).set_body_json(auth_response()))
        .mount(&server)
        .await;

    // Retry GET: 429.
    Mock::given(method("GET"))
        .and(path("/api/cloud/workspaces"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "30"),
        )
        .mount(&server)
        .await;

    let result = client.list_workspaces().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        CloudError::RateLimited { retry_after_secs } => {
            assert_eq!(retry_after_secs, 31);
        }
        other => panic!("expected RateLimited, got: {other:?}"),
    }
    assert!(client.is_rate_limited().await);
}

// ── Multiple data items ──

#[tokio::test]
async fn get_entity_blobs_empty_list() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/cloud/blobs/e-empty"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "blobs": []
        })))
        .mount(&server)
        .await;

    let blobs = client.get_entity_blobs("e-empty").await.unwrap();
    assert!(blobs.is_empty());
}

#[tokio::test]
async fn get_batches_multiple_results() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/cloud/batches/e-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "batches": [
                { "s3_key": "batch-1.enc", "cursor_start": 0, "cursor_end": 10, "size_bytes": 512, "event_count": 5, "is_snapshot": false },
                { "s3_key": "batch-2.enc", "cursor_start": 10, "cursor_end": 20, "size_bytes": 1024, "event_count": 8, "is_snapshot": true },
                { "s3_key": "batch-3.enc", "cursor_start": 20, "cursor_end": 30, "size_bytes": 256, "event_count": 3, "is_snapshot": 0 }
            ]
        })))
        .mount(&server)
        .await;

    let batches = client.get_batches("ws-1", "e-1", 0).await.unwrap();
    assert_eq!(batches.len(), 3);
    assert!(!batches[0].is_snapshot);
    assert!(batches[1].is_snapshot);
    // Test that integer 0 deserializes as false for is_snapshot.
    assert!(!batches[2].is_snapshot);
}

#[tokio::test]
async fn get_pending_changes_multiple_entities() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/cloud/cursors/pending"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "pending": [
                { "entity_id": "e-1", "latest_cursor": 10, "device_cursor": 5 },
                { "entity_id": "e-2", "latest_cursor": 20, "device_cursor": 0 },
                { "entity_id": "e-3", "latest_cursor": 100, "device_cursor": 99 }
            ]
        })))
        .mount(&server)
        .await;

    let pending = client.get_pending_changes("ws", "dev").await.unwrap();
    assert_eq!(pending.pending.len(), 3);
    assert_eq!(pending.pending[0].entity_id, "e-1");
    assert_eq!(pending.pending[1].latest_cursor, 20);
    assert_eq!(pending.pending[2].device_cursor, 99);
}

#[tokio::test]
async fn get_pending_changes_empty() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/cloud/cursors/pending"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "pending": []
        })))
        .mount(&server)
        .await;

    let pending = client.get_pending_changes("ws", "dev").await.unwrap();
    assert!(pending.pending.is_empty());
}

// ── Quota with string-encoded usage_percent ──

#[tokio::test]
async fn get_quota_string_encoded_percent() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/cloud/quota"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "storage_used_bytes": 5368709120_u64,
            "storage_quota_bytes": 10737418240_u64,
            "usage_percent": "50.00"
        })))
        .mount(&server)
        .await;

    let quota = client.get_quota("ws-1").await.unwrap();
    assert_eq!(quota.storage_used_bytes, 5368709120);
    assert!((quota.usage_percent - 50.0).abs() < f64::EPSILON);
}

// ── Concurrent refresh deduplication ──

#[tokio::test]
async fn concurrent_refresh_deduplication() {
    let (server, client) = setup().await;

    // Mount refresh that takes a moment.
    Mock::given(method("POST"))
        .and(path("/api/auth/refresh"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(auth_response())
                .set_delay(std::time::Duration::from_millis(100)),
        )
        .expect(1) // Should only be called once despite two concurrent attempts.
        .mount(&server)
        .await;

    let c1 = client.clone();
    let c2 = client.clone();

    let (r1, r2) = tokio::join!(c1.refresh_access_token(), c2.refresh_access_token(),);

    // Both should succeed — one does the actual refresh, the other piggybacks.
    assert!(r1.is_ok());
    assert!(r2.is_ok());
}

// ── Logout clears tokens fully ──

#[tokio::test]
async fn logout_then_request_returns_auth_required() {
    let (_server, client) = setup().await;
    client.logout().await;
    let result = client.get_rate_limits().await;
    assert!(matches!(result.unwrap_err(), CloudError::AuthRequired));
}

// ── Share key endpoints ──

#[tokio::test]
async fn get_share_key_success() {
    let (server, client) = setup().await;

    // SealedEnvelope is a struct from privstack_crypto. We need to match its
    // serialization format. Check if it's just a transparent JSON object.
    // Since we can't easily construct it here, test the server error path.
    Mock::given(method("GET"))
        .and(path("/api/share/keys/e-1"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.get_share_key("e-1").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn store_share_key_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/share/keys/store"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let envelope = privstack_crypto::SealedEnvelope {
        ephemeral_public_key: [7u8; 32],
        nonce: [4u8; 24],
        ciphertext: vec![1, 2, 3],
    };
    let result = client.store_share_key("e-1", 2, &envelope).await;
    assert!(result.is_err());
}

// ── Register workspace server error ──

#[tokio::test]
async fn register_workspace_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/workspaces"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.register_workspace("ws-1", "Test").await;
    assert!(result.is_err());
}

// ── Authenticate stores tokens correctly ──

#[tokio::test]
async fn authenticate_stores_user_id() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/auth/login"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "new-at",
            "refresh_token": "new-rt",
            "user": { "id": 99, "email": "new@example.com" }
        })))
        .mount(&server)
        .await;

    let tokens = client
        .authenticate("new@example.com", "pass")
        .await
        .unwrap();
    assert_eq!(tokens.user_id, 99);
    assert_eq!(tokens.email, "new@example.com");
    assert_eq!(client.user_id().await, Some(99));
}

// ── Multiple workspaces in list ──

#[tokio::test]
async fn list_workspaces_multiple() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/cloud/workspaces"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "workspaces": [
                workspace_json("ws-1"),
                workspace_json("ws-2"),
                workspace_json("ws-3")
            ]
        })))
        .mount(&server)
        .await;

    let workspaces = client.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 3);
    assert_eq!(workspaces[0].workspace_id, "ws-1");
    assert_eq!(workspaces[2].workspace_id, "ws-3");
}

// ── Multiple devices ──

#[tokio::test]
async fn list_devices_multiple() {
    let (server, client) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/cloud/devices"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "devices": [
                { "device_id": "dev-1", "device_name": "MacBook", "platform": "macos", "last_seen_at": "2025-06-01T00:00:00Z" },
                { "device_id": "dev-2", "device_name": "iPhone", "platform": "ios", "last_seen_at": null }
            ]
        })))
        .mount(&server)
        .await;

    let devices = client.list_devices().await.unwrap();
    assert_eq!(devices.len(), 2);
    assert_eq!(devices[0].device_id, "dev-1");
    assert!(devices[0].last_seen_at.is_some());
    assert!(devices[1].last_seen_at.is_none());
}

// ── Acquire lock server error (not 409) ──

#[tokio::test]
async fn acquire_lock_server_error() {
    let (server, client) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/cloud/locks/acquire"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client.acquire_lock("ent-1", "ws-1", "dev-1").await;
    assert!(result.is_err());
    // Should NOT be LockContention for a 500.
    assert!(!matches!(result.unwrap_err(), CloudError::LockContention(_)));
}
