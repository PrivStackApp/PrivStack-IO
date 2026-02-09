use privstack_crypto::{
    decrypt_document, derive_key, encrypt_document, reencrypt_document_key, KdfParams, Salt,
};

fn test_master_key() -> privstack_crypto::DerivedKey {
    let salt = Salt::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
    let params = KdfParams {
        memory_cost: 1024,
        time_cost: 1,
        parallelism: 1,
    };
    derive_key("test_password", &salt, &params).unwrap()
}

#[test]
fn encrypt_decrypt_roundtrip() {
    let master_key = test_master_key();
    let data = b"Hello, encrypted world!";

    let encrypted = encrypt_document("test-id-1", data, &master_key).unwrap();
    let decrypted = decrypt_document(&encrypted, &master_key).unwrap();

    assert_eq!(decrypted, data);
}

#[test]
fn encrypted_has_correct_id() {
    let master_key = test_master_key();
    let encrypted = encrypt_document("my-entity", b"test", &master_key).unwrap();
    assert_eq!(encrypted.id, "my-entity");
}

#[test]
fn wrong_key_fails_decryption() {
    let key1 = test_master_key();
    let key2 = {
        let salt = Salt::from_bytes([16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);
        let params = KdfParams {
            memory_cost: 1024,
            time_cost: 1,
            parallelism: 1,
        };
        derive_key("different_password", &salt, &params).unwrap()
    };

    let encrypted = encrypt_document("secret", b"secret data", &key1).unwrap();
    let result = decrypt_document(&encrypted, &key2);
    assert!(result.is_err());
}

#[test]
fn reencrypt_with_new_key() {
    let old_key = test_master_key();
    let new_key = {
        let salt = Salt::from_bytes([16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);
        let params = KdfParams {
            memory_cost: 1024,
            time_cost: 1,
            parallelism: 1,
        };
        derive_key("new_password", &salt, &params).unwrap()
    };

    let data = b"Content to protect";
    let encrypted = encrypt_document("test", data, &old_key).unwrap();
    let reencrypted = reencrypt_document_key(&encrypted, &old_key, &new_key).unwrap();

    assert!(decrypt_document(&reencrypted, &old_key).is_err());

    let decrypted = decrypt_document(&reencrypted, &new_key).unwrap();
    assert_eq!(decrypted, data);
}

#[test]
fn same_data_produces_different_ciphertext() {
    let master_key = test_master_key();
    let data = b"test data";

    let encrypted1 = encrypt_document("id1", data, &master_key).unwrap();
    let encrypted2 = encrypt_document("id2", data, &master_key).unwrap();

    assert_ne!(
        encrypted1.encrypted_content.ciphertext,
        encrypted2.encrypted_content.ciphertext
    );
}

#[test]
fn metadata_extraction() {
    use privstack_crypto::EncryptedDocumentMetadata;

    let master_key = test_master_key();
    let encrypted = encrypt_document("test-id", b"data", &master_key).unwrap();
    let metadata = EncryptedDocumentMetadata::from(&encrypted);

    assert_eq!(metadata.id, "test-id");
    assert!(metadata.encrypted_size > 0);
}

#[test]
fn full_encryption_workflow() {
    let password = "my_secure_password_123!";
    let salt = Salt::random();
    let params = KdfParams::default();

    let master_key = derive_key(password, &salt, &params).unwrap();

    let data = br#"{"title":"Meeting Notes","items":["Review PR","Update docs"]}"#;
    let encrypted = encrypt_document("entity-123", data, &master_key).unwrap();

    assert_eq!(encrypted.id, "entity-123");

    let decrypted = decrypt_document(&encrypted, &master_key).unwrap();
    assert_eq!(decrypted, data);
}

#[test]
fn encrypted_document_version() {
    let key = test_master_key();
    let encrypted = encrypt_document("v", b"data", &key).unwrap();
    assert_eq!(encrypted.version, 1);
}

#[test]
fn encrypted_document_serde_roundtrip() {
    let key = test_master_key();
    let encrypted = encrypt_document("serde", b"test", &key).unwrap();
    let json = serde_json::to_string(&encrypted).unwrap();
    let parsed: privstack_crypto::EncryptedDocument = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.id, "serde");
    let decrypted = decrypt_document(&parsed, &key).unwrap();
    assert_eq!(decrypted, b"test");
}

#[test]
fn metadata_version_and_size() {
    use privstack_crypto::EncryptedDocumentMetadata;
    let key = test_master_key();
    let encrypted = encrypt_document("meta", b"hello world", &key).unwrap();
    let metadata = EncryptedDocumentMetadata::from(&encrypted);
    assert_eq!(metadata.id, "meta");
    assert_eq!(metadata.version, 1);
    assert!(metadata.encrypted_size > 0);
}

#[test]
fn password_change_workflow() {
    let old_password = "old_password";
    let old_salt = Salt::random();
    let old_key = derive_key(old_password, &old_salt, &KdfParams::default()).unwrap();

    let data = b"Important Data";
    let encrypted = encrypt_document("entity-456", data, &old_key).unwrap();

    let new_password = "new_secure_password";
    let new_salt = Salt::random();
    let new_key = derive_key(new_password, &new_salt, &KdfParams::default()).unwrap();

    let reencrypted = reencrypt_document_key(&encrypted, &old_key, &new_key).unwrap();

    assert!(decrypt_document(&reencrypted, &old_key).is_err());

    let decrypted = decrypt_document(&reencrypted, &new_key).unwrap();
    assert_eq!(decrypted, data);
}
