//! Key derivation and management.
//!
//! Uses Argon2id for deriving encryption keys from passwords.

use crate::error::{CryptoError, CryptoResult};
use argon2::{Argon2, Params, Version};
use rand::RngCore;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Size of encryption keys in bytes (256 bits for ChaCha20).
pub const KEY_SIZE: usize = 32;

/// Size of salt in bytes.
pub const SALT_SIZE: usize = 16;

/// A derived encryption key with automatic zeroization on drop.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct DerivedKey {
    bytes: [u8; KEY_SIZE],
}

impl DerivedKey {
    /// Creates a new derived key from raw bytes.
    ///
    /// # Panics
    /// Panics if bytes length is not KEY_SIZE.
    pub fn from_bytes(bytes: [u8; KEY_SIZE]) -> Self {
        Self { bytes }
    }

    /// Returns the key bytes.
    pub fn as_bytes(&self) -> &[u8; KEY_SIZE] {
        &self.bytes
    }
}

impl std::fmt::Debug for DerivedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DerivedKey")
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}

/// Salt for key derivation.
#[derive(Clone, Debug)]
pub struct Salt {
    bytes: [u8; SALT_SIZE],
}

impl Salt {
    /// Generates a random salt.
    pub fn random() -> Self {
        let mut bytes = [0u8; SALT_SIZE];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        Self { bytes }
    }

    /// Creates a salt from raw bytes.
    pub fn from_bytes(bytes: [u8; SALT_SIZE]) -> Self {
        Self { bytes }
    }

    /// Returns the salt bytes.
    pub fn as_bytes(&self) -> &[u8; SALT_SIZE] {
        &self.bytes
    }
}

/// Key derivation parameters.
///
/// Default values are tuned for a balance of security and performance
/// on modern hardware.
#[derive(Clone, Debug)]
pub struct KdfParams {
    /// Memory cost in KiB.
    pub memory_cost: u32,
    /// Time cost (iterations).
    pub time_cost: u32,
    /// Parallelism factor.
    pub parallelism: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        // OWASP recommendations for Argon2id (2023)
        // These values provide good security while keeping derivation under 1 second
        Self {
            memory_cost: 19 * 1024, // 19 MiB
            time_cost: 2,
            parallelism: 1,
        }
    }
}

impl KdfParams {
    /// Creates parameters for testing (fast but insecure).
    #[cfg(test)]
    pub fn test() -> Self {
        Self {
            memory_cost: 1024, // 1 MiB
            time_cost: 1,
            parallelism: 1,
        }
    }
}

/// Derives an encryption key from a password using Argon2id.
///
/// # Arguments
/// * `password` - The user's password
/// * `salt` - A unique salt for this derivation
/// * `params` - Key derivation parameters
///
/// # Returns
/// A derived key suitable for use with ChaCha20-Poly1305.
pub fn derive_key(password: &str, salt: &Salt, params: &KdfParams) -> CryptoResult<DerivedKey> {
    let argon2_params = Params::new(
        params.memory_cost,
        params.time_cost,
        params.parallelism,
        Some(KEY_SIZE),
    )
    .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;

    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, argon2_params);

    let mut key_bytes = [0u8; KEY_SIZE];
    argon2
        .hash_password_into(password.as_bytes(), salt.as_bytes(), &mut key_bytes)
        .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;

    Ok(DerivedKey::from_bytes(key_bytes))
}

/// Generates a random encryption key (for document keys, not password-derived).
pub fn generate_random_key() -> DerivedKey {
    let mut bytes = [0u8; KEY_SIZE];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    DerivedKey::from_bytes(bytes)
}
