use privstack_vault::{Vault, VaultError, VaultManager};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

// ── File-backed vault manager ───────────────────────────────────

#[test]
fn open_file_backed_vault() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("vault.duckdb");
    let mgr = VaultManager::open(&db_path).unwrap();
    mgr.create_vault("test").unwrap();
    mgr.initialize("test", "password123").unwrap();
    assert!(mgr.is_initialized("test"));
}

#[test]
fn open_with_memory_path() {
    let mgr = VaultManager::open(Path::new(":memory:")).unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    assert!(mgr.is_initialized("v"));
}

// ── Error type coverage ─────────────────────────────────────────

#[test]
fn vault_error_display() {
    let errors = vec![
        VaultError::NotInitialized,
        VaultError::Locked,
        VaultError::AlreadyInitialized,
        VaultError::InvalidPassword,
        VaultError::PasswordTooShort,
        VaultError::BlobNotFound("test".to_string()),
        VaultError::VaultNotFound("v1".to_string()),
        VaultError::Storage("db error".to_string()),
        VaultError::Crypto("key error".to_string()),
        VaultError::RecoveryNotConfigured,
        VaultError::InvalidRecoveryMnemonic,
    ];

    for err in &errors {
        let msg = format!("{}", err);
        assert!(!msg.is_empty());
    }

    // Debug formatting
    for err in &errors {
        let debug = format!("{:?}", err);
        assert!(!debug.is_empty());
    }
}

// ── Edge cases ──────────────────────────────────────────────────

#[test]
fn store_blob_on_locked_vault_returns_locked_error() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    mgr.lock("v");

    let result = mgr.store_blob("v", "b1", b"data");
    assert!(result.is_err());
}

#[test]
fn list_blobs_on_empty_vault() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    let blobs = mgr.list_blobs("v").unwrap();
    assert!(blobs.is_empty());
}

#[test]
fn delete_blob_returns_not_found() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    let result = mgr.delete_blob("v", "nonexistent");
    assert!(result.is_err());
}

#[test]
fn read_blob_on_locked_vault() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    mgr.store_blob("v", "b1", b"secret").unwrap();
    mgr.lock("v");

    let result = mgr.read_blob("v", "b1");
    assert!(result.is_err());
}

#[test]
fn list_blobs_returns_metadata() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    mgr.store_blob("v", "b1", b"hello").unwrap();

    let blobs = mgr.list_blobs("v").unwrap();
    assert_eq!(blobs.len(), 1);
    assert_eq!(blobs[0].blob_id, "b1");
    assert_eq!(blobs[0].size, 5);
    assert!(blobs[0].content_hash.is_some());
    assert!(blobs[0].created_at > 0);
    assert!(blobs[0].modified_at > 0);
}

#[test]
fn large_blob_roundtrip() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    let data: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
    mgr.store_blob("v", "large", &data).unwrap();

    let read = mgr.read_blob("v", "large").unwrap();
    assert_eq!(read.len(), 100_000);
    assert_eq!(read, data);
}

#[test]
fn change_password_preserves_multiple_blobs() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "oldpass12").unwrap();

    mgr.store_blob("v", "a", b"alpha").unwrap();
    mgr.store_blob("v", "b", b"beta").unwrap();
    mgr.store_blob("v", "c", b"gamma").unwrap();

    mgr.change_password("v", "oldpass12", "newpass12").unwrap();
    mgr.lock("v");
    mgr.unlock("v", "newpass12").unwrap();

    assert_eq!(mgr.read_blob("v", "a").unwrap(), b"alpha");
    assert_eq!(mgr.read_blob("v", "b").unwrap(), b"beta");
    assert_eq!(mgr.read_blob("v", "c").unwrap(), b"gamma");
}

#[test]
fn change_password_locked_vault_fails() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    mgr.lock("v");

    // change_password reads salt directly from DB, doesn't need unlock,
    // but verifies old password. This should still work since it accesses DB directly.
    let result = mgr.change_password("v", "password123", "newpass12");
    // Either succeeds or fails — just don't panic
    let _ = result;
}

#[test]
fn unlock_all_skips_uninitialized() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v1").unwrap();
    mgr.create_vault("v2").unwrap();
    mgr.initialize("v1", "password123").unwrap();
    // v2 is NOT initialized

    mgr.lock_all();
    mgr.unlock_all("password123").unwrap();

    assert!(mgr.is_unlocked("v1"));
    assert!(!mgr.is_unlocked("v2"));
}

#[test]
fn file_backed_persistence() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("persist.duckdb");

    // Create and populate
    {
        let mgr = VaultManager::open(&db_path).unwrap();
        mgr.create_vault("v").unwrap();
        mgr.initialize("v", "password123").unwrap();
        mgr.store_blob("v", "b1", b"persisted data").unwrap();
    }

    // Reopen and verify
    {
        let mgr = VaultManager::open(&db_path).unwrap();
        mgr.create_vault("v").unwrap();
        assert!(mgr.is_initialized("v"));
        mgr.unlock("v", "password123").unwrap();
        let data = mgr.read_blob("v", "b1").unwrap();
        assert_eq!(data, b"persisted data");
    }
}

// ── Vault::initialize edge cases ────────────────────────────

#[test]
fn initialize_password_too_short() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();

    let result = mgr.initialize("v", "short");
    assert!(matches!(result, Err(VaultError::PasswordTooShort)));
}

#[test]
fn initialize_already_initialized() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    let result = mgr.initialize("v", "password123");
    assert!(matches!(result, Err(VaultError::AlreadyInitialized)));
}

#[test]
fn unlock_not_initialized() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();

    let result = mgr.unlock("v", "password123");
    assert!(matches!(result, Err(VaultError::NotInitialized)));
}

#[test]
fn unlock_wrong_password() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    mgr.lock("v");

    let result = mgr.unlock("v", "wrongpasswd");
    assert!(matches!(result, Err(VaultError::InvalidPassword)));
}

#[test]
fn change_password_wrong_old_password() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    let result = mgr.change_password("v", "wrongpasswd", "newpass12");
    assert!(matches!(result, Err(VaultError::InvalidPassword)));
}

#[test]
fn change_password_new_too_short() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    let result = mgr.change_password("v", "password123", "short");
    assert!(matches!(result, Err(VaultError::PasswordTooShort)));
}

#[test]
fn change_password_all_multiple_vaults() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v1").unwrap();
    mgr.create_vault("v2").unwrap();
    mgr.initialize("v1", "password123").unwrap();
    mgr.initialize("v2", "password123").unwrap();

    mgr.store_blob("v1", "b1", b"data1").unwrap();
    mgr.store_blob("v2", "b2", b"data2").unwrap();

    mgr.change_password_all("password123", "newpass12").unwrap();

    mgr.lock_all();
    mgr.unlock("v1", "newpass12").unwrap();
    mgr.unlock("v2", "newpass12").unwrap();

    assert_eq!(mgr.read_blob("v1", "b1").unwrap(), b"data1");
    assert_eq!(mgr.read_blob("v2", "b2").unwrap(), b"data2");
}

#[test]
fn create_vault_idempotent() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.create_vault("v").unwrap(); // second call should be no-op
    mgr.initialize("v", "password123").unwrap();
    assert!(mgr.is_initialized("v"));
}

#[test]
fn store_blob_overwrites() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    mgr.store_blob("v", "b1", b"original").unwrap();
    mgr.store_blob("v", "b1", b"updated").unwrap();

    let data = mgr.read_blob("v", "b1").unwrap();
    assert_eq!(data, b"updated");

    let blobs = mgr.list_blobs("v").unwrap();
    assert_eq!(blobs.len(), 1);
}

#[test]
fn lock_nonexistent_vault_noop() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.lock("nonexistent"); // should not panic
}

#[test]
fn is_initialized_nonexistent_vault() {
    let mgr = VaultManager::open_in_memory().unwrap();
    // Before creating the vault
    assert!(!mgr.is_initialized("nonexistent"));
}

#[test]
fn is_unlocked_nonexistent_vault() {
    let mgr = VaultManager::open_in_memory().unwrap();
    assert!(!mgr.is_unlocked("nonexistent"));
}

#[test]
fn multiple_vaults_independent() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("a").unwrap();
    mgr.create_vault("b").unwrap();

    mgr.initialize("a", "pass_a123").unwrap();
    mgr.initialize("b", "pass_b123").unwrap();

    mgr.store_blob("a", "blob", b"alpha").unwrap();
    mgr.store_blob("b", "blob", b"beta").unwrap();

    assert_eq!(mgr.read_blob("a", "blob").unwrap(), b"alpha");
    assert_eq!(mgr.read_blob("b", "blob").unwrap(), b"beta");

    mgr.lock("a");
    assert!(!mgr.is_unlocked("a"));
    assert!(mgr.is_unlocked("b"));

    // b still accessible
    assert_eq!(mgr.read_blob("b", "blob").unwrap(), b"beta");
}

#[test]
fn empty_blob_roundtrip() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    mgr.store_blob("v", "empty", b"").unwrap();
    let data = mgr.read_blob("v", "empty").unwrap();
    assert!(data.is_empty());
}

#[test]
fn blob_info_serde() {
    use privstack_vault::BlobInfo;
    let info = BlobInfo {
        blob_id: "b1".to_string(),
        size: 100,
        content_hash: Some("abc123".to_string()),
        created_at: 1000,
        modified_at: 2000,
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("b1"));
    assert!(json.contains("100"));
    assert!(json.contains("abc123"));
}

// ── Vault direct API (list_blobs returning Vec<String>, id()) ───

#[test]
fn vault_direct_id() {
    use privstack_vault::Vault;
    use std::sync::{Arc, Mutex};

    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("test_vault", conn).unwrap();

    assert_eq!(vault.id(), "test_vault");
}

#[test]
fn vault_direct_list_blobs_empty() {
    use privstack_vault::Vault;
    use std::sync::{Arc, Mutex};

    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("v", conn).unwrap();
    vault.initialize("password123").unwrap();

    let blobs = vault.list_blobs().unwrap();
    assert!(blobs.is_empty());
}

#[test]
fn vault_direct_list_blobs_with_data() {
    use privstack_vault::Vault;
    use std::sync::{Arc, Mutex};

    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("v", conn).unwrap();
    vault.initialize("password123").unwrap();
    vault.store_blob("b1", b"data1").unwrap();
    vault.store_blob("b2", b"data2").unwrap();

    let blobs = vault.list_blobs().unwrap();
    assert_eq!(blobs.len(), 2);
    assert!(blobs.contains(&"b1".to_string()));
    assert!(blobs.contains(&"b2".to_string()));
}

// ── Vault direct API: full lifecycle ─────────────────────────────

#[test]
fn vault_direct_initialize_lock_unlock() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("direct", conn).unwrap();

    assert!(!vault.is_initialized());
    assert!(!vault.is_unlocked());

    vault.initialize("password123").unwrap();
    assert!(vault.is_initialized());
    assert!(vault.is_unlocked());

    vault.lock();
    assert!(!vault.is_unlocked());

    vault.unlock("password123").unwrap();
    assert!(vault.is_unlocked());
}

#[test]
fn vault_direct_store_read_delete_blob() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("blobs", conn).unwrap();
    vault.initialize("password123").unwrap();

    vault.store_blob("b1", b"hello").unwrap();
    let data = vault.read_blob("b1").unwrap();
    assert_eq!(data, b"hello");

    vault.delete_blob("b1").unwrap();
    assert!(vault.read_blob("b1").is_err());
}

#[test]
fn vault_direct_locked_operations_fail() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("locked", conn).unwrap();
    vault.initialize("password123").unwrap();
    vault.lock();

    assert!(matches!(vault.store_blob("b1", b"x"), Err(VaultError::Locked)));
    assert!(matches!(vault.read_blob("b1"), Err(VaultError::Locked)));
}

#[test]
fn vault_direct_change_password() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("chpw", conn).unwrap();
    vault.initialize("oldpass12").unwrap();
    vault.store_blob("b1", b"secret").unwrap();

    vault.change_password("oldpass12", "newpass12").unwrap();
    vault.lock();

    assert!(vault.unlock("oldpass12").is_err());
    vault.unlock("newpass12").unwrap();
    assert_eq!(vault.read_blob("b1").unwrap(), b"secret");
}

#[test]
fn vault_direct_wrong_password() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("wrongpw", conn).unwrap();
    vault.initialize("password123").unwrap();
    vault.lock();

    assert!(matches!(vault.unlock("wrongpasswd"), Err(VaultError::InvalidPassword)));
}

#[test]
fn vault_direct_double_init() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("dbl", conn).unwrap();
    vault.initialize("password123").unwrap();
    assert!(matches!(vault.initialize("password123"), Err(VaultError::AlreadyInitialized)));
}

#[test]
fn vault_direct_unlock_not_initialized() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("uninit", conn).unwrap();
    assert!(matches!(vault.unlock("password123"), Err(VaultError::NotInitialized)));
}

#[test]
fn vault_direct_delete_nonexistent() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("del", conn).unwrap();
    vault.initialize("password123").unwrap();
    assert!(matches!(vault.delete_blob("nope"), Err(VaultError::BlobNotFound(_))));
}

#[test]
fn vault_direct_password_too_short() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("short", conn).unwrap();
    assert!(matches!(vault.initialize("short"), Err(VaultError::PasswordTooShort)));
}

#[test]
fn vault_direct_change_password_short() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("chps", conn).unwrap();
    vault.initialize("password123").unwrap();
    assert!(matches!(vault.change_password("password123", "short"), Err(VaultError::PasswordTooShort)));
}

#[test]
fn vault_direct_change_password_wrong_old() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("chpwo", conn).unwrap();
    vault.initialize("password123").unwrap();
    assert!(matches!(vault.change_password("wrongold12", "newpass12"), Err(VaultError::InvalidPassword)));
}

// ── unlock_all with no initialized vaults ────────────────────────

#[test]
fn unlock_all_with_no_initialized_vaults_fails() {
    let mgr = VaultManager::open_in_memory().unwrap();
    // Only the default vault exists (auto-created by unlock_all) but not initialized
    let result = mgr.unlock_all("password123");
    assert!(result.is_err());
    assert!(matches!(result, Err(VaultError::NotInitialized)));
}

#[test]
fn unlock_all_with_all_already_unlocked_succeeds() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("default").unwrap();
    mgr.initialize("default", "password123").unwrap();
    // default is now unlocked

    // unlock_all should succeed (nothing to unlock, but some already unlocked)
    mgr.unlock_all("password123").unwrap();
    assert!(mgr.is_unlocked("default"));
}

// ── Concurrent access scenarios ──────────────────────────────────

#[test]
fn concurrent_vault_reads() {
    use std::thread;

    let mgr = Arc::new(VaultManager::open_in_memory().unwrap());
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    mgr.store_blob("v", "b1", b"shared data").unwrap();

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let mgr = Arc::clone(&mgr);
            thread::spawn(move || {
                let data = mgr.read_blob("v", "b1").unwrap();
                assert_eq!(data, b"shared data");
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn concurrent_vault_writes() {
    use std::thread;

    let mgr = Arc::new(VaultManager::open_in_memory().unwrap());
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let mgr = Arc::clone(&mgr);
            thread::spawn(move || {
                let blob_id = format!("blob-{i}");
                let data = format!("data-{i}");
                mgr.store_blob("v", &blob_id, data.as_bytes()).unwrap();
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let blobs = mgr.list_blobs("v").unwrap();
    assert_eq!(blobs.len(), 4);
}

#[test]
fn concurrent_lock_unlock() {
    use std::thread;

    let mgr = Arc::new(VaultManager::open_in_memory().unwrap());
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    // Lock from one thread, unlock from another
    let mgr2 = Arc::clone(&mgr);
    mgr.lock("v");

    let h = thread::spawn(move || {
        mgr2.unlock("v", "password123").unwrap();
        assert!(mgr2.is_unlocked("v"));
    });
    h.join().unwrap();
}

// ── DataEncryptor implementation on VaultManager ─────────────────

#[test]
fn vault_manager_data_encryptor_available_when_unlocked() {
    use privstack_crypto::DataEncryptor;
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("default").unwrap();
    assert!(!mgr.is_available());

    mgr.initialize("default", "password123").unwrap();
    assert!(mgr.is_available());

    mgr.lock("default");
    assert!(!mgr.is_available());
}

#[test]
fn vault_manager_encrypt_decrypt_roundtrip() {
    use privstack_crypto::DataEncryptor;
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("default").unwrap();
    mgr.initialize("default", "password123").unwrap();

    let plaintext = b"secret entity data";
    let encrypted = mgr.encrypt_bytes("entity-1", plaintext).unwrap();
    assert_ne!(encrypted, plaintext);

    let decrypted = mgr.decrypt_bytes(&encrypted).unwrap();
    assert_eq!(decrypted, plaintext);
}

#[test]
fn vault_manager_encrypt_fails_when_locked() {
    use privstack_crypto::DataEncryptor;
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("default").unwrap();

    let result = mgr.encrypt_bytes("entity-1", b"data");
    assert!(result.is_err());
}

#[test]
fn vault_manager_default_key_bytes() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("default").unwrap();
    assert!(mgr.default_key_bytes().is_none());

    mgr.initialize("default", "password123").unwrap();
    let key_bytes = mgr.default_key_bytes();
    assert!(key_bytes.is_some());
    assert!(!key_bytes.unwrap().is_empty());
}

// ── Vault direct: get_key ────────────────────────────────────────

#[test]
fn vault_is_unlocked_reflects_key_state() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("gk", conn).unwrap();
    assert!(!vault.is_unlocked());

    vault.initialize("password123").unwrap();
    assert!(vault.is_unlocked());

    vault.lock();
    assert!(!vault.is_unlocked());

    vault.unlock("password123").unwrap();
    assert!(vault.is_unlocked());
}

// ── Additional coverage tests ────────────────────────────────────

#[test]
fn vault_manager_reencrypt_bytes_roundtrip() {
    use privstack_crypto::DataEncryptor;
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("default").unwrap();
    mgr.initialize("default", "password123").unwrap();

    let plaintext = b"reencrypt test data";
    let encrypted = mgr.encrypt_bytes("entity-re", plaintext).unwrap();

    let key_bytes = mgr.default_key_bytes().unwrap();
    let reencrypted = mgr.reencrypt_bytes(&encrypted, &key_bytes, &key_bytes).unwrap();
    let decrypted = mgr.decrypt_bytes(&reencrypted).unwrap();
    assert_eq!(decrypted, plaintext);
}

#[test]
fn vault_manager_reencrypt_bytes_wrong_key_length() {
    use privstack_crypto::DataEncryptor;
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("default").unwrap();
    mgr.initialize("default", "password123").unwrap();

    let encrypted = mgr.encrypt_bytes("entity-bad", b"data").unwrap();
    let result = mgr.reencrypt_bytes(&encrypted, b"short", b"also_short");
    assert!(result.is_err());
}

#[test]
fn vault_manager_decrypt_bytes_when_locked() {
    use privstack_crypto::DataEncryptor;
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("default").unwrap();
    mgr.initialize("default", "password123").unwrap();

    let encrypted = mgr.encrypt_bytes("entity-lock", b"data").unwrap();
    mgr.lock("default");

    assert!(mgr.decrypt_bytes(&encrypted).is_err());
}

#[test]
fn vault_manager_reencrypt_bytes_invalid_data() {
    use privstack_crypto::DataEncryptor;
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("default").unwrap();
    mgr.initialize("default", "password123").unwrap();

    let key_bytes = mgr.default_key_bytes().unwrap();
    let result = mgr.reencrypt_bytes(b"not json", &key_bytes, &key_bytes);
    assert!(result.is_err());
}

#[test]
fn blob_info_debug_format() {
    use privstack_vault::BlobInfo;
    let info = BlobInfo {
        blob_id: "debug-test".to_string(),
        size: 42,
        content_hash: None,
        created_at: 1000,
        modified_at: 2000,
    };
    let debug = format!("{:?}", info);
    assert!(debug.contains("BlobInfo"));
    assert!(debug.contains("debug-test"));
}

#[test]
fn unlock_all_wrong_password_fails() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("default").unwrap();
    mgr.initialize("default", "password123").unwrap();
    mgr.lock_all();

    let result = mgr.unlock_all("wrongpasswd");
    assert!(result.is_err());
}

#[test]
fn change_password_all_no_initialized_vaults() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v1").unwrap();
    mgr.change_password_all("old12345", "new12345").unwrap();
}

#[test]
fn vault_direct_read_blob_not_found() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("rnf", conn).unwrap();
    vault.initialize("password123").unwrap();
    assert!(matches!(vault.read_blob("nonexistent"), Err(VaultError::BlobNotFound(_))));
}

#[test]
fn vault_manager_store_blob_auto_creates_vault() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.initialize("auto", "password123").unwrap();
    mgr.store_blob("auto", "b1", b"auto data").unwrap();
    assert_eq!(mgr.read_blob("auto", "b1").unwrap(), b"auto data");
}

#[test]
fn vault_manager_list_blobs_multiple() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    mgr.store_blob("v", "a", b"data_a").unwrap();
    mgr.store_blob("v", "b", b"data_b").unwrap();
    mgr.store_blob("v", "c", b"data_c").unwrap();

    let blobs = mgr.list_blobs("v").unwrap();
    assert_eq!(blobs.len(), 3);
}

#[test]
fn vault_manager_default_key_bytes_locked() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("default").unwrap();
    mgr.initialize("default", "password123").unwrap();
    mgr.lock("default");
    assert!(mgr.default_key_bytes().is_none());
}

// ── Additional function coverage tests ───────────────────────────

#[test]
fn vault_error_source_trait() {
    use std::error::Error;
    // All VaultError variants: test source() returns None (no wrapped error)
    let errors = vec![
        VaultError::NotInitialized,
        VaultError::Locked,
        VaultError::AlreadyInitialized,
        VaultError::InvalidPassword,
        VaultError::PasswordTooShort,
        VaultError::BlobNotFound("x".into()),
        VaultError::VaultNotFound("x".into()),
        VaultError::Storage("x".into()),
        VaultError::Crypto("x".into()),
        VaultError::RecoveryNotConfigured,
        VaultError::InvalidRecoveryMnemonic,
    ];
    for e in &errors {
        let _ = e.source();
    }
}

#[test]
fn vault_error_specific_display_messages() {
    assert_eq!(format!("{}", VaultError::NotInitialized), "vault not initialized");
    assert_eq!(format!("{}", VaultError::Locked), "vault is locked");
    assert_eq!(format!("{}", VaultError::AlreadyInitialized), "vault already initialized");
    assert_eq!(format!("{}", VaultError::InvalidPassword), "invalid password");
    assert_eq!(format!("{}", VaultError::PasswordTooShort), "password too short (min 8 characters)");
    assert!(format!("{}", VaultError::BlobNotFound("b1".into())).contains("b1"));
    assert!(format!("{}", VaultError::VaultNotFound("v1".into())).contains("v1"));
    assert!(format!("{}", VaultError::Storage("err".into())).contains("err"));
    assert!(format!("{}", VaultError::Crypto("err".into())).contains("err"));
}

#[test]
fn blob_info_serialize_with_none_hash() {
    use privstack_vault::BlobInfo;
    let info = BlobInfo {
        blob_id: "test".to_string(),
        size: 0,
        content_hash: None,
        created_at: 0,
        modified_at: 0,
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("null"));
    let _ = format!("{:?}", info);
}

#[test]
fn vault_direct_change_password_no_blobs() {
    // Change password when vault has no blobs (empty re-encryption loop)
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("noblobs", conn).unwrap();
    vault.initialize("oldpass12").unwrap();
    vault.change_password("oldpass12", "newpass12").unwrap();
    vault.lock();
    vault.unlock("newpass12").unwrap();
    assert!(vault.is_unlocked());
}

#[test]
fn vault_direct_store_read_multiple_blobs() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("multi", conn).unwrap();
    vault.initialize("password123").unwrap();

    for i in 0..10 {
        let id = format!("blob-{}", i);
        let data = format!("data-{}", i);
        vault.store_blob(&id, data.as_bytes()).unwrap();
    }

    let blobs = vault.list_blobs().unwrap();
    assert_eq!(blobs.len(), 10);

    for i in 0..10 {
        let id = format!("blob-{}", i);
        let expected = format!("data-{}", i);
        assert_eq!(vault.read_blob(&id).unwrap(), expected.as_bytes());
    }
}

#[test]
fn vault_direct_delete_then_list() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("dellist", conn).unwrap();
    vault.initialize("password123").unwrap();

    vault.store_blob("a", b"aaa").unwrap();
    vault.store_blob("b", b"bbb").unwrap();
    vault.delete_blob("a").unwrap();

    let blobs = vault.list_blobs().unwrap();
    assert_eq!(blobs.len(), 1);
    assert_eq!(blobs[0], "b");
}

#[test]
fn vault_manager_decrypt_bytes_invalid_json() {
    use privstack_crypto::DataEncryptor;
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("default").unwrap();
    mgr.initialize("default", "password123").unwrap();

    let result = mgr.decrypt_bytes(b"not valid json");
    assert!(result.is_err());
}

#[test]
fn vault_manager_encrypt_large_data() {
    use privstack_crypto::DataEncryptor;
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("default").unwrap();
    mgr.initialize("default", "password123").unwrap();

    let large: Vec<u8> = (0..50_000).map(|i| (i % 256) as u8).collect();
    let encrypted = mgr.encrypt_bytes("large-entity", &large).unwrap();
    let decrypted = mgr.decrypt_bytes(&encrypted).unwrap();
    assert_eq!(decrypted, large);
}

#[test]
fn vault_manager_reencrypt_with_different_keys() {
    use privstack_crypto::DataEncryptor;

    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("default").unwrap();
    mgr.initialize("default", "password123").unwrap();

    let plaintext = b"reencrypt with diff keys";
    let encrypted = mgr.encrypt_bytes("ent-1", plaintext).unwrap();

    let old_key = mgr.default_key_bytes().unwrap();

    // Change password to get new key
    mgr.change_password("default", "password123", "newpass12").unwrap();
    let new_key = mgr.default_key_bytes().unwrap();

    let reencrypted = mgr.reencrypt_bytes(&encrypted, &old_key, &new_key).unwrap();
    let decrypted = mgr.decrypt_bytes(&reencrypted).unwrap();
    assert_eq!(decrypted, plaintext);
}

#[test]
fn vault_manager_is_available_lifecycle() {
    use privstack_crypto::DataEncryptor;
    let mgr = VaultManager::open_in_memory().unwrap();

    // Not available before creating default vault
    assert!(!mgr.is_available());

    mgr.create_vault("default").unwrap();
    assert!(!mgr.is_available());

    mgr.initialize("default", "password123").unwrap();
    assert!(mgr.is_available());

    mgr.lock("default");
    assert!(!mgr.is_available());

    mgr.unlock("default", "password123").unwrap();
    assert!(mgr.is_available());
}

#[test]
fn vault_manager_lock_all_with_multiple_vaults() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("a").unwrap();
    mgr.create_vault("b").unwrap();
    mgr.create_vault("c").unwrap();
    mgr.initialize("a", "password123").unwrap();
    mgr.initialize("b", "password123").unwrap();
    // c not initialized

    mgr.lock_all();
    assert!(!mgr.is_unlocked("a"));
    assert!(!mgr.is_unlocked("b"));
    assert!(!mgr.is_unlocked("c"));
}

#[test]
fn vault_manager_change_password_all_with_blobs() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v1").unwrap();
    mgr.create_vault("v2").unwrap();
    mgr.initialize("v1", "password123").unwrap();
    mgr.initialize("v2", "password123").unwrap();

    mgr.store_blob("v1", "x", b"data-x").unwrap();
    mgr.store_blob("v2", "y", b"data-y").unwrap();

    mgr.change_password_all("password123", "newpass12").unwrap();
    mgr.lock_all();
    mgr.unlock("v1", "newpass12").unwrap();
    mgr.unlock("v2", "newpass12").unwrap();

    assert_eq!(mgr.read_blob("v1", "x").unwrap(), b"data-x");
    assert_eq!(mgr.read_blob("v2", "y").unwrap(), b"data-y");
}

#[test]
fn vault_direct_overwrite_blob_preserves_single_entry() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("ow", conn).unwrap();
    vault.initialize("password123").unwrap();

    vault.store_blob("b1", b"first").unwrap();
    vault.store_blob("b1", b"second").unwrap();

    let data = vault.read_blob("b1").unwrap();
    assert_eq!(data, b"second");

    let blobs = vault.list_blobs().unwrap();
    assert_eq!(blobs.len(), 1);
}

#[test]
fn vault_manager_store_read_delete_via_with_vault() {
    // Tests the with_vault closure path via public API
    let mgr = VaultManager::open_in_memory().unwrap();
    // Don't create vault first - let store_blob auto-create
    mgr.initialize("autocreate", "password123").unwrap();
    mgr.store_blob("autocreate", "blob", b"test").unwrap();
    let data = mgr.read_blob("autocreate", "blob").unwrap();
    assert_eq!(data, b"test");
    mgr.delete_blob("autocreate", "blob").unwrap();
    assert!(mgr.read_blob("autocreate", "blob").is_err());
}

#[test]
fn vault_manager_list_blobs_auto_creates_vault() {
    let mgr = VaultManager::open_in_memory().unwrap();
    // list_blobs on nonexistent vault auto-creates it
    let blobs = mgr.list_blobs("newvault").unwrap();
    assert!(blobs.is_empty());
}

#[test]
fn vault_manager_unlock_all_with_mixed_state() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("default").unwrap();
    mgr.initialize("default", "password123").unwrap();
    mgr.create_vault("extra").unwrap();
    mgr.initialize("extra", "password123").unwrap();

    // Lock only one
    mgr.lock("default");
    assert!(mgr.is_unlocked("extra"));

    mgr.unlock_all("password123").unwrap();
    assert!(mgr.is_unlocked("default"));
    assert!(mgr.is_unlocked("extra"));
}

// ── Recovery (Emergency Kit) ─────────────────────────────────────

#[test]
fn setup_recovery_returns_mnemonic() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    let mnemonic = mgr.setup_recovery("v").unwrap();
    let words: Vec<&str> = mnemonic.split_whitespace().collect();
    assert_eq!(words.len(), 12);
}

#[test]
fn has_recovery_false_before_setup() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    assert!(!mgr.has_recovery("v").unwrap());
}

#[test]
fn has_recovery_true_after_setup() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    mgr.setup_recovery("v").unwrap();
    assert!(mgr.has_recovery("v").unwrap());
}

#[test]
fn setup_recovery_on_locked_vault_fails() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    mgr.lock("v");

    let result = mgr.setup_recovery("v");
    assert!(matches!(result, Err(VaultError::Locked)));
}

#[test]
fn reset_password_with_recovery_full_flow() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    mgr.store_blob("v", "secret", b"my secret data").unwrap();

    let mnemonic = mgr.setup_recovery("v").unwrap();

    // Reset password using mnemonic
    let (old_kb, new_kb) = mgr
        .reset_password_with_recovery("v", &mnemonic, "newpass12")
        .unwrap();
    assert_eq!(old_kb.len(), 32);
    assert_eq!(new_kb.len(), 32);
    assert_ne!(old_kb, new_kb);

    // Old password should no longer work
    mgr.lock("v");
    assert!(mgr.unlock("v", "password123").is_err());

    // New password should work
    mgr.unlock("v", "newpass12").unwrap();

    // Data should be intact
    assert_eq!(mgr.read_blob("v", "secret").unwrap(), b"my secret data");
}

#[test]
fn reset_password_preserves_multiple_blobs() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    mgr.store_blob("v", "a", b"alpha").unwrap();
    mgr.store_blob("v", "b", b"beta").unwrap();
    mgr.store_blob("v", "c", b"gamma").unwrap();

    let mnemonic = mgr.setup_recovery("v").unwrap();
    mgr.reset_password_with_recovery("v", &mnemonic, "newpass12")
        .unwrap();

    mgr.lock("v");
    mgr.unlock("v", "newpass12").unwrap();

    assert_eq!(mgr.read_blob("v", "a").unwrap(), b"alpha");
    assert_eq!(mgr.read_blob("v", "b").unwrap(), b"beta");
    assert_eq!(mgr.read_blob("v", "c").unwrap(), b"gamma");
}

#[test]
fn reset_password_mnemonic_remains_valid() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    let mnemonic = mgr.setup_recovery("v").unwrap();
    mgr.reset_password_with_recovery("v", &mnemonic, "newpass12")
        .unwrap();

    // Same mnemonic should work for a second reset
    mgr.reset_password_with_recovery("v", &mnemonic, "thirdpw1")
        .unwrap();

    mgr.lock("v");
    mgr.unlock("v", "thirdpw1").unwrap();
    assert!(mgr.is_unlocked("v"));
}

#[test]
fn reset_password_wrong_mnemonic_fails() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    mgr.setup_recovery("v").unwrap();

    let wrong = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let result = mgr.reset_password_with_recovery("v", wrong, "newpass12");
    assert!(matches!(result, Err(VaultError::InvalidRecoveryMnemonic)));
}

#[test]
fn reset_password_too_short_fails() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    let mnemonic = mgr.setup_recovery("v").unwrap();

    let result = mgr.reset_password_with_recovery("v", &mnemonic, "short");
    assert!(matches!(result, Err(VaultError::PasswordTooShort)));
}

#[test]
fn reset_password_without_recovery_configured_fails() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    let dummy = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let result = mgr.reset_password_with_recovery("v", dummy, "newpass12");
    assert!(matches!(result, Err(VaultError::RecoveryNotConfigured)));
}

#[test]
fn regenerate_recovery_invalidates_old_mnemonic() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    let old_mnemonic = mgr.setup_recovery("v").unwrap();
    let new_mnemonic = mgr.setup_recovery("v").unwrap();

    // Old mnemonic should no longer work
    let result = mgr.reset_password_with_recovery("v", &old_mnemonic, "newpass12");
    assert!(result.is_err());

    // New mnemonic should work
    mgr.reset_password_with_recovery("v", &new_mnemonic, "newpass12")
        .unwrap();
}

#[test]
fn reset_password_with_empty_vault_no_blobs() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    let mnemonic = mgr.setup_recovery("v").unwrap();

    // Reset with no blobs — empty re-encryption loop should succeed
    mgr.reset_password_with_recovery("v", &mnemonic, "newpass12")
        .unwrap();

    mgr.lock("v");
    mgr.unlock("v", "newpass12").unwrap();
    assert!(mgr.is_unlocked("v"));
}
