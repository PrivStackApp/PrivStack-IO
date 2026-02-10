use privstack_crypto::{
    decrypt, decrypt_string, encrypt, encrypt_string, generate_random_key, EncryptedData,
};

#[test]
fn encrypt_decrypt_roundtrip() {
    let key = generate_random_key();
    let plaintext = b"Hello, World!";
    let encrypted = encrypt(&key, plaintext).unwrap();
    let decrypted = decrypt(&key, &encrypted).unwrap();
    assert_eq!(decrypted, plaintext);
}

#[test]
fn encrypt_decrypt_empty() {
    let key = generate_random_key();
    let encrypted = encrypt(&key, b"").unwrap();
    let decrypted = decrypt(&key, &encrypted).unwrap();
    assert_eq!(decrypted, b"");
}

#[test]
fn encrypt_decrypt_large_data() {
    let key = generate_random_key();
    let plaintext: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();
    let encrypted = encrypt(&key, &plaintext).unwrap();
    let decrypted = decrypt(&key, &encrypted).unwrap();
    assert_eq!(decrypted, plaintext);
}

#[test]
fn wrong_key_fails_decryption() {
    let key1 = generate_random_key();
    let key2 = generate_random_key();
    let encrypted = encrypt(&key1, b"Secret").unwrap();
    assert!(decrypt(&key2, &encrypted).is_err());
}

#[test]
fn tampered_data_fails_decryption() {
    let key = generate_random_key();
    let mut encrypted = encrypt(&key, b"Secret").unwrap();
    if !encrypted.ciphertext.is_empty() {
        encrypted.ciphertext[0] ^= 0xFF;
    }
    assert!(decrypt(&key, &encrypted).is_err());
}

#[test]
fn same_plaintext_produces_different_ciphertext() {
    let key = generate_random_key();
    let e1 = encrypt(&key, b"Same").unwrap();
    let e2 = encrypt(&key, b"Same").unwrap();
    assert_ne!(e1.nonce, e2.nonce);
    assert_ne!(e1.ciphertext, e2.ciphertext);
}

// ── EncryptedData ────────────────────────────────────────────────

#[test]
fn encrypted_data_len() {
    let key = generate_random_key();
    let encrypted = encrypt(&key, b"test").unwrap();
    assert_eq!(encrypted.len(), 12 + encrypted.ciphertext.len());
}

#[test]
fn encrypted_data_is_empty() {
    let ed = EncryptedData {
        nonce: [0u8; 12],
        ciphertext: vec![],
    };
    assert!(ed.is_empty());

    let key = generate_random_key();
    let encrypted = encrypt(&key, b"data").unwrap();
    assert!(!encrypted.is_empty());
}

#[test]
fn base64_roundtrip() {
    let key = generate_random_key();
    let encrypted = encrypt(&key, b"Data").unwrap();
    let encoded = encrypted.to_base64();
    let decoded = EncryptedData::from_base64(&encoded).unwrap();
    assert_eq!(encrypted.nonce, decoded.nonce);
    assert_eq!(encrypted.ciphertext, decoded.ciphertext);
}

#[test]
fn base64_too_short_fails() {
    // Less than NONCE_SIZE + TAG_SIZE = 28 bytes
    use base64::{engine::general_purpose::STANDARD, Engine};
    let short = STANDARD.encode([0u8; 10]);
    assert!(EncryptedData::from_base64(&short).is_err());
}

#[test]
fn base64_invalid_fails() {
    assert!(EncryptedData::from_base64("!!!not-base64!!!").is_err());
}

#[test]
fn encrypted_data_serde_roundtrip() {
    let key = generate_random_key();
    let encrypted = encrypt(&key, b"test").unwrap();
    let json = serde_json::to_string(&encrypted).unwrap();
    let parsed: EncryptedData = serde_json::from_str(&json).unwrap();
    assert_eq!(encrypted.nonce, parsed.nonce);
    assert_eq!(encrypted.ciphertext, parsed.ciphertext);
}

// ── String ───────────────────────────────────────────────────────

#[test]
fn string_encrypt_decrypt() {
    let key = generate_random_key();
    let plaintext = "Hello, 世界! 🌍";
    let encrypted = encrypt_string(&key, plaintext).unwrap();
    let decrypted = decrypt_string(&key, &encrypted).unwrap();
    assert_eq!(decrypted, plaintext);
}

#[test]
fn decrypt_string_wrong_key_fails() {
    let k1 = generate_random_key();
    let k2 = generate_random_key();
    let encrypted = encrypt_string(&k1, "secret").unwrap();
    assert!(decrypt_string(&k2, &encrypted).is_err());
}

#[test]
fn decrypt_string_invalid_base64_fails() {
    let key = generate_random_key();
    assert!(decrypt_string(&key, "not-valid-base64!!!").is_err());
}
