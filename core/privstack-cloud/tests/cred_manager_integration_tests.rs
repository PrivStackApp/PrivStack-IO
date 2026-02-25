//! Integration tests for CredentialManager: caching, refresh, clear, has_valid_credentials.

use chrono::{Duration, Utc};
use privstack_cloud::api_client::CloudApiClient;
use privstack_cloud::config::CloudConfig;
use privstack_cloud::credential_manager::CredentialManager;
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

fn sts_response(expires_in_secs: i64) -> serde_json::Value {
    let expires_at = Utc::now() + Duration::seconds(expires_in_secs);
    serde_json::json!({
        "access_key_id": "AKIATEST",
        "secret_access_key": "secret",
        "session_token": "token",
        "expires_at": expires_at.to_rfc3339(),
        "bucket": "test-bucket",
        "region": "us-east-2"
    })
}

async fn mount_sts(server: &MockServer, expires_in_secs: i64, expected_calls: Option<u64>) {
    let mock = Mock::given(method("POST"))
        .and(path("/api/cloud/credentials"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(sts_response(expires_in_secs)),
        );
    let mock = if let Some(n) = expected_calls {
        mock.expect(n)
    } else {
        mock
    };
    mock.mount(server).await;
}

// ── has_valid_credentials ──────────────────────────────────────────────

#[tokio::test]
async fn no_valid_credentials_initially() {
    let (_server, api) = setup().await;
    let mgr = CredentialManager::new(api, "ws-1".into(), 300);
    assert!(!mgr.has_valid_credentials().await);
}

// ── get_credentials (fetch + cache) ────────────────────────────────────

#[tokio::test]
async fn get_credentials_fetches_on_first_call() {
    let (server, api) = setup().await;
    mount_sts(&server, 3600, Some(1)).await;

    let mgr = CredentialManager::new(api, "ws-1".into(), 300);
    let creds = mgr.get_credentials().await.unwrap();
    assert_eq!(creds.access_key_id, "AKIATEST");
    assert!(mgr.has_valid_credentials().await);
}

#[tokio::test]
async fn get_credentials_returns_cached_on_second_call() {
    let (server, api) = setup().await;
    mount_sts(&server, 3600, Some(1)).await;

    let mgr = CredentialManager::new(api, "ws-1".into(), 300);
    let c1 = mgr.get_credentials().await.unwrap();
    let c2 = mgr.get_credentials().await.unwrap();
    assert_eq!(c1.access_key_id, c2.access_key_id);
}

// ── refresh ────────────────────────────────────────────────────────────

#[tokio::test]
async fn refresh_always_calls_api() {
    let (server, api) = setup().await;
    mount_sts(&server, 3600, Some(2)).await;

    let mgr = CredentialManager::new(api, "ws-1".into(), 300);
    mgr.get_credentials().await.unwrap();
    let refreshed = mgr.refresh().await.unwrap();
    assert_eq!(refreshed.access_key_id, "AKIATEST");
}

// ── clear ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn clear_removes_cached_credentials() {
    let (server, api) = setup().await;
    mount_sts(&server, 3600, None).await;

    let mgr = CredentialManager::new(api, "ws-1".into(), 300);
    mgr.get_credentials().await.unwrap();
    assert!(mgr.has_valid_credentials().await);

    mgr.clear().await;
    assert!(!mgr.has_valid_credentials().await);
}

// ── expiry-driven refresh ──────────────────────────────────────────────

#[tokio::test]
async fn get_credentials_refreshes_when_near_expiry() {
    let (server, api) = setup().await;
    // Creds expire in 100s but margin is 300s → always within margin → must re-fetch
    mount_sts(&server, 100, Some(2)).await;

    let mgr = CredentialManager::new(api, "ws-1".into(), 300);
    mgr.get_credentials().await.unwrap();
    mgr.get_credentials().await.unwrap();
}

// ── error handling ─────────────────────────────────────────────────────

#[tokio::test]
async fn refresh_propagates_api_error() {
    let (server, api) = setup().await;
    Mock::given(method("POST"))
        .and(path("/api/cloud/credentials"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let mgr = CredentialManager::new(api, "ws-1".into(), 300);
    let err = mgr.refresh().await;
    assert!(err.is_err());
}
