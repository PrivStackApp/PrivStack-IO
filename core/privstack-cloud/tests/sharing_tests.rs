//! Tests for ShareManager: create, accept, revoke, get shares.

use privstack_cloud::api_client::CloudApiClient;
use privstack_cloud::config::CloudConfig;
use privstack_cloud::sharing::ShareManager;
use privstack_cloud::types::*;
use std::sync::Arc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn setup() -> (MockServer, Arc<CloudApiClient>, ShareManager) {
    let server = MockServer::start().await;
    let config = CloudConfig {
        api_base_url: server.uri(),
        s3_bucket: "test".into(),
        s3_region: "us-east-2".into(),
        s3_endpoint_override: None,
        credential_refresh_margin_secs: 60,
        poll_interval_secs: 5,
    };
    let api = Arc::new(CloudApiClient::new(config));
    api.set_tokens("at".into(), "rt".into(), 1).await;
    let mgr = ShareManager::new(api.clone());
    (server, api, mgr)
}

// ── create_share ───────────────────────────────────────────────────────

#[tokio::test]
async fn create_share_success() {
    let (server, _api, mgr) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/share/create"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "share_id": 1,
            "entity_id": "ent-1",
            "entity_type": "note",
            "entity_name": "My Note",
            "recipient_email": "bob@example.com",
            "permission": "read",
            "status": "pending",
            "created_at": "2026-01-01T00:00:00Z",
            "accepted_at": null
        })))
        .expect(1)
        .mount(&server)
        .await;

    let req = CreateShareRequest {
        entity_id: "ent-1".into(),
        entity_type: "note".into(),
        entity_name: Some("My Note".into()),
        recipient_email: "bob@example.com".into(),
        permission: SharePermission::Read,
        workspace_id: "ws-1".into(),
    };

    let share = mgr.create_share(&req).await.unwrap();
    assert_eq!(share.entity_id, "ent-1");
    assert_eq!(share.recipient_email, "bob@example.com");
}

// ── accept_share ───────────────────────────────────────────────────────

#[tokio::test]
async fn accept_share_success() {
    let (server, _api, mgr) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/share/accept"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    mgr.accept_share("tok-abc").await.unwrap();
}

// ── revoke_share ───────────────────────────────────────────────────────

#[tokio::test]
async fn revoke_share_success() {
    let (server, _api, mgr) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/share/revoke"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    mgr.revoke_share("ent-1", "bob@example.com").await.unwrap();
}

// ── get_entity_shares ──────────────────────────────────────────────────

#[tokio::test]
async fn get_entity_shares_returns_list() {
    let (server, _api, mgr) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/share/entity/ent-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "shares": [
                {
                    "share_id": 1,
                    "entity_id": "ent-1",
                    "entity_type": "note",
                    "entity_name": null,
                    "recipient_email": "bob@example.com",
                    "permission": "read",
                    "status": "accepted",
                    "created_at": "2026-01-01T00:00:00Z",
                    "accepted_at": "2026-01-01T12:00:00Z"
                },
                {
                    "share_id": 2,
                    "entity_id": "ent-1",
                    "entity_type": "note",
                    "entity_name": null,
                    "recipient_email": "alice@example.com",
                    "permission": "write",
                    "status": "pending",
                    "created_at": "2026-01-02T00:00:00Z",
                    "accepted_at": null
                }
            ]
        })))
        .mount(&server)
        .await;

    let shares = mgr.get_entity_shares("ent-1").await.unwrap();
    assert_eq!(shares.len(), 2);
    assert_eq!(shares[0].recipient_email, "bob@example.com");
    assert_eq!(shares[1].permission, SharePermission::Write);
}

#[tokio::test]
async fn get_entity_shares_empty() {
    let (server, _api, mgr) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/share/entity/ent-empty"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "shares": []
        })))
        .mount(&server)
        .await;

    let shares = mgr.get_entity_shares("ent-empty").await.unwrap();
    assert!(shares.is_empty());
}

// ── get_shared_with_me ─────────────────────────────────────────────────

#[tokio::test]
async fn get_shared_with_me_returns_list() {
    let (server, _api, mgr) = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/share/received"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "shares": [
                {
                    "entity_id": "ent-shared-1",
                    "entity_type": "note",
                    "entity_name": "Shared Note",
                    "owner_user_id": 42,
                    "workspace_id": "ws-42",
                    "permission": "read"
                }
            ]
        })))
        .mount(&server)
        .await;

    let shared = mgr.get_shared_with_me().await.unwrap();
    assert_eq!(shared.len(), 1);
    assert_eq!(shared[0].entity_id, "ent-shared-1");
}

// ── error cases ────────────────────────────────────────────────────────

#[tokio::test]
async fn create_share_server_error() {
    let (server, _api, mgr) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/share/create"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let req = CreateShareRequest {
        entity_id: "ent-1".into(),
        entity_type: "note".into(),
        entity_name: None,
        recipient_email: "bob@example.com".into(),
        permission: SharePermission::Read,
        workspace_id: "ws-1".into(),
    };

    assert!(mgr.create_share(&req).await.is_err());
}

#[tokio::test]
async fn accept_share_not_found() {
    let (server, _api, mgr) = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/share/accept"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    assert!(mgr.accept_share("bad-token").await.is_err());
}
