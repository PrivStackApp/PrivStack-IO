use privstack_crypto::{EncryptorError, PassthroughEncryptor, DataEncryptor};

// ── EncryptorError Display ──────────────────────────────────────

#[test]
fn encryptor_error_display_unavailable() {
    let err = EncryptorError::Unavailable;
    let msg = format!("{err}");
    assert!(msg.contains("unavailable"));
    assert!(msg.contains("vault locked"));
}

#[test]
fn encryptor_error_display_crypto() {
    let err = EncryptorError::Crypto("bad cipher".into());
    let msg = format!("{err}");
    assert!(msg.contains("crypto error"));
    assert!(msg.contains("bad cipher"));
}

#[test]
fn encryptor_error_display_serialization() {
    let err = EncryptorError::Serialization("corrupt".into());
    let msg = format!("{err}");
    assert!(msg.contains("serialization error"));
    assert!(msg.contains("corrupt"));
}

#[test]
fn encryptor_error_debug() {
    let err = EncryptorError::Unavailable;
    let dbg = format!("{err:?}");
    assert!(dbg.contains("Unavailable"));
}

// ── PassthroughEncryptor ────────────────────────────────────────

#[test]
fn passthrough_is_available() {
    let enc = PassthroughEncryptor;
    assert!(enc.is_available());
}

#[test]
fn passthrough_encrypt_returns_same_data() {
    let enc = PassthroughEncryptor;
    let data = b"hello world";
    let result = enc.encrypt_bytes("entity-1", data).unwrap();
    assert_eq!(result, data);
}

#[test]
fn passthrough_decrypt_returns_same_data() {
    let enc = PassthroughEncryptor;
    let data = b"hello world";
    let result = enc.decrypt_bytes(data).unwrap();
    assert_eq!(result, data);
}

#[test]
fn passthrough_reencrypt_returns_same_data() {
    let enc = PassthroughEncryptor;
    let data = b"some payload";
    let result = enc.reencrypt_bytes(data, b"old_key", b"new_key").unwrap();
    assert_eq!(result, data);
}

#[test]
fn passthrough_encrypt_empty_data() {
    let enc = PassthroughEncryptor;
    let result = enc.encrypt_bytes("e", b"").unwrap();
    assert!(result.is_empty());
}

#[test]
fn passthrough_decrypt_empty_data() {
    let enc = PassthroughEncryptor;
    let result = enc.decrypt_bytes(b"").unwrap();
    assert!(result.is_empty());
}

#[test]
fn passthrough_reencrypt_empty_data() {
    let enc = PassthroughEncryptor;
    let result = enc.reencrypt_bytes(b"", b"", b"").unwrap();
    assert!(result.is_empty());
}

#[test]
fn passthrough_large_data() {
    let enc = PassthroughEncryptor;
    let data: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
    let encrypted = enc.encrypt_bytes("big", &data).unwrap();
    assert_eq!(encrypted, data);
    let decrypted = enc.decrypt_bytes(&encrypted).unwrap();
    assert_eq!(decrypted, data);
}

#[test]
fn passthrough_roundtrip() {
    let enc = PassthroughEncryptor;
    let original = b"roundtrip test data";
    let encrypted = enc.encrypt_bytes("rt", original).unwrap();
    let decrypted = enc.decrypt_bytes(&encrypted).unwrap();
    assert_eq!(decrypted, original);
}

// ── DataEncryptor as trait object ───────────────────────────────

#[test]
fn passthrough_as_dyn_trait() {
    let enc: Box<dyn DataEncryptor> = Box::new(PassthroughEncryptor);
    assert!(enc.is_available());
    let data = b"trait object test";
    let result = enc.encrypt_bytes("dyn", data).unwrap();
    assert_eq!(result, data);
}
