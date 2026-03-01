use privstack_vault::{Vault, VaultError, VaultManager};
use std::sync::{Arc, Mutex};

// ── Helpers ──────────────────────────────────────────────────────

fn make_manager() -> VaultManager {
    VaultManager::open_in_memory().unwrap()
}

fn init_and_unlock(mgr: &VaultManager, vault_id: &str) {
    mgr.create_vault(vault_id).unwrap();
    mgr.initialize(vault_id, "password123").unwrap();
}

/// Generate a valid BIP39 mnemonic by setting up recovery on a throwaway vault.
fn generate_mnemonic() -> String {
    let mgr = make_manager();
    init_and_unlock(&mgr, "tmp");
    mgr.setup_recovery("tmp").unwrap()
}

// ============================================================================
// setup_recovery_with_mnemonic — Vault direct API
// ============================================================================

#[test]
fn setup_recovery_with_mnemonic_stores_blob() {
    let conn = privstack_db::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("v", conn).unwrap();
    vault.initialize("password123").unwrap();

    let mnemonic = generate_mnemonic();
    vault.setup_recovery_with_mnemonic(&mnemonic).unwrap();

    assert!(vault.has_recovery().unwrap());
}

#[test]
fn setup_recovery_with_mnemonic_on_locked_vault_fails() {
    let conn = privstack_db::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("v", conn).unwrap();
    vault.initialize("password123").unwrap();
    vault.lock();

    let mnemonic = generate_mnemonic();
    let result = vault.setup_recovery_with_mnemonic(&mnemonic);
    assert!(matches!(result, Err(VaultError::Locked)));
}

#[test]
fn setup_recovery_with_mnemonic_allows_password_reset() {
    let conn = privstack_db::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("v", conn).unwrap();
    vault.initialize("password123").unwrap();
    vault.store_blob("secret", b"classified data").unwrap();

    let mnemonic = generate_mnemonic();
    vault.setup_recovery_with_mnemonic(&mnemonic).unwrap();

    let (old_kb, new_kb) = vault
        .reset_password_with_recovery(&mnemonic, "newpass12")
        .unwrap();
    assert_eq!(old_kb.len(), 32);
    assert_eq!(new_kb.len(), 32);
    assert_ne!(old_kb, new_kb);

    vault.lock();
    vault.unlock("newpass12").unwrap();
    assert_eq!(vault.read_blob("secret").unwrap(), b"classified data");
}

#[test]
fn setup_recovery_with_mnemonic_overwrites_previous_recovery() {
    let conn = privstack_db::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("v", conn).unwrap();
    vault.initialize("password123").unwrap();

    let mnemonic_a = generate_mnemonic();
    let mnemonic_b = generate_mnemonic();

    vault.setup_recovery_with_mnemonic(&mnemonic_a).unwrap();
    vault.setup_recovery_with_mnemonic(&mnemonic_b).unwrap();

    // Old mnemonic should no longer work
    let result = vault.reset_password_with_recovery(&mnemonic_a, "newpass12");
    assert!(result.is_err());

    // New mnemonic should work
    vault
        .reset_password_with_recovery(&mnemonic_b, "newpass12")
        .unwrap();
}

// ============================================================================
// VaultManager wrappers for setup_recovery_with_mnemonic
// ============================================================================

#[test]
fn manager_setup_recovery_with_mnemonic_roundtrip() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "v");
    mgr.store_blob("v", "b1", b"hello").unwrap();

    let mnemonic = generate_mnemonic();
    mgr.setup_recovery_with_mnemonic("v", &mnemonic).unwrap();

    assert!(mgr.has_recovery("v").unwrap());

    let (old_kb, new_kb) = mgr
        .reset_password_with_recovery("v", &mnemonic, "newpass12")
        .unwrap();
    assert_eq!(old_kb.len(), 32);
    assert_eq!(new_kb.len(), 32);

    mgr.lock("v");
    mgr.unlock("v", "newpass12").unwrap();
    assert_eq!(mgr.read_blob("v", "b1").unwrap(), b"hello");
}

#[test]
fn manager_setup_recovery_with_mnemonic_locked_fails() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "v");
    mgr.lock("v");

    let mnemonic = generate_mnemonic();
    let result = mgr.setup_recovery_with_mnemonic("v", &mnemonic);
    assert!(matches!(result, Err(VaultError::Locked)));
}

#[test]
fn manager_has_recovery_false_before_setup() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "v");
    assert!(!mgr.has_recovery("v").unwrap());
}

#[test]
fn manager_has_recovery_true_after_with_mnemonic() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "v");
    let mnemonic = generate_mnemonic();
    mgr.setup_recovery_with_mnemonic("v", &mnemonic).unwrap();
    assert!(mgr.has_recovery("v").unwrap());
}

// ============================================================================
// reset_password_with_recovery — re-encryption loop with blobs
// ============================================================================

#[test]
fn reset_password_reencrypts_many_blobs() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "v");

    for i in 0..10 {
        let id = format!("blob-{i}");
        let data = format!("payload-{i}");
        mgr.store_blob("v", &id, data.as_bytes()).unwrap();
    }

    let mnemonic = mgr.setup_recovery("v").unwrap();
    mgr.reset_password_with_recovery("v", &mnemonic, "newpass12")
        .unwrap();

    mgr.lock("v");
    mgr.unlock("v", "newpass12").unwrap();

    for i in 0..10 {
        let id = format!("blob-{i}");
        let expected = format!("payload-{i}");
        assert_eq!(mgr.read_blob("v", &id).unwrap(), expected.as_bytes());
    }
}

#[test]
fn reset_password_with_mnemonic_reencrypts_blobs() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "v");

    mgr.store_blob("v", "a", b"alpha").unwrap();
    mgr.store_blob("v", "b", b"beta").unwrap();
    mgr.store_blob("v", "c", b"gamma").unwrap();

    let mnemonic = generate_mnemonic();
    mgr.setup_recovery_with_mnemonic("v", &mnemonic).unwrap();

    mgr.reset_password_with_recovery("v", &mnemonic, "newpass12")
        .unwrap();

    mgr.lock("v");
    mgr.unlock("v", "newpass12").unwrap();

    assert_eq!(mgr.read_blob("v", "a").unwrap(), b"alpha");
    assert_eq!(mgr.read_blob("v", "b").unwrap(), b"beta");
    assert_eq!(mgr.read_blob("v", "c").unwrap(), b"gamma");
}

#[test]
fn reset_password_with_large_blob() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "v");

    let large: Vec<u8> = (0..50_000).map(|i| (i % 256) as u8).collect();
    mgr.store_blob("v", "big", &large).unwrap();

    let mnemonic = mgr.setup_recovery("v").unwrap();
    mgr.reset_password_with_recovery("v", &mnemonic, "newpass12")
        .unwrap();

    mgr.lock("v");
    mgr.unlock("v", "newpass12").unwrap();
    assert_eq!(mgr.read_blob("v", "big").unwrap(), large);
}

#[test]
fn reset_password_mnemonic_stays_valid_with_caller_mnemonic() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "v");

    let mnemonic = generate_mnemonic();
    mgr.setup_recovery_with_mnemonic("v", &mnemonic).unwrap();

    // First reset
    mgr.reset_password_with_recovery("v", &mnemonic, "secondpw")
        .unwrap();
    mgr.lock("v");
    mgr.unlock("v", "secondpw").unwrap();

    // Second reset with the same mnemonic
    mgr.reset_password_with_recovery("v", &mnemonic, "thirdpwd")
        .unwrap();
    mgr.lock("v");
    mgr.unlock("v", "thirdpwd").unwrap();
    assert!(mgr.is_unlocked("v"));
}

#[test]
fn reset_password_returns_different_key_pairs() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "v");

    let mnemonic = mgr.setup_recovery("v").unwrap();

    let (_old_kb1, new_kb1) = mgr
        .reset_password_with_recovery("v", &mnemonic, "pass_two")
        .unwrap();

    let (old_kb2, new_kb2) = mgr
        .reset_password_with_recovery("v", &mnemonic, "passthree")
        .unwrap();

    // old_kb2 should equal new_kb1 (the key from the first reset became the "old" key)
    assert_eq!(old_kb2, new_kb1);
    // new keys from different passwords should differ
    assert_ne!(new_kb1, new_kb2);
}

// ============================================================================
// sanitize_for_sql edge cases (tested indirectly via vault IDs)
// ============================================================================

#[test]
fn vault_id_with_dots() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "my.vault.id");
    mgr.store_blob("my.vault.id", "b1", b"dot data").unwrap();
    assert_eq!(mgr.read_blob("my.vault.id", "b1").unwrap(), b"dot data");
}

#[test]
fn vault_id_with_dashes() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "my-vault-id");
    mgr.store_blob("my-vault-id", "b1", b"dash data").unwrap();
    assert_eq!(
        mgr.read_blob("my-vault-id", "b1").unwrap(),
        b"dash data"
    );
}

#[test]
fn vault_id_with_spaces() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "my vault id");
    mgr.store_blob("my vault id", "b1", b"space data").unwrap();
    assert_eq!(
        mgr.read_blob("my vault id", "b1").unwrap(),
        b"space data"
    );
}

#[test]
fn vault_id_with_mixed_special_chars() {
    let mgr = make_manager();
    let vault_id = "user@host:path/to.vault-1";
    init_and_unlock(&mgr, vault_id);
    mgr.store_blob(vault_id, "b1", b"mixed data").unwrap();
    assert_eq!(mgr.read_blob(vault_id, "b1").unwrap(), b"mixed data");
}

#[test]
fn vault_id_with_underscores_passthrough() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "already_safe_id");
    mgr.store_blob("already_safe_id", "b1", b"safe").unwrap();
    assert_eq!(mgr.read_blob("already_safe_id", "b1").unwrap(), b"safe");
}

#[test]
fn two_vault_ids_that_sanitize_to_same_prefix_share_tables() {
    // "a.b" and "a_b" both sanitize to "a_b", so they share the same tables.
    // This tests that sanitize_for_sql is deterministic and the vault data
    // is reachable from either original ID within the same Vault instance.
    let mgr = make_manager();
    init_and_unlock(&mgr, "a.b");
    mgr.store_blob("a.b", "x", b"dot-version").unwrap();

    // Creating a vault with "a_b" will share the same tables
    mgr.create_vault("a_b").unwrap();
    // It should see the same tables (already initialized by "a.b")
    assert!(mgr.is_initialized("a_b"));
}

// ============================================================================
// checkpoint method
// ============================================================================

#[test]
fn checkpoint_succeeds_on_in_memory_db() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "v");
    mgr.store_blob("v", "b1", b"data").unwrap();
    mgr.checkpoint().unwrap();
}

#[test]
fn checkpoint_succeeds_on_file_backed_db() {
    let temp = tempfile::TempDir::new().unwrap();
    let db_path = temp.path().join("checkpoint_test.db");
    let mgr = VaultManager::open(&db_path).unwrap();
    mgr.create_vault("v").unwrap();
    mgr.initialize("v", "password123").unwrap();
    mgr.store_blob("v", "b1", b"persisted").unwrap();

    mgr.checkpoint().unwrap();

    // Data should still be readable after checkpoint
    assert_eq!(mgr.read_blob("v", "b1").unwrap(), b"persisted");
}

#[test]
fn checkpoint_on_empty_db() {
    let mgr = make_manager();
    // No vaults created, checkpoint should still succeed
    mgr.checkpoint().unwrap();
}

// ============================================================================
// hex::encode coverage (tested indirectly via store_blob + list_blobs)
// ============================================================================

#[test]
fn blob_content_hash_is_hex_encoded_sha256() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "v");

    let data = b"hello world";
    mgr.store_blob("v", "b1", data).unwrap();

    let blobs = mgr.list_blobs("v").unwrap();
    assert_eq!(blobs.len(), 1);

    let hash = blobs[0].content_hash.as_ref().unwrap();
    // SHA-256 produces 32 bytes = 64 hex chars
    assert_eq!(hash.len(), 64);
    // All chars should be valid hex
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn blob_content_hash_is_deterministic() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "v");

    mgr.store_blob("v", "b1", b"same content").unwrap();
    let blobs1 = mgr.list_blobs("v").unwrap();
    let hash1 = blobs1[0].content_hash.as_ref().unwrap().clone();

    // Overwrite with same content
    mgr.store_blob("v", "b1", b"same content").unwrap();
    let blobs2 = mgr.list_blobs("v").unwrap();
    let hash2 = blobs2[0].content_hash.as_ref().unwrap().clone();

    assert_eq!(hash1, hash2);
}

#[test]
fn blob_content_hash_differs_for_different_data() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "v");

    mgr.store_blob("v", "b1", b"data one").unwrap();
    mgr.store_blob("v", "b2", b"data two").unwrap();

    let blobs = mgr.list_blobs("v").unwrap();
    let hashes: Vec<_> = blobs
        .iter()
        .map(|b| b.content_hash.as_ref().unwrap().clone())
        .collect();
    assert_ne!(hashes[0], hashes[1]);
}

#[test]
fn empty_blob_has_valid_hex_hash() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "v");

    mgr.store_blob("v", "empty", b"").unwrap();
    let blobs = mgr.list_blobs("v").unwrap();
    let hash = blobs[0].content_hash.as_ref().unwrap();
    assert_eq!(hash.len(), 64);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
}

// ============================================================================
// Recovery edge cases
// ============================================================================

#[test]
fn reset_password_with_recovery_new_password_too_short() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "v");
    let mnemonic = generate_mnemonic();
    mgr.setup_recovery_with_mnemonic("v", &mnemonic).unwrap();

    let result = mgr.reset_password_with_recovery("v", &mnemonic, "short");
    assert!(matches!(result, Err(VaultError::PasswordTooShort)));
}

#[test]
fn reset_password_without_recovery_configured_fails() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "v");

    let mnemonic = generate_mnemonic();
    let result = mgr.reset_password_with_recovery("v", &mnemonic, "newpass12");
    assert!(matches!(result, Err(VaultError::RecoveryNotConfigured)));
}

#[test]
fn reset_password_wrong_mnemonic_fails() {
    let mgr = make_manager();
    init_and_unlock(&mgr, "v");
    let mnemonic = generate_mnemonic();
    mgr.setup_recovery_with_mnemonic("v", &mnemonic).unwrap();

    // Use a different valid mnemonic
    let wrong_mnemonic = generate_mnemonic();
    let result = mgr.reset_password_with_recovery("v", &wrong_mnemonic, "newpass12");
    assert!(matches!(result, Err(VaultError::InvalidRecoveryMnemonic)));
}

#[test]
fn recovery_with_mnemonic_on_vault_with_special_id() {
    let mgr = make_manager();
    let vault_id = "user.vault-test";
    init_and_unlock(&mgr, vault_id);
    mgr.store_blob(vault_id, "s1", b"special").unwrap();

    let mnemonic = generate_mnemonic();
    mgr.setup_recovery_with_mnemonic(vault_id, &mnemonic)
        .unwrap();
    assert!(mgr.has_recovery(vault_id).unwrap());

    mgr.reset_password_with_recovery(vault_id, &mnemonic, "newpass12")
        .unwrap();
    mgr.lock(vault_id);
    mgr.unlock(vault_id, "newpass12").unwrap();
    assert_eq!(mgr.read_blob(vault_id, "s1").unwrap(), b"special");
}

#[test]
fn vault_direct_reset_password_no_blobs() {
    let conn = privstack_db::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("v", conn).unwrap();
    vault.initialize("password123").unwrap();

    let mnemonic = generate_mnemonic();
    vault.setup_recovery_with_mnemonic(&mnemonic).unwrap();

    // Reset with empty blob table
    let (old_kb, new_kb) = vault
        .reset_password_with_recovery(&mnemonic, "newpass12")
        .unwrap();
    assert_eq!(old_kb.len(), 32);
    assert_eq!(new_kb.len(), 32);

    vault.lock();
    vault.unlock("newpass12").unwrap();
    assert!(vault.is_unlocked());
}

#[test]
fn vault_direct_has_recovery_false_initially() {
    let conn = privstack_db::open_in_memory().unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let vault = Vault::open("v", conn).unwrap();
    vault.initialize("password123").unwrap();

    assert!(!vault.has_recovery().unwrap());
}
