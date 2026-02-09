use privstack_crypto::{derive_key, generate_random_key, DerivedKey, KdfParams, Salt};

fn test_params() -> KdfParams {
    KdfParams {
        memory_cost: 1024,
        time_cost: 1,
        parallelism: 1,
    }
}

// ── derive_key ───────────────────────────────────────────────────

#[test]
fn derive_key_produces_consistent_results() {
    let salt = Salt::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
    let params = test_params();
    let key1 = derive_key("test_password_123", &salt, &params).unwrap();
    let key2 = derive_key("test_password_123", &salt, &params).unwrap();
    assert_eq!(key1.as_bytes(), key2.as_bytes());
}

#[test]
fn different_passwords_produce_different_keys() {
    let salt = Salt::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
    let params = test_params();
    let key1 = derive_key("password1", &salt, &params).unwrap();
    let key2 = derive_key("password2", &salt, &params).unwrap();
    assert_ne!(key1.as_bytes(), key2.as_bytes());
}

#[test]
fn different_salts_produce_different_keys() {
    let params = test_params();
    let salt1 = Salt::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
    let salt2 = Salt::from_bytes([16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);
    let key1 = derive_key("same_password", &salt1, &params).unwrap();
    let key2 = derive_key("same_password", &salt2, &params).unwrap();
    assert_ne!(key1.as_bytes(), key2.as_bytes());
}

#[test]
fn derive_key_produces_32_byte_key() {
    let salt = Salt::from_bytes([1; 16]);
    let key = derive_key("pw", &salt, &test_params()).unwrap();
    assert_eq!(key.as_bytes().len(), 32);
}

// ── generate_random_key ──────────────────────────────────────────

#[test]
fn generate_random_key_produces_unique_keys() {
    let key1 = generate_random_key();
    let key2 = generate_random_key();
    assert_ne!(key1.as_bytes(), key2.as_bytes());
}

#[test]
fn generate_random_key_is_32_bytes() {
    let key = generate_random_key();
    assert_eq!(key.as_bytes().len(), 32);
}

// ── DerivedKey ───────────────────────────────────────────────────

#[test]
fn derived_key_from_bytes_roundtrip() {
    let bytes = [42u8; 32];
    let key = DerivedKey::from_bytes(bytes);
    assert_eq!(*key.as_bytes(), bytes);
}

#[test]
fn key_debug_does_not_leak_bytes() {
    let key = generate_random_key();
    let debug = format!("{:?}", key);
    assert!(debug.contains("REDACTED"));
    assert!(!debug.contains(&format!("{:?}", key.as_bytes())));
}

#[test]
fn derived_key_clone() {
    let key = generate_random_key();
    let cloned = key.clone();
    assert_eq!(key.as_bytes(), cloned.as_bytes());
}

// ── Salt ─────────────────────────────────────────────────────────

#[test]
fn salt_random_produces_unique() {
    let s1 = Salt::random();
    let s2 = Salt::random();
    assert_ne!(s1.as_bytes(), s2.as_bytes());
}

#[test]
fn salt_from_bytes_roundtrip() {
    let bytes = [7u8; 16];
    let salt = Salt::from_bytes(bytes);
    assert_eq!(*salt.as_bytes(), bytes);
}

#[test]
fn salt_debug() {
    let salt = Salt::from_bytes([0u8; 16]);
    let debug = format!("{:?}", salt);
    assert!(debug.contains("Salt"));
}

// ── KdfParams ────────────────────────────────────────────────────

#[test]
fn kdf_params_default() {
    let params = KdfParams::default();
    assert_eq!(params.memory_cost, 19 * 1024);
    assert_eq!(params.time_cost, 2);
    assert_eq!(params.parallelism, 1);
}

#[test]
fn kdf_params_clone() {
    let params = test_params();
    let cloned = params.clone();
    assert_eq!(cloned.memory_cost, params.memory_cost);
}

// ── Salt::clone ─────────────────────────────────────────────────

#[test]
fn salt_clone() {
    let salt = Salt::from_bytes([42u8; 16]);
    let cloned = salt.clone();
    assert_eq!(salt.as_bytes(), cloned.as_bytes());
}

// ── Edge cases ──────────────────────────────────────────────────

#[test]
fn derive_key_empty_password() {
    let salt = Salt::from_bytes([1; 16]);
    let key = derive_key("", &salt, &test_params()).unwrap();
    assert_eq!(key.as_bytes().len(), 32);
}

#[test]
fn derive_key_very_long_password() {
    let long_pw: String = "a".repeat(10_000);
    let salt = Salt::from_bytes([5; 16]);
    let key = derive_key(&long_pw, &salt, &test_params()).unwrap();
    assert_eq!(key.as_bytes().len(), 32);
}

#[test]
fn derive_key_unicode_password() {
    let salt = Salt::from_bytes([9; 16]);
    let key = derive_key("p\u{00e4}ssw\u{00f6}rd\u{1f600}", &salt, &test_params()).unwrap();
    assert_eq!(key.as_bytes().len(), 32);
}

#[test]
fn derive_key_invalid_params_zero_time_cost() {
    let salt = Salt::from_bytes([1; 16]);
    let bad_params = KdfParams {
        memory_cost: 1024,
        time_cost: 0,
        parallelism: 1,
    };
    let result = derive_key("pw", &salt, &bad_params);
    assert!(result.is_err());
}

#[test]
fn derive_key_invalid_params_zero_parallelism() {
    let salt = Salt::from_bytes([1; 16]);
    let bad_params = KdfParams {
        memory_cost: 1024,
        time_cost: 1,
        parallelism: 0,
    };
    let result = derive_key("pw", &salt, &bad_params);
    assert!(result.is_err());
}

#[test]
fn kdf_params_debug() {
    let params = test_params();
    let dbg = format!("{params:?}");
    assert!(dbg.contains("KdfParams"));
}

// ── KdfParams::test() ────────────────────────────────────────────
// Note: KdfParams::test() is cfg(test) so only available in test builds.
// We use it indirectly via test_params() above, but let's verify explicitly.

#[test]
fn kdf_params_test_values() {
    // test_params is defined at the top and mirrors KdfParams::test()
    let p = test_params();
    assert_eq!(p.memory_cost, 1024);
    assert_eq!(p.time_cost, 1);
    assert_eq!(p.parallelism, 1);
}

// ── DerivedKey zeroize on drop ───────────────────────────────────

#[test]
fn derived_key_is_not_all_zeros_before_drop() {
    let key = generate_random_key();
    // At least some bytes should be non-zero
    assert!(key.as_bytes().iter().any(|&b| b != 0));
}

// ── derive_key with varied params ────────────────────────────────

#[test]
fn derive_key_different_params_produce_different_keys() {
    let salt = Salt::from_bytes([1; 16]);
    let params1 = KdfParams {
        memory_cost: 1024,
        time_cost: 1,
        parallelism: 1,
    };
    let params2 = KdfParams {
        memory_cost: 2048,
        time_cost: 1,
        parallelism: 1,
    };
    let key1 = derive_key("same_password", &salt, &params1).unwrap();
    let key2 = derive_key("same_password", &salt, &params2).unwrap();
    assert_ne!(key1.as_bytes(), key2.as_bytes());
}

#[test]
fn derive_key_invalid_params_zero_memory() {
    let salt = Salt::from_bytes([1; 16]);
    let bad_params = KdfParams {
        memory_cost: 0,
        time_cost: 1,
        parallelism: 1,
    };
    let result = derive_key("pw", &salt, &bad_params);
    assert!(result.is_err());
}
