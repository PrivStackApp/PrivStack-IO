//! Abstract encryption interface for routing data through the vault layer.
//!
//! Consumers (EntityStore, BlobStore) depend on `Arc<dyn DataEncryptor>` —
//! they never see raw keys. VaultManager implements this trait; tests use
//! `PassthroughEncryptor` for zero-overhead operation without a password.

use thiserror::Error;

/// Errors from the encryption layer.
#[derive(Debug, Error)]
pub enum EncryptorError {
    /// The vault is locked or no key is available.
    #[error("encryptor unavailable (vault locked)")]
    Unavailable,
    /// Underlying crypto failure.
    #[error("crypto error: {0}")]
    Crypto(String),
    /// Serialization round-trip failure.
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub type EncryptorResult<T> = Result<T, EncryptorError>;

/// Trait for encrypting/decrypting opaque byte slices.
///
/// Implementations own the key material. Callers never see raw keys.
pub trait DataEncryptor: Send + Sync {
    /// Encrypt `data` for the given entity, returning an opaque ciphertext blob.
    fn encrypt_bytes(&self, entity_id: &str, data: &[u8]) -> EncryptorResult<Vec<u8>>;

    /// Decrypt a blob previously produced by `encrypt_bytes`.
    fn decrypt_bytes(&self, data: &[u8]) -> EncryptorResult<Vec<u8>>;

    /// Re-wrap the per-entity key from `old_key_bytes` to `new_key_bytes`
    /// without touching content. Used during password changes.
    fn reencrypt_bytes(
        &self,
        data: &[u8],
        old_key_bytes: &[u8],
        new_key_bytes: &[u8],
    ) -> EncryptorResult<Vec<u8>>;

    /// Whether the encryptor is ready (vault unlocked).
    fn is_available(&self) -> bool;
}

/// No-op encryptor for tests and pre-unlock operation.
/// Data passes through unchanged.
pub struct PassthroughEncryptor;

impl DataEncryptor for PassthroughEncryptor {
    fn encrypt_bytes(&self, _entity_id: &str, data: &[u8]) -> EncryptorResult<Vec<u8>> {
        Ok(data.to_vec())
    }

    fn decrypt_bytes(&self, data: &[u8]) -> EncryptorResult<Vec<u8>> {
        Ok(data.to_vec())
    }

    fn reencrypt_bytes(
        &self,
        data: &[u8],
        _old_key_bytes: &[u8],
        _new_key_bytes: &[u8],
    ) -> EncryptorResult<Vec<u8>> {
        Ok(data.to_vec())
    }

    fn is_available(&self) -> bool {
        true
    }
}
