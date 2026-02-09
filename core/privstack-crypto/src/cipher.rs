//! Document encryption using ChaCha20-Poly1305.
//!
//! Provides authenticated encryption with associated data (AEAD).

use crate::error::{CryptoError, CryptoResult};
use crate::key::DerivedKey;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};

/// Size of nonce in bytes (96 bits for ChaCha20-Poly1305).
pub const NONCE_SIZE: usize = 12;

/// Size of authentication tag in bytes.
pub const TAG_SIZE: usize = 16;

/// Encrypted data with metadata needed for decryption.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedData {
    /// The nonce used for encryption (unique per encryption).
    pub nonce: [u8; NONCE_SIZE],
    /// The encrypted ciphertext (includes auth tag).
    pub ciphertext: Vec<u8>,
}

impl EncryptedData {
    /// Returns the total size of the encrypted data.
    pub fn len(&self) -> usize {
        NONCE_SIZE + self.ciphertext.len()
    }

    /// Returns true if the ciphertext is empty.
    pub fn is_empty(&self) -> bool {
        self.ciphertext.is_empty()
    }

    /// Encodes to base64 for storage/transmission.
    pub fn to_base64(&self) -> String {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let mut bytes = Vec::with_capacity(self.len());
        bytes.extend_from_slice(&self.nonce);
        bytes.extend_from_slice(&self.ciphertext);
        STANDARD.encode(&bytes)
    }

    /// Decodes from base64.
    pub fn from_base64(encoded: &str) -> CryptoResult<Self> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|e| CryptoError::Decryption(format!("invalid base64: {}", e)))?;

        if bytes.len() < NONCE_SIZE + TAG_SIZE {
            return Err(CryptoError::Decryption("data too short".to_string()));
        }

        let mut nonce = [0u8; NONCE_SIZE];
        nonce.copy_from_slice(&bytes[..NONCE_SIZE]);
        let ciphertext = bytes[NONCE_SIZE..].to_vec();

        Ok(Self { nonce, ciphertext })
    }
}

/// Encrypts plaintext using ChaCha20-Poly1305.
///
/// # Arguments
/// * `key` - The encryption key
/// * `plaintext` - Data to encrypt
///
/// # Returns
/// Encrypted data with nonce and ciphertext.
pub fn encrypt(key: &DerivedKey, plaintext: &[u8]) -> CryptoResult<EncryptedData> {
    let cipher = ChaCha20Poly1305::new(key.as_bytes().into());

    // Generate random nonce
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| CryptoError::Encryption(e.to_string()))?;

    Ok(EncryptedData {
        nonce: nonce_bytes,
        ciphertext,
    })
}

/// Decrypts ciphertext using ChaCha20-Poly1305.
///
/// # Arguments
/// * `key` - The encryption key (must be the same key used for encryption)
/// * `encrypted` - The encrypted data
///
/// # Returns
/// The decrypted plaintext, or an error if decryption fails.
pub fn decrypt(key: &DerivedKey, encrypted: &EncryptedData) -> CryptoResult<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new(key.as_bytes().into());
    let nonce = Nonce::from_slice(&encrypted.nonce);

    cipher
        .decrypt(nonce, encrypted.ciphertext.as_ref())
        .map_err(|_| {
            CryptoError::Decryption("decryption failed (wrong key or tampered data)".to_string())
        })
}

/// Encrypts a string and returns base64-encoded result.
pub fn encrypt_string(key: &DerivedKey, plaintext: &str) -> CryptoResult<String> {
    let encrypted = encrypt(key, plaintext.as_bytes())?;
    Ok(encrypted.to_base64())
}

/// Decrypts a base64-encoded string.
pub fn decrypt_string(key: &DerivedKey, encoded: &str) -> CryptoResult<String> {
    let encrypted = EncryptedData::from_base64(encoded)?;
    let plaintext = decrypt(key, &encrypted)?;
    String::from_utf8(plaintext)
        .map_err(|e| CryptoError::Decryption(format!("invalid UTF-8: {}", e)))
}
