//! Tests for CloudError helper methods: is_rate_limited, is_transient, retry_after.

use privstack_cloud::CloudError;
use std::time::Duration;

// ── is_rate_limited ─────────────────────────────────────────────────────

#[test]
fn rate_limited_variant_is_rate_limited() {
    let err = CloudError::RateLimited { retry_after_secs: 60 };
    assert!(err.is_rate_limited());
}

#[test]
fn api_429_message_is_rate_limited() {
    let err = CloudError::Api("429 Too Many Requests".into());
    assert!(err.is_rate_limited());
}

#[test]
fn api_non_429_is_not_rate_limited() {
    let err = CloudError::Api("500 Internal Server Error".into());
    assert!(!err.is_rate_limited());
}

#[test]
fn s3_error_is_not_rate_limited() {
    let err = CloudError::S3("bucket not found".into());
    assert!(!err.is_rate_limited());
}

#[test]
fn auth_required_is_not_rate_limited() {
    let err = CloudError::AuthRequired;
    assert!(!err.is_rate_limited());
}

#[test]
fn credential_expired_is_not_rate_limited() {
    let err = CloudError::CredentialExpired;
    assert!(!err.is_rate_limited());
}

// ── is_transient ────────────────────────────────────────────────────────

#[test]
fn rate_limited_is_transient() {
    let err = CloudError::RateLimited { retry_after_secs: 30 };
    assert!(err.is_transient());
}

#[test]
fn s3_503_is_transient() {
    let err = CloudError::S3("503 Service Unavailable".into());
    assert!(err.is_transient());
}

#[test]
fn s3_dispatch_failure_is_transient() {
    let err = CloudError::S3("dispatch failure: connection pool exhausted".into());
    assert!(err.is_transient());
}

#[test]
fn s3_service_error_is_transient() {
    let err = CloudError::S3("upload failed: service error".into());
    assert!(err.is_transient());
}

#[test]
fn s3_connection_reset_is_transient() {
    let err = CloudError::S3("ConnectionReset: peer closed connection".into());
    assert!(err.is_transient());

    let err2 = CloudError::S3("Connection reset by peer".into());
    assert!(err2.is_transient());
}

#[test]
fn s3_timeout_is_transient() {
    let err = CloudError::S3("request timed out after 30s".into());
    assert!(err.is_transient());

    let err2 = CloudError::S3("connection timeout".into());
    assert!(err2.is_transient());
}

#[test]
fn s3_permanent_error_is_not_transient() {
    let err = CloudError::S3("AccessDenied: check IAM policy".into());
    assert!(!err.is_transient());
}

#[test]
fn api_error_is_not_transient() {
    let err = CloudError::Api("400 Bad Request".into());
    assert!(!err.is_transient());
}

#[test]
fn auth_required_is_not_transient() {
    let err = CloudError::AuthRequired;
    assert!(!err.is_transient());
}

#[test]
fn envelope_error_is_not_transient() {
    let err = CloudError::Envelope("decryption failed".into());
    assert!(!err.is_transient());
}

#[test]
fn quota_exceeded_is_not_transient() {
    let err = CloudError::QuotaExceeded { used: 100, quota: 50 };
    assert!(!err.is_transient());
}

// ── retry_after ─────────────────────────────────────────────────────────

#[test]
fn retry_after_on_rate_limited() {
    let err = CloudError::RateLimited { retry_after_secs: 120 };
    assert_eq!(err.retry_after(), Some(Duration::from_secs(120)));
}

#[test]
fn retry_after_on_rate_limited_zero() {
    let err = CloudError::RateLimited { retry_after_secs: 0 };
    assert_eq!(err.retry_after(), Some(Duration::from_secs(0)));
}

#[test]
fn retry_after_on_non_rate_limited_is_none() {
    let err = CloudError::S3("timeout".into());
    assert!(err.retry_after().is_none());

    let err = CloudError::AuthRequired;
    assert!(err.retry_after().is_none());

    let err = CloudError::Api("500".into());
    assert!(err.retry_after().is_none());
}

// ── Display strings for uncovered variants ──────────────────────────────

#[test]
fn rate_limited_display() {
    let err = CloudError::RateLimited { retry_after_secs: 45 };
    assert_eq!(err.to_string(), "rate limited: retry after 45s");
}

#[test]
fn crypto_error_display() {
    let err = CloudError::Crypto(privstack_crypto::CryptoError::Decryption("wrong key".into()));
    assert!(err.to_string().contains("crypto error"));
}

// ── Display for all other variants ─────────────────────────────────────

#[test]
fn s3_error_display() {
    let err = CloudError::S3("bucket not found".into());
    assert_eq!(err.to_string(), "S3 operation failed: bucket not found");
}

#[test]
fn api_error_display() {
    let err = CloudError::Api("400 Bad Request".into());
    assert_eq!(err.to_string(), "API request failed: 400 Bad Request");
}

#[test]
fn quota_exceeded_display() {
    let err = CloudError::QuotaExceeded { used: 100, quota: 50 };
    assert!(err.to_string().contains("100"));
    assert!(err.to_string().contains("50"));
}

#[test]
fn credential_expired_display() {
    let err = CloudError::CredentialExpired;
    assert!(err.to_string().contains("expired"));
}

#[test]
fn lock_contention_display() {
    let err = CloudError::LockContention("entity locked by device-2".into());
    assert!(err.to_string().contains("entity locked by device-2"));
}

#[test]
fn share_denied_display() {
    let err = CloudError::ShareDenied("insufficient permission".into());
    assert!(err.to_string().contains("insufficient permission"));
}

#[test]
fn envelope_error_display() {
    let err = CloudError::Envelope("decryption failed".into());
    assert!(err.to_string().contains("decryption failed"));
}

#[test]
fn auth_required_display() {
    let err = CloudError::AuthRequired;
    assert_eq!(err.to_string(), "authentication required");
}

#[test]
fn auth_failed_display() {
    let err = CloudError::AuthFailed("bad token".into());
    assert!(err.to_string().contains("bad token"));
}

#[test]
fn not_found_display() {
    let err = CloudError::NotFound("entity abc".into());
    assert!(err.to_string().contains("entity abc"));
}

#[test]
fn config_error_display() {
    let err = CloudError::Config("missing bucket".into());
    assert!(err.to_string().contains("missing bucket"));
}

// ── Http variant via real request failure ───────────────────────────────

#[tokio::test]
async fn http_error_is_not_rate_limited() {
    // Create a real reqwest::Error by making a request to an invalid URL
    let client = reqwest::Client::new();
    let err = client.get("http://[::1]:1").send().await.unwrap_err();
    let cloud_err = CloudError::Http(err);
    assert!(!cloud_err.is_rate_limited());
}

#[tokio::test]
async fn http_connect_error_is_transient() {
    let client = reqwest::Client::new();
    let err = client.get("http://[::1]:1").send().await.unwrap_err();
    let cloud_err = CloudError::Http(err);
    // Connection errors are transient (is_connect())
    assert!(cloud_err.is_transient());
}

#[tokio::test]
async fn http_timeout_error_is_transient() {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(1))
        .build()
        .unwrap();
    // Use wiremock to create a delayed response that triggers timeout
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::any())
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string("ok")
                .set_delay(std::time::Duration::from_secs(5)),
        )
        .mount(&server)
        .await;

    let err = client.get(server.uri()).send().await.unwrap_err();
    let cloud_err = CloudError::Http(err);
    assert!(cloud_err.is_transient());
}

#[tokio::test]
async fn http_error_display() {
    let client = reqwest::Client::new();
    let err = client.get("http://[::1]:1").send().await.unwrap_err();
    let cloud_err = CloudError::Http(err);
    assert!(cloud_err.to_string().contains("HTTP error"));
}
