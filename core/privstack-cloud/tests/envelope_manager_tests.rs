//! Tests for EnvelopeManager: keypair state, open_dek, seal/open roundtrip.

use privstack_cloud::api_client::CloudApiClient;
use privstack_cloud::config::CloudConfig;
use privstack_cloud::envelope::EnvelopeManager;
use privstack_crypto::envelope::{generate_cloud_keypair, seal_dek};
use std::sync::Arc;
use wiremock::MockServer;

fn make_manager(api: Arc<CloudApiClient>) -> EnvelopeManager {
    EnvelopeManager::new(api)
}

async fn setup() -> (MockServer, Arc<CloudApiClient>) {
    let server = MockServer::start().await;
    let config = CloudConfig {
        api_base_url: server.uri(),
        s3_bucket: "test".into(),
        s3_region: "us-east-2".into(),
        s3_endpoint_override: None,
        credential_refresh_margin_secs: 60,
        poll_interval_secs: 5,
    };
    let client = Arc::new(CloudApiClient::new(config));
    (server, client)
}

// ── has_keypair ────────────────────────────────────────────────────────

#[tokio::test]
async fn no_keypair_initially() {
    let (_server, api) = setup().await;
    let mgr = make_manager(api);
    assert!(!mgr.has_keypair());
}

#[tokio::test]
async fn has_keypair_after_set() {
    let (_server, api) = setup().await;
    let mut mgr = make_manager(api);
    let kp = generate_cloud_keypair();
    mgr.set_keypair(kp);
    assert!(mgr.has_keypair());
}

// ── public_key_bytes ───────────────────────────────────────────────────

#[tokio::test]
async fn public_key_bytes_none_without_keypair() {
    let (_server, api) = setup().await;
    let mgr = make_manager(api);
    assert!(mgr.public_key_bytes().is_none());
}

#[tokio::test]
async fn public_key_bytes_matches_keypair() {
    let (_server, api) = setup().await;
    let mut mgr = make_manager(api);
    let kp = generate_cloud_keypair();
    let expected = kp.public_bytes();
    mgr.set_keypair(kp);
    assert_eq!(mgr.public_key_bytes(), Some(expected));
}

// ── open_dek ───────────────────────────────────────────────────────────

#[tokio::test]
async fn open_dek_without_keypair_returns_error() {
    let (_server, api) = setup().await;
    let mgr = make_manager(api);
    let kp = generate_cloud_keypair();
    let dek = b"0123456789abcdef0123456789abcdef";
    let envelope = seal_dek(dek, &kp.public).unwrap();

    let err = mgr.open_dek(&envelope).unwrap_err();
    assert!(err.to_string().contains("no keypair loaded"));
}

#[tokio::test]
async fn open_dek_roundtrip_with_correct_keypair() {
    let (_server, api) = setup().await;
    let mut mgr = make_manager(api);
    let kp = generate_cloud_keypair();
    let dek = b"0123456789abcdef0123456789abcdef";
    let envelope = seal_dek(dek, &kp.public).unwrap();

    mgr.set_keypair(kp);
    let opened = mgr.open_dek(&envelope).unwrap();
    assert_eq!(opened, dek);
}

#[tokio::test]
async fn open_dek_with_wrong_keypair_fails() {
    let (_server, api) = setup().await;
    let mut mgr = make_manager(api);
    let intended = generate_cloud_keypair();
    let wrong = generate_cloud_keypair();
    let dek = b"0123456789abcdef0123456789abcdef";
    let envelope = seal_dek(dek, &intended.public).unwrap();

    mgr.set_keypair(wrong);
    let err = mgr.open_dek(&envelope);
    assert!(err.is_err());
}

// ── seal_dek_for_user (requires wiremock) ──────────────────────────────

#[tokio::test]
async fn seal_dek_for_user_roundtrip() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    let (server, api) = setup().await;
    let mgr = make_manager(api.clone());

    // Generate recipient keypair
    let recipient = generate_cloud_keypair();
    let recipient_pk_b64 = STANDARD.encode(recipient.public_bytes());

    // Mock the public key endpoint
    Mock::given(method("GET"))
        .and(path("/api/cloud/keys/public/99"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "public_key": recipient_pk_b64 })),
        )
        .mount(&server)
        .await;

    // Need auth tokens for the API call
    api.set_tokens("at".into(), "rt".into(), 1).await;

    let dek = b"0123456789abcdef0123456789abcdef";
    let envelope = mgr.seal_dek_for_user(dek, 99).await.unwrap();

    // Recipient should be able to open it
    let mut recipient_mgr = make_manager(Arc::new(CloudApiClient::new(CloudConfig {
        api_base_url: "http://unused".into(),
        s3_bucket: "test".into(),
        s3_region: "us-east-2".into(),
        s3_endpoint_override: None,
        credential_refresh_margin_secs: 60,
        poll_interval_secs: 5,
    })));
    recipient_mgr.set_keypair(recipient);
    let opened = recipient_mgr.open_dek(&envelope).unwrap();
    assert_eq!(opened, dek);
}

// ── seal_dek_for_user error cases ──────────────────────────────────────

#[tokio::test]
async fn seal_dek_for_user_fails_without_auth() {
    let (_server, api) = setup().await;
    let mgr = make_manager(api);

    let dek = b"0123456789abcdef0123456789abcdef";
    let err = mgr.seal_dek_for_user(dek, 99).await;
    assert!(err.is_err());
}

// ── create_and_store_envelope ──────────────────────────────────────────

#[tokio::test]
async fn create_and_store_envelope_roundtrip() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    let (server, api) = setup().await;
    let mgr = make_manager(api.clone());
    api.set_tokens("at".into(), "rt".into(), 1).await;

    let recipient = generate_cloud_keypair();
    let recipient_pk_b64 = STANDARD.encode(recipient.public_bytes());

    // Mock: get public key
    Mock::given(method("GET"))
        .and(path("/api/cloud/keys/public/42"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "public_key": recipient_pk_b64 })),
        )
        .mount(&server)
        .await;

    // Mock: store share key
    Mock::given(method("POST"))
        .and(path("/api/share/keys/store"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let dek = b"0123456789abcdef0123456789abcdef";
    mgr.create_and_store_envelope("ent-1", dek, 42)
        .await
        .unwrap();
}

// ── retrieve_and_open_dek ──────────────────────────────────────────────

#[tokio::test]
async fn retrieve_and_open_dek_roundtrip() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    let (server, api) = setup().await;
    let mut mgr = make_manager(api.clone());
    api.set_tokens("at".into(), "rt".into(), 1).await;

    let recipient = generate_cloud_keypair();
    let dek = b"0123456789abcdef0123456789abcdef";
    let envelope = seal_dek(dek, &recipient.public).unwrap();

    // Mock: get share key
    Mock::given(method("GET"))
        .and(path("/api/share/keys/ent-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(&envelope),
        )
        .mount(&server)
        .await;

    mgr.set_keypair(recipient);
    let opened = mgr.retrieve_and_open_dek("ent-1").await.unwrap();
    assert_eq!(opened, dek);
}

#[tokio::test]
async fn retrieve_and_open_dek_fails_without_keypair() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    let (server, api) = setup().await;
    let mgr = make_manager(api.clone());
    api.set_tokens("at".into(), "rt".into(), 1).await;

    let kp = generate_cloud_keypair();
    let dek = b"0123456789abcdef0123456789abcdef";
    let envelope = seal_dek(dek, &kp.public).unwrap();

    Mock::given(method("GET"))
        .and(path("/api/share/keys/ent-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(&envelope),
        )
        .mount(&server)
        .await;

    let err = mgr.retrieve_and_open_dek("ent-1").await;
    assert!(err.is_err());
}
