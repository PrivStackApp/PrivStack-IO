use privstack_blobstore::{BlobStore, BlobStoreError};
use std::sync::Arc;

// ── Error type coverage ─────────────────────────────────────────
#[test]
fn error_display() {
    let err = BlobStoreError::NotFound("ns".to_string(), "id".to_string());
    assert!(format!("{err}").contains("ns"));
    assert!(format!("{err}").contains("id"));

    let err = BlobStoreError::Storage("disk full".to_string());
    assert!(format!("{err}").contains("disk full"));
}

#[test]
fn error_debug() {
    let err = BlobStoreError::NotFound("ns".to_string(), "id".to_string());
    assert!(format!("{err:?}").contains("NotFound"));

    let err = BlobStoreError::Storage("oops".to_string());
    assert!(format!("{err:?}").contains("Storage"));
}

// ── open_with_conn ──────────────────────────────────────────────
#[test]
fn open_with_conn() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = std::sync::Arc::new(std::sync::Mutex::new(conn));
    let store = BlobStore::open_with_conn(conn).unwrap();

    store.store("ns", "b1", b"data", None).unwrap();
    assert_eq!(store.read("ns", "b1").unwrap(), b"data");
}

// ── BlobMetadata Debug ──────────────────────────────────────────
#[test]
fn blob_metadata_debug() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "b1", b"data", Some(r#"{"k":"v"}"#)).unwrap();
    let items = store.list("ns").unwrap();
    let debug = format!("{:?}", items[0]);
    assert!(debug.contains("BlobMetadata"));
    assert!(debug.contains("b1"));
}

// ── BlobMetadata Serialize ──────────────────────────────────────
#[test]
fn blob_metadata_serialize() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "s1", b"ser test", Some(r#"{"x":1}"#)).unwrap();
    let items = store.list("ns").unwrap();
    let json = serde_json::to_string(&items[0]).unwrap();
    assert!(json.contains("s1"));
    assert!(json.contains("ns"));
}

// ── Multiple namespaces listing ─────────────────────────────────
#[test]
fn list_multiple_namespaces_independent() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("a", "1", b"x", None).unwrap();
    store.store("b", "2", b"y", None).unwrap();
    store.store("b", "3", b"z", None).unwrap();

    assert_eq!(store.list("a").unwrap().len(), 1);
    assert_eq!(store.list("b").unwrap().len(), 2);
    assert!(store.list("c").unwrap().is_empty());
}

// ── Update metadata preserves data ──────────────────────────────
#[test]
fn update_metadata_preserves_data() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "b1", b"original data", Some(r#"{"v":1}"#)).unwrap();
    store.update_metadata("ns", "b1", r#"{"v":2}"#).unwrap();

    let data = store.read("ns", "b1").unwrap();
    assert_eq!(data, b"original data");
    let items = store.list("ns").unwrap();
    assert_eq!(items[0].metadata_json.as_deref(), Some(r#"{"v":2}"#));
}

// ── Store no metadata ───────────────────────────────────────────
#[test]
fn store_no_metadata_returns_none() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "b1", b"data", None).unwrap();
    let items = store.list("ns").unwrap();
    assert!(items[0].metadata_json.is_none());
}

#[test]
fn store_and_read() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("media", "img1", b"png data", None).unwrap();
    let data = store.read("media", "img1").unwrap();
    assert_eq!(data, b"png data");
}

#[test]
fn store_with_metadata() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("docs", "f1", b"content", Some(r#"{"mime":"text/plain"}"#)).unwrap();
    let items = store.list("docs").unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].metadata_json.as_deref(), Some(r#"{"mime":"text/plain"}"#));
}

#[test]
fn read_nonexistent_fails() {
    let store = BlobStore::open_in_memory().unwrap();
    assert!(store.read("ns", "nope").is_err());
}

#[test]
fn delete_blob() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "b1", b"data", None).unwrap();
    store.delete("ns", "b1").unwrap();
    assert!(store.read("ns", "b1").is_err());
}

#[test]
fn delete_nonexistent_fails() {
    let store = BlobStore::open_in_memory().unwrap();
    assert!(store.delete("ns", "nope").is_err());
}

#[test]
fn list_empty_namespace() {
    let store = BlobStore::open_in_memory().unwrap();
    assert!(store.list("empty").unwrap().is_empty());
}

#[test]
fn list_multiple_blobs() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "a", b"aaa", None).unwrap();
    store.store("ns", "b", b"bbb", None).unwrap();
    store.store("other", "c", b"ccc", None).unwrap();

    assert_eq!(store.list("ns").unwrap().len(), 2);
    assert_eq!(store.list("other").unwrap().len(), 1);
}

#[test]
fn blob_metadata_fields() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "b1", b"hello", None).unwrap();
    let items = store.list("ns").unwrap();
    assert_eq!(items[0].namespace, "ns");
    assert_eq!(items[0].blob_id, "b1");
    assert_eq!(items[0].size, 5);
    assert!(items[0].content_hash.is_some());
    assert!(items[0].created_at > 0);
    assert!(items[0].modified_at > 0);
}

#[test]
fn content_hash_is_sha256() {
    use sha2::{Digest, Sha256};
    let store = BlobStore::open_in_memory().unwrap();
    let data = b"test data for hashing";
    store.store("ns", "h1", data, None).unwrap();

    let expected: String = Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect();
    let items = store.list("ns").unwrap();
    assert_eq!(items[0].content_hash.as_deref(), Some(expected.as_str()));
}

#[test]
fn overwrite_blob() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "b1", b"v1", None).unwrap();
    store.store("ns", "b1", b"v2 updated", None).unwrap();

    let data = store.read("ns", "b1").unwrap();
    assert_eq!(data, b"v2 updated");
    let items = store.list("ns").unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].size, 10);
}

#[test]
fn update_metadata() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "b1", b"data", Some(r#"{"mime":"image/png"}"#)).unwrap();
    store.update_metadata("ns", "b1", r#"{"tags":["new"]}"#).unwrap();

    let items = store.list("ns").unwrap();
    assert_eq!(items[0].metadata_json.as_deref(), Some(r#"{"tags":["new"]}"#));
}

#[test]
fn update_metadata_nonexistent_fails() {
    let store = BlobStore::open_in_memory().unwrap();
    assert!(store.update_metadata("ns", "nope", "{}").is_err());
}

#[test]
fn empty_blob() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "empty", b"", None).unwrap();
    let data = store.read("ns", "empty").unwrap();
    assert!(data.is_empty());
    let items = store.list("ns").unwrap();
    assert_eq!(items[0].size, 0);
}

#[test]
fn large_blob() {
    let store = BlobStore::open_in_memory().unwrap();
    let data: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
    store.store("ns", "large", &data, None).unwrap();
    let retrieved = store.read("ns", "large").unwrap();
    assert_eq!(retrieved, data);
}

#[test]
fn namespace_isolation() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("a", "key", b"from_a", None).unwrap();
    store.store("b", "key", b"from_b", None).unwrap();

    assert_eq!(store.read("a", "key").unwrap(), b"from_a");
    assert_eq!(store.read("b", "key").unwrap(), b"from_b");

    store.delete("a", "key").unwrap();
    assert!(store.read("a", "key").is_err());
    assert_eq!(store.read("b", "key").unwrap(), b"from_b");
}

#[test]
fn open_with_path() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("blobs.db");
    let store = BlobStore::open(&db_path).unwrap();
    store.store("ns", "b1", b"data", None).unwrap();
    drop(store);

    // Reopen and verify persistence
    let store2 = BlobStore::open(&db_path).unwrap();
    let data = store2.read("ns", "b1").unwrap();
    assert_eq!(data, b"data");
}

// ── Encryption failure during store ─────────────────────────────

/// A mock encryptor that fails on encrypt.
struct FailingEncryptor;

impl privstack_crypto::DataEncryptor for FailingEncryptor {
    fn encrypt_bytes(&self, _entity_id: &str, _data: &[u8]) -> privstack_crypto::EncryptorResult<Vec<u8>> {
        Err(privstack_crypto::EncryptorError::Crypto("simulated encrypt failure".into()))
    }
    fn decrypt_bytes(&self, _data: &[u8]) -> privstack_crypto::EncryptorResult<Vec<u8>> {
        Err(privstack_crypto::EncryptorError::Crypto("simulated decrypt failure".into()))
    }
    fn reencrypt_bytes(&self, _data: &[u8], _old: &[u8], _new: &[u8]) -> privstack_crypto::EncryptorResult<Vec<u8>> {
        Err(privstack_crypto::EncryptorError::Crypto("simulated reencrypt failure".into()))
    }
    fn is_available(&self) -> bool {
        true
    }
}

/// Encryptor that is unavailable (simulates locked vault).
struct UnavailableEncryptor;

impl privstack_crypto::DataEncryptor for UnavailableEncryptor {
    fn encrypt_bytes(&self, _entity_id: &str, _data: &[u8]) -> privstack_crypto::EncryptorResult<Vec<u8>> {
        Err(privstack_crypto::EncryptorError::Unavailable)
    }
    fn decrypt_bytes(&self, _data: &[u8]) -> privstack_crypto::EncryptorResult<Vec<u8>> {
        Err(privstack_crypto::EncryptorError::Unavailable)
    }
    fn reencrypt_bytes(&self, _data: &[u8], _old: &[u8], _new: &[u8]) -> privstack_crypto::EncryptorResult<Vec<u8>> {
        Err(privstack_crypto::EncryptorError::Unavailable)
    }
    fn is_available(&self) -> bool {
        false
    }
}

#[test]
fn store_with_encryption_failure() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("enc_fail.db");
    let store = BlobStore::open_with_encryptor(&db_path, Arc::new(FailingEncryptor)).unwrap();

    let result = store.store("ns", "b1", b"data", None);
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Encryption(msg) => assert!(msg.contains("simulated")),
        other => panic!("expected Encryption error, got: {other}"),
    }
}

// ── Decryption fallback (legacy unencrypted data) ────────────────

#[test]
fn read_falls_back_to_raw_when_decrypt_fails() {
    // Store data with passthrough (no encryption), then read with FailingEncryptor.
    // The read method should fall back to returning raw data.
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("fallback.db");

    // Store with passthrough
    {
        let store = BlobStore::open(&db_path).unwrap();
        store.store("ns", "b1", b"legacy data", None).unwrap();
    }

    // Read with failing encryptor — decrypt fails, should return raw
    {
        let store = BlobStore::open_with_encryptor(&db_path, Arc::new(FailingEncryptor)).unwrap();
        let data = store.read("ns", "b1").unwrap();
        assert_eq!(data, b"legacy data");
    }
}

#[test]
fn read_without_encryptor_available_returns_raw() {
    // Encryptor not available — read returns raw bytes
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("unavail.db");

    {
        let store = BlobStore::open(&db_path).unwrap();
        store.store("ns", "b1", b"raw bytes", None).unwrap();
    }

    {
        let store = BlobStore::open_with_encryptor(&db_path, Arc::new(UnavailableEncryptor)).unwrap();
        let data = store.read("ns", "b1").unwrap();
        assert_eq!(data, b"raw bytes");
    }
}

// ── re_encrypt_all error counting ────────────────────────────────

#[test]
fn re_encrypt_all_skips_unencrypted_blobs() {
    // With passthrough, reencrypt_bytes just returns data unchanged.
    // With FailingEncryptor, reencrypt fails → skipped (count=0).
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("reenc.db");

    {
        let store = BlobStore::open(&db_path).unwrap();
        store.store("ns", "b1", b"data1", None).unwrap();
        store.store("ns", "b2", b"data2", None).unwrap();
    }

    {
        let store = BlobStore::open_with_encryptor(&db_path, Arc::new(FailingEncryptor)).unwrap();
        let count = store.re_encrypt_all(b"old", b"new").unwrap();
        // FailingEncryptor.reencrypt_bytes returns Err → all skipped
        assert_eq!(count, 0);
    }
}

#[test]
fn re_encrypt_all_with_passthrough() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "b1", b"data1", None).unwrap();
    store.store("ns", "b2", b"data2", None).unwrap();

    // Passthrough reencrypt just returns same data — succeeds for all rows
    let count = store.re_encrypt_all(b"old", b"new").unwrap();
    assert_eq!(count, 2);

    // Data still readable
    assert_eq!(store.read("ns", "b1").unwrap(), b"data1");
    assert_eq!(store.read("ns", "b2").unwrap(), b"data2");
}

#[test]
fn re_encrypt_all_empty_store() {
    let store = BlobStore::open_in_memory().unwrap();
    let count = store.re_encrypt_all(b"old", b"new").unwrap();
    assert_eq!(count, 0);
}

// ── migrate_unencrypted without vault (unavailable) ──────────────

#[test]
fn migrate_unencrypted_without_vault_fails() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("migrate.db");
    let store = BlobStore::open_with_encryptor(&db_path, Arc::new(UnavailableEncryptor)).unwrap();

    let result = store.migrate_unencrypted();
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Encryption(msg) => assert!(msg.contains("unavailable")),
        other => panic!("expected Encryption error, got: {other}"),
    }
}

#[test]
fn migrate_unencrypted_empty_store() {
    let store = BlobStore::open_in_memory().unwrap();
    // passthrough is_available=true, but no rows
    let count = store.migrate_unencrypted().unwrap();
    assert_eq!(count, 0);
}

#[test]
fn migrate_unencrypted_with_passthrough_skips_all() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "b1", b"data1", None).unwrap();
    store.store("ns", "b2", b"data2", None).unwrap();

    // Passthrough decrypt_bytes always succeeds → blobs considered already encrypted → skip
    let count = store.migrate_unencrypted().unwrap();
    assert_eq!(count, 0);
}

// ── open with :memory: path ──────────────────────────────────────

#[test]
fn open_with_memory_path_string() {
    let store = BlobStore::open(std::path::Path::new(":memory:")).unwrap();
    store.store("ns", "b1", b"mem data", None).unwrap();
    let data = store.read("ns", "b1").unwrap();
    assert_eq!(data, b"mem data");
}

#[test]
fn open_with_encryptor_memory_path() {
    let store = BlobStore::open_with_encryptor(
        std::path::Path::new(":memory:"),
        Arc::new(privstack_crypto::PassthroughEncryptor),
    ).unwrap();
    store.store("ns", "b1", b"enc mem", None).unwrap();
    assert_eq!(store.read("ns", "b1").unwrap(), b"enc mem");
}

// ── Encryption error display ─────────────────────────────────────

#[test]
fn encryption_error_display() {
    let err = BlobStoreError::Encryption("key error".to_string());
    assert!(format!("{err}").contains("key error"));
    assert!(format!("{err:?}").contains("Encryption"));
}

// ── Store without encryptor available stores plaintext ────────────

#[test]
fn store_without_encryptor_available_stores_plaintext() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("noenc.db");

    let store = BlobStore::open_with_encryptor(&db_path, Arc::new(UnavailableEncryptor)).unwrap();
    // Encryptor not available → store plaintext
    store.store("ns", "b1", b"plain data", None).unwrap();

    // Read back — encryptor not available → raw data returned
    let data = store.read("ns", "b1").unwrap();
    assert_eq!(data, b"plain data");
}

// ── migrate_unencrypted with actual encryption ───────────────────

/// An encryptor that succeeds for encrypt but fails for decrypt (simulates unencrypted data detection).
struct EncryptOnlyEncryptor;

impl privstack_crypto::DataEncryptor for EncryptOnlyEncryptor {
    fn encrypt_bytes(&self, _entity_id: &str, data: &[u8]) -> privstack_crypto::EncryptorResult<Vec<u8>> {
        // Prefix with a marker so we can detect "encrypted" data
        let mut out = b"ENC:".to_vec();
        out.extend_from_slice(data);
        Ok(out)
    }
    fn decrypt_bytes(&self, data: &[u8]) -> privstack_crypto::EncryptorResult<Vec<u8>> {
        if data.starts_with(b"ENC:") {
            Ok(data[4..].to_vec())
        } else {
            Err(privstack_crypto::EncryptorError::Crypto("not encrypted".into()))
        }
    }
    fn reencrypt_bytes(&self, _data: &[u8], _old: &[u8], _new: &[u8]) -> privstack_crypto::EncryptorResult<Vec<u8>> {
        Err(privstack_crypto::EncryptorError::Crypto("not supported".into()))
    }
    fn is_available(&self) -> bool {
        true
    }
}

#[test]
fn migrate_unencrypted_encrypts_raw_blobs() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("migrate_enc.db");

    // Store blobs without encryption (passthrough)
    {
        let store = BlobStore::open(&db_path).unwrap();
        store.store("ns", "b1", b"plain1", None).unwrap();
        store.store("ns", "b2", b"plain2", None).unwrap();
    }

    // Migrate with EncryptOnlyEncryptor — decrypt fails on raw data → encrypts them
    {
        let store = BlobStore::open_with_encryptor(&db_path, Arc::new(EncryptOnlyEncryptor)).unwrap();
        let count = store.migrate_unencrypted().unwrap();
        assert_eq!(count, 2);
    }

    // After migration, reading with same encryptor should decrypt successfully
    {
        let store = BlobStore::open_with_encryptor(&db_path, Arc::new(EncryptOnlyEncryptor)).unwrap();
        let data = store.read("ns", "b1").unwrap();
        assert_eq!(data, b"plain1");
    }
}

#[test]
fn migrate_unencrypted_skips_already_encrypted() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("migrate_skip.db");

    // Store with EncryptOnlyEncryptor (data will be "encrypted")
    {
        let store = BlobStore::open_with_encryptor(&db_path, Arc::new(EncryptOnlyEncryptor)).unwrap();
        store.store("ns", "b1", b"already_enc", None).unwrap();
    }

    // Migrate — decrypt succeeds → skip
    {
        let store = BlobStore::open_with_encryptor(&db_path, Arc::new(EncryptOnlyEncryptor)).unwrap();
        let count = store.migrate_unencrypted().unwrap();
        assert_eq!(count, 0);
    }
}

#[test]
fn migrate_unencrypted_mixed_blobs() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("migrate_mixed.db");

    // Store one blob encrypted, one plain
    {
        let store = BlobStore::open_with_encryptor(&db_path, Arc::new(EncryptOnlyEncryptor)).unwrap();
        store.store("ns", "encrypted_one", b"secret", None).unwrap();
    }
    {
        let store = BlobStore::open(&db_path).unwrap();
        store.store("ns", "plain_one", b"visible", None).unwrap();
    }

    // Migrate — only the plain one gets encrypted
    {
        let store = BlobStore::open_with_encryptor(&db_path, Arc::new(EncryptOnlyEncryptor)).unwrap();
        let count = store.migrate_unencrypted().unwrap();
        assert_eq!(count, 1);
    }
}

// ── re_encrypt_all with successful encryptor ─────────────────────

/// An encryptor that actually does reencrypt
struct ReencryptEncryptor;

impl privstack_crypto::DataEncryptor for ReencryptEncryptor {
    fn encrypt_bytes(&self, _entity_id: &str, data: &[u8]) -> privstack_crypto::EncryptorResult<Vec<u8>> {
        Ok(data.to_vec())
    }
    fn decrypt_bytes(&self, data: &[u8]) -> privstack_crypto::EncryptorResult<Vec<u8>> {
        Ok(data.to_vec())
    }
    fn reencrypt_bytes(&self, data: &[u8], _old: &[u8], _new: &[u8]) -> privstack_crypto::EncryptorResult<Vec<u8>> {
        // Just return same data to simulate successful reencrypt
        Ok(data.to_vec())
    }
    fn is_available(&self) -> bool {
        true
    }
}

#[test]
fn re_encrypt_all_counts_successful_rows() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("reenc_ok.db");
    let store = BlobStore::open_with_encryptor(&db_path, Arc::new(ReencryptEncryptor)).unwrap();
    store.store("ns", "b1", b"data1", None).unwrap();
    store.store("ns", "b2", b"data2", None).unwrap();
    store.store("other", "b3", b"data3", None).unwrap();

    let count = store.re_encrypt_all(b"old_key", b"new_key").unwrap();
    assert_eq!(count, 3);
}

// ── open_with_encryptor file-backed path ─────────────────────────

#[test]
fn open_with_encryptor_file_backed() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("enc_file.db");
    let store = BlobStore::open_with_encryptor(
        &db_path,
        Arc::new(privstack_crypto::PassthroughEncryptor),
    ).unwrap();
    store.store("ns", "b1", b"file data", None).unwrap();
    assert_eq!(store.read("ns", "b1").unwrap(), b"file data");
}

// ── Poisoned mutex tests — trigger .map_err closures on conn.lock() ──

/// Helper: create a BlobStore with a shared conn, poison it, then return the store.
fn make_poisoned_store() -> BlobStore {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let shared = Arc::new(std::sync::Mutex::new(conn));
    let store = BlobStore::open_with_conn(shared.clone()).unwrap();
    // Poison the mutex by panicking while holding the lock
    let shared2 = shared.clone();
    let _ = std::thread::spawn(move || {
        let _guard = shared2.lock().unwrap();
        panic!("intentional poison");
    })
    .join();
    store
}

#[test]
fn store_with_poisoned_mutex() {
    let store = make_poisoned_store();
    let result = store.store("ns", "b1", b"data", None);
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(msg.contains("poison"), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn read_with_poisoned_mutex() {
    let store = make_poisoned_store();
    let result = store.read("ns", "b1");
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(msg.contains("poison"), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn delete_with_poisoned_mutex() {
    let store = make_poisoned_store();
    let result = store.delete("ns", "b1");
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(msg.contains("poison"), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn list_with_poisoned_mutex() {
    let store = make_poisoned_store();
    let result = store.list("ns");
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(msg.contains("poison"), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn update_metadata_with_poisoned_mutex() {
    let store = make_poisoned_store();
    let result = store.update_metadata("ns", "b1", "{}");
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(msg.contains("poison"), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn re_encrypt_all_with_poisoned_mutex() {
    let store = make_poisoned_store();
    let result = store.re_encrypt_all(b"old", b"new");
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(msg.contains("poison"), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn migrate_unencrypted_with_poisoned_mutex() {
    let store = make_poisoned_store();
    let result = store.migrate_unencrypted();
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(msg.contains("poison"), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn ensure_tables_with_poisoned_conn() {
    // Poison before open_with_conn to trigger ensure_tables failure
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let shared = Arc::new(std::sync::Mutex::new(conn));
    let shared2 = shared.clone();
    let _ = std::thread::spawn(move || {
        let _guard = shared2.lock().unwrap();
        panic!("intentional poison");
    })
    .join();
    let result = BlobStore::open_with_conn(shared);
    assert!(result.is_err());
    match result.err().unwrap() {
        BlobStoreError::Storage(msg) => assert!(msg.contains("poison"), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

// ── Dropped-table tests — trigger SQL execution .map_err closures ──

/// Helper: create a store with shared conn, then drop the blobs table to cause SQL failures.
fn make_broken_store() -> BlobStore {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let shared = Arc::new(std::sync::Mutex::new(conn));
    let store = BlobStore::open_with_conn(shared.clone()).unwrap();
    // Store some data first
    store.store("ns", "b1", b"data", None).unwrap();
    // Now drop the table to cause subsequent SQL operations to fail
    {
        let c = shared.lock().unwrap();
        c.execute_batch("DROP TABLE blobs").unwrap();
    }
    store
}

#[test]
fn store_with_dropped_table() {
    let store = make_broken_store();
    let result = store.store("ns", "b2", b"data", None);
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(!msg.is_empty(), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn read_with_dropped_table() {
    let store = make_broken_store();
    // read uses query_row which returns NotFound on error
    let result = store.read("ns", "b1");
    assert!(result.is_err());
}

#[test]
fn delete_with_dropped_table() {
    let store = make_broken_store();
    let result = store.delete("ns", "b1");
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(!msg.is_empty(), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn list_with_dropped_table() {
    let store = make_broken_store();
    let result = store.list("ns");
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(!msg.is_empty(), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn update_metadata_with_dropped_table() {
    let store = make_broken_store();
    let result = store.update_metadata("ns", "b1", "{}");
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(!msg.is_empty(), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn re_encrypt_all_with_dropped_table() {
    let store = make_broken_store();
    let result = store.re_encrypt_all(b"old", b"new");
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(!msg.is_empty(), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn migrate_unencrypted_with_dropped_table() {
    let store = make_broken_store();
    let result = store.migrate_unencrypted();
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(!msg.is_empty(), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

// ── Invalid path tests — trigger Connection::open .map_err closures ──

#[test]
fn open_with_invalid_path() {
    let result = BlobStore::open(std::path::Path::new("/nonexistent/dir/that/does/not/exist/db.duckdb"));
    assert!(result.is_err());
    match result.err().unwrap() {
        BlobStoreError::Storage(msg) => assert!(!msg.is_empty(), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn open_with_encryptor_invalid_path() {
    let result = BlobStore::open_with_encryptor(
        std::path::Path::new("/nonexistent/dir/that/does/not/exist/db.duckdb"),
        Arc::new(privstack_crypto::PassthroughEncryptor),
    );
    assert!(result.is_err());
    match result.err().unwrap() {
        BlobStoreError::Storage(msg) => assert!(!msg.is_empty(), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}
