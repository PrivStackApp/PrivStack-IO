//! Shared test helpers for license tests.

#![allow(dead_code)]

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signer, SigningKey};

/// Returns a deterministic Ed25519 key pair from a fixed seed.
pub fn test_keypair() -> (SigningKey, [u8; 32]) {
    let seed: [u8; 32] = [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31, 32,
    ];
    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();
    (signing_key, verifying_key.to_bytes())
}

/// Creates a signed license key string: `base64url(payload_json).base64url(signature)`.
/// Signs over the base64url-encoded payload bytes (matching server behavior).
pub fn sign_key(signing_key: &SigningKey, payload_json: &str) -> String {
    let payload_b64 = URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
    let signature = signing_key.sign(payload_b64.as_bytes());
    let sig_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());
    format!("{payload_b64}.{sig_b64}")
}

/// Creates a signed key with a standard perpetual payload.
pub fn make_perpetual_key(signing_key: &SigningKey) -> String {
    let now = chrono::Utc::now().timestamp();
    let payload = format!(
        r#"{{"sub":1,"email":"test@example.com","plan":"perpetual","iat":{now}}}"#
    );
    sign_key(signing_key, &payload)
}

/// Creates a signed key with a monthly payload issued at a given timestamp.
pub fn make_monthly_key_at(signing_key: &SigningKey, iat: i64) -> String {
    let payload = format!(
        r#"{{"sub":1,"email":"test@example.com","plan":"monthly","iat":{iat}}}"#
    );
    sign_key(signing_key, &payload)
}

/// Creates a signed key with an annual payload issued at a given timestamp.
pub fn make_annual_key_at(signing_key: &SigningKey, iat: i64) -> String {
    let payload = format!(
        r#"{{"sub":1,"email":"test@example.com","plan":"annual","iat":{iat}}}"#
    );
    sign_key(signing_key, &payload)
}
