//! Property-based tests for the crypto module.
//!
//! These tests verify security properties that must always hold:
//! - Encryption is reversible with the correct key
//! - Wrong keys fail decryption
//! - Tampering is detected
//! - Keys are derived deterministically from passwords

use privstack_crypto::{
    decrypt, decrypt_string, derive_key, encrypt, encrypt_string, generate_random_key,
    EncryptedData, KdfParams, Salt, KEY_SIZE, NONCE_SIZE,
};
use proptest::prelude::*;

// =============================================================================
// HELPER STRATEGIES
// =============================================================================

fn salt_strategy() -> impl Strategy<Value = Salt> {
    prop::array::uniform16(any::<u8>()).prop_map(Salt::from_bytes)
}

fn plaintext_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..10000)
}

fn string_plaintext_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[\\x00-\\x7F]{0,1000}").unwrap()
}

/// Fast KDF params for testing (low memory/iterations for speed)
fn fast_kdf_params() -> KdfParams {
    KdfParams {
        memory_cost: 1024, // 1 MiB
        time_cost: 1,
        parallelism: 1,
    }
}

fn password_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9!@#$%^&*()]{1,100}").unwrap()
}

// =============================================================================
// ENCRYPTION PROPERTIES
// =============================================================================

mod encryption_properties {
    use super::*;

    proptest! {
        /// Encryption followed by decryption with the same key returns original plaintext
        #[test]
        fn roundtrip_preserves_data(plaintext in plaintext_strategy()) {
            let key = generate_random_key();

            let encrypted = encrypt(&key, &plaintext).unwrap();
            let decrypted = decrypt(&key, &encrypted).unwrap();

            prop_assert_eq!(decrypted, plaintext);
        }

        /// String encryption roundtrip preserves the string
        #[test]
        fn string_roundtrip_preserves_data(plaintext in string_plaintext_strategy()) {
            let key = generate_random_key();

            let encrypted = encrypt_string(&key, &plaintext).unwrap();
            let decrypted = decrypt_string(&key, &encrypted).unwrap();

            prop_assert_eq!(decrypted, plaintext);
        }

        /// Different keys produce different ciphertexts for the same plaintext
        #[test]
        fn different_keys_different_ciphertexts(plaintext in plaintext_strategy()) {
            prop_assume!(!plaintext.is_empty());

            let key1 = generate_random_key();
            let key2 = generate_random_key();

            let encrypted1 = encrypt(&key1, &plaintext).unwrap();
            let encrypted2 = encrypt(&key2, &plaintext).unwrap();

            // Ciphertexts should be different (different keys + different nonces)
            prop_assert_ne!(encrypted1.ciphertext, encrypted2.ciphertext);
        }

        /// Same key encrypting same plaintext produces different ciphertexts (random nonce)
        #[test]
        fn same_key_different_nonces(plaintext in plaintext_strategy()) {
            let key = generate_random_key();

            let encrypted1 = encrypt(&key, &plaintext).unwrap();
            let encrypted2 = encrypt(&key, &plaintext).unwrap();

            // Nonces should be different
            prop_assert_ne!(encrypted1.nonce, encrypted2.nonce);

            // Both should decrypt correctly
            let decrypted1 = decrypt(&key, &encrypted1).unwrap();
            let decrypted2 = decrypt(&key, &encrypted2).unwrap();

            prop_assert_eq!(decrypted1, plaintext.clone());
            prop_assert_eq!(decrypted2, plaintext);
        }

        /// Wrong key fails to decrypt
        #[test]
        fn wrong_key_fails_decryption(plaintext in plaintext_strategy()) {
            prop_assume!(!plaintext.is_empty());

            let correct_key = generate_random_key();
            let wrong_key = generate_random_key();

            let encrypted = encrypt(&correct_key, &plaintext).unwrap();
            let result = decrypt(&wrong_key, &encrypted);

            prop_assert!(result.is_err());
        }

        /// Tampered ciphertext fails authentication
        #[test]
        fn tampered_ciphertext_fails(
            plaintext in plaintext_strategy(),
            tamper_pos in any::<usize>(),
            tamper_byte in any::<u8>(),
        ) {
            prop_assume!(!plaintext.is_empty());

            let key = generate_random_key();
            let mut encrypted = encrypt(&key, &plaintext).unwrap();

            // Only tamper if there's ciphertext to tamper
            if !encrypted.ciphertext.is_empty() {
                let pos = tamper_pos % encrypted.ciphertext.len();
                // Only test if we're actually changing the byte
                if encrypted.ciphertext[pos] != tamper_byte {
                    encrypted.ciphertext[pos] = tamper_byte;
                    let result = decrypt(&key, &encrypted);
                    prop_assert!(result.is_err());
                }
            }
        }

        /// Tampered nonce fails authentication
        #[test]
        fn tampered_nonce_fails(
            plaintext in plaintext_strategy(),
            tamper_pos in 0usize..NONCE_SIZE,
            tamper_byte in any::<u8>(),
        ) {
            prop_assume!(!plaintext.is_empty());

            let key = generate_random_key();
            let mut encrypted = encrypt(&key, &plaintext).unwrap();

            // Only test if we're actually changing the byte
            if encrypted.nonce[tamper_pos] != tamper_byte {
                encrypted.nonce[tamper_pos] = tamper_byte;
                let result = decrypt(&key, &encrypted);
                prop_assert!(result.is_err());
            }
        }

        /// Ciphertext is longer than plaintext (due to auth tag)
        #[test]
        fn ciphertext_includes_auth_tag(plaintext in plaintext_strategy()) {
            let key = generate_random_key();
            let encrypted = encrypt(&key, &plaintext).unwrap();

            // Ciphertext should be plaintext length + 16 bytes auth tag
            prop_assert_eq!(encrypted.ciphertext.len(), plaintext.len() + 16);
        }
    }
}

// =============================================================================
// KEY DERIVATION PROPERTIES
// =============================================================================

mod key_derivation_properties {
    use super::*;

    proptest! {
        /// Same password + salt produces same key (deterministic)
        #[test]
        fn derivation_is_deterministic(
            password in password_strategy(),
            salt in salt_strategy(),
        ) {
            let params = fast_kdf_params();

            let key1 = derive_key(&password, &salt, &params).unwrap();
            let key2 = derive_key(&password, &salt, &params).unwrap();

            prop_assert_eq!(key1.as_bytes(), key2.as_bytes());
        }

        /// Different passwords produce different keys
        #[test]
        fn different_passwords_different_keys(
            password1 in password_strategy(),
            password2 in password_strategy(),
            salt in salt_strategy(),
        ) {
            prop_assume!(password1 != password2);

            let params = fast_kdf_params();

            let key1 = derive_key(&password1, &salt, &params).unwrap();
            let key2 = derive_key(&password2, &salt, &params).unwrap();

            prop_assert_ne!(key1.as_bytes(), key2.as_bytes());
        }

        /// Different salts produce different keys
        #[test]
        fn different_salts_different_keys(
            password in password_strategy(),
            salt1 in salt_strategy(),
            salt2 in salt_strategy(),
        ) {
            prop_assume!(salt1.as_bytes() != salt2.as_bytes());

            let params = fast_kdf_params();

            let key1 = derive_key(&password, &salt1, &params).unwrap();
            let key2 = derive_key(&password, &salt2, &params).unwrap();

            prop_assert_ne!(key1.as_bytes(), key2.as_bytes());
        }

        /// Derived key has correct length
        #[test]
        fn derived_key_has_correct_length(
            password in password_strategy(),
            salt in salt_strategy(),
        ) {
            let params = fast_kdf_params();
            let key = derive_key(&password, &salt, &params).unwrap();

            prop_assert_eq!(key.as_bytes().len(), KEY_SIZE);
        }

        /// Random keys have correct length
        #[test]
        fn random_key_has_correct_length(_dummy in any::<u8>()) {
            let key = generate_random_key();
            prop_assert_eq!(key.as_bytes().len(), KEY_SIZE);
        }

        /// Random keys are unique
        #[test]
        fn random_keys_are_unique(_dummy in any::<u8>()) {
            let key1 = generate_random_key();
            let key2 = generate_random_key();

            prop_assert_ne!(key1.as_bytes(), key2.as_bytes());
        }
    }
}

// =============================================================================
// BASE64 ENCODING PROPERTIES
// =============================================================================

mod base64_properties {
    use super::*;

    proptest! {
        /// Base64 encoding is reversible
        #[test]
        fn base64_roundtrip(plaintext in plaintext_strategy()) {
            let key = generate_random_key();
            let encrypted = encrypt(&key, &plaintext).unwrap();

            let encoded = encrypted.to_base64();
            let decoded = EncryptedData::from_base64(&encoded).unwrap();

            prop_assert_eq!(encrypted.nonce, decoded.nonce);
            prop_assert_eq!(encrypted.ciphertext, decoded.ciphertext);
        }

        /// Base64 encoded data can be decrypted
        #[test]
        fn base64_then_decrypt(plaintext in plaintext_strategy()) {
            let key = generate_random_key();
            let encrypted = encrypt(&key, &plaintext).unwrap();

            let encoded = encrypted.to_base64();
            let decoded = EncryptedData::from_base64(&encoded).unwrap();
            let decrypted = decrypt(&key, &decoded).unwrap();

            prop_assert_eq!(decrypted, plaintext);
        }
    }
}

// =============================================================================
// INTEGRATION TESTS
// =============================================================================

mod integration {
    use super::*;
    use privstack_crypto::{encrypt_document, decrypt_document, reencrypt_document_key};

    proptest! {
        /// Entity encryption roundtrip preserves all data
        #[test]
        fn entity_encryption_roundtrip(
            data in plaintext_strategy(),
            password in password_strategy(),
            salt in salt_strategy(),
        ) {
            let params = fast_kdf_params();
            let master_key = derive_key(&password, &salt, &params).unwrap();

            let encrypted = encrypt_document("test-entity", &data, &master_key).unwrap();
            let decrypted = decrypt_document(&encrypted, &master_key).unwrap();

            prop_assert_eq!(decrypted, data);
        }

        /// Password change re-encryption works correctly
        #[test]
        fn password_change_workflow(
            data in plaintext_strategy(),
            old_password in password_strategy(),
            new_password in password_strategy(),
            old_salt in salt_strategy(),
            new_salt in salt_strategy(),
        ) {
            let params = fast_kdf_params();
            let old_key = derive_key(&old_password, &old_salt, &params).unwrap();
            let new_key = derive_key(&new_password, &new_salt, &params).unwrap();

            let encrypted = encrypt_document("test-entity", &data, &old_key).unwrap();
            let reencrypted = reencrypt_document_key(&encrypted, &old_key, &new_key).unwrap();

            prop_assert!(decrypt_document(&reencrypted, &old_key).is_err());

            let decrypted = decrypt_document(&reencrypted, &new_key).unwrap();
            prop_assert_eq!(decrypted, data);
        }
    }
}
