use privstack_vault::VaultManager;

// ── Vault lifecycle ──────────────────────────────────────────────

#[test]
fn vault_init_unlock_lock() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("test").unwrap();
    assert!(!mgr.is_initialized("test"));

    mgr.initialize("test", "password123").unwrap();
    assert!(mgr.is_initialized("test"));
    assert!(mgr.is_unlocked("test"));

    mgr.lock("test");
    assert!(!mgr.is_unlocked("test"));

    mgr.unlock("test", "password123").unwrap();
    assert!(mgr.is_unlocked("test"));
}

#[test]
fn wrong_password_fails() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    mgr.lock("v");
    assert!(mgr.unlock("v", "wrongpass").is_err());
}

#[test]
fn password_too_short() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    assert!(mgr.initialize("v", "short").is_err());
}

#[test]
fn double_initialize_fails() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    assert!(mgr.initialize("v", "password123").is_err());
}

#[test]
fn unlock_uninitialized_fails() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    assert!(mgr.unlock("v", "password123").is_err());
}

#[test]
fn create_vault_idempotent() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.create_vault("v").unwrap(); // no error
}

// ── Blob operations ──────────────────────────────────────────────

#[test]
fn blob_store_read_delete() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("test").unwrap();
    mgr.initialize("test", "password123").unwrap();

    mgr.store_blob("test", "blob1", b"hello world").unwrap();
    let read = mgr.read_blob("test", "blob1").unwrap();
    assert_eq!(read, b"hello world");

    let blobs = mgr.list_blobs("test").unwrap();
    assert_eq!(blobs.len(), 1);
    assert_eq!(blobs[0].blob_id, "blob1");
    assert_eq!(blobs[0].size, 11);
    assert!(blobs[0].content_hash.is_some());

    mgr.delete_blob("test", "blob1").unwrap();
    assert!(mgr.read_blob("test", "blob1").is_err());
}

#[test]
fn locked_vault_rejects_blob_ops() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("test").unwrap();
    mgr.initialize("test", "password123").unwrap();
    mgr.lock("test");

    assert!(mgr.store_blob("test", "b1", b"data").is_err());
    assert!(mgr.read_blob("test", "b1").is_err());
}

#[test]
fn read_nonexistent_blob_fails() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    assert!(mgr.read_blob("v", "nope").is_err());
}

#[test]
fn delete_nonexistent_blob_fails() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    assert!(mgr.delete_blob("v", "nope").is_err());
}

#[test]
fn overwrite_blob() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    mgr.store_blob("v", "b1", b"v1").unwrap();
    mgr.store_blob("v", "b1", b"v2 updated").unwrap();

    let data = mgr.read_blob("v", "b1").unwrap();
    assert_eq!(data, b"v2 updated");

    let blobs = mgr.list_blobs("v").unwrap();
    assert_eq!(blobs.len(), 1);
}

#[test]
fn multiple_blobs() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    mgr.store_blob("v", "a", b"aaa").unwrap();
    mgr.store_blob("v", "b", b"bbb").unwrap();
    mgr.store_blob("v", "c", b"ccc").unwrap();

    let blobs = mgr.list_blobs("v").unwrap();
    assert_eq!(blobs.len(), 3);

    assert_eq!(mgr.read_blob("v", "a").unwrap(), b"aaa");
    assert_eq!(mgr.read_blob("v", "b").unwrap(), b"bbb");
    assert_eq!(mgr.read_blob("v", "c").unwrap(), b"ccc");
}

#[test]
fn empty_blob() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();

    mgr.store_blob("v", "empty", b"").unwrap();
    let data = mgr.read_blob("v", "empty").unwrap();
    assert!(data.is_empty());
}

// ── Change password ──────────────────────────────────────────────

#[test]
fn change_password_reencrypts() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("test").unwrap();
    mgr.initialize("test", "oldpass12").unwrap();
    mgr.store_blob("test", "b1", b"secret data").unwrap();

    mgr.change_password("test", "oldpass12", "newpass12").unwrap();

    mgr.lock("test");
    assert!(mgr.unlock("test", "oldpass12").is_err());

    mgr.unlock("test", "newpass12").unwrap();
    assert_eq!(mgr.read_blob("test", "b1").unwrap(), b"secret data");
}

#[test]
fn change_password_too_short_fails() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    assert!(mgr.change_password("v", "password123", "short").is_err());
}

#[test]
fn change_password_wrong_old_fails() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    assert!(mgr.change_password("v", "wrongold!", "newpass12").is_err());
}

// ── Multi-vault ──────────────────────────────────────────────────

#[test]
fn multiple_vaults_isolated() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v1").unwrap();
    mgr.create_vault("v2").unwrap();
    mgr.initialize("v1", "password123").unwrap();
    mgr.initialize("v2", "password123").unwrap();

    mgr.store_blob("v1", "b1", b"from_v1").unwrap();
    mgr.store_blob("v2", "b1", b"from_v2").unwrap();

    assert_eq!(mgr.read_blob("v1", "b1").unwrap(), b"from_v1");
    assert_eq!(mgr.read_blob("v2", "b1").unwrap(), b"from_v2");
}

#[test]
fn unlock_all_and_lock_all() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v1").unwrap();
    mgr.create_vault("v2").unwrap();
    mgr.initialize("v1", "password123").unwrap();
    mgr.initialize("v2", "password123").unwrap();

    mgr.lock_all();
    assert!(!mgr.is_unlocked("v1"));
    assert!(!mgr.is_unlocked("v2"));

    mgr.unlock_all("password123").unwrap();
    assert!(mgr.is_unlocked("v1"));
    assert!(mgr.is_unlocked("v2"));
}

#[test]
fn change_password_all() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.create_vault("v1").unwrap();
    mgr.create_vault("v2").unwrap();
    mgr.initialize("v1", "password123").unwrap();
    mgr.initialize("v2", "password123").unwrap();
    mgr.store_blob("v1", "b", b"d1").unwrap();
    mgr.store_blob("v2", "b", b"d2").unwrap();

    mgr.change_password_all("password123", "newpass12").unwrap();

    mgr.lock_all();
    mgr.unlock_all("newpass12").unwrap();
    assert_eq!(mgr.read_blob("v1", "b").unwrap(), b"d1");
    assert_eq!(mgr.read_blob("v2", "b").unwrap(), b"d2");
}

// ── is_initialized / is_unlocked for unknown vault ───────────────

#[test]
fn is_initialized_unknown_vault() {
    let mgr = VaultManager::open_in_memory().unwrap();
    // auto-creates on check
    assert!(!mgr.is_initialized("unknown"));
}

#[test]
fn is_unlocked_unknown_vault() {
    let mgr = VaultManager::open_in_memory().unwrap();
    assert!(!mgr.is_unlocked("unknown"));
}

#[test]
fn lock_unknown_vault_is_noop() {
    let mgr = VaultManager::open_in_memory().unwrap();
    mgr.lock("unknown"); // should not panic
}
