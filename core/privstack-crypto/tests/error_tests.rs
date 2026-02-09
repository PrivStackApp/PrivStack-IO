use privstack_crypto::CryptoError;

#[test]
fn error_display_key_derivation() {
    let err = CryptoError::KeyDerivation("bad params".into());
    assert!(format!("{err}").contains("key derivation failed"));
    assert!(format!("{err}").contains("bad params"));
}

#[test]
fn error_display_encryption() {
    let err = CryptoError::Encryption("oops".into());
    assert!(format!("{err}").contains("encryption failed"));
}

#[test]
fn error_display_decryption() {
    let err = CryptoError::Decryption("tampered".into());
    assert!(format!("{err}").contains("decryption failed"));
}

#[test]
fn error_display_invalid_key_length() {
    let err = CryptoError::InvalidKeyLength {
        expected: 32,
        actual: 16,
    };
    let msg = format!("{err}");
    assert!(msg.contains("32"));
    assert!(msg.contains("16"));
}

#[test]
fn error_display_invalid_nonce_length() {
    let err = CryptoError::InvalidNonceLength {
        expected: 12,
        actual: 8,
    };
    let msg = format!("{err}");
    assert!(msg.contains("12"));
    assert!(msg.contains("8"));
}

#[test]
fn error_from_serde_json() {
    let serde_err: Result<serde_json::Value, _> = serde_json::from_str("not json");
    let crypto_err: CryptoError = serde_err.unwrap_err().into();
    assert!(format!("{crypto_err}").contains("serialization"));
}

#[test]
fn error_is_debug() {
    let err = CryptoError::Encryption("test".into());
    let _ = format!("{err:?}");
}
