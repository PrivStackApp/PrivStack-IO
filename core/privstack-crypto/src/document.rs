//! Entity-level encryption.
//!
//! Provides encryption and decryption for arbitrary data using
//! a two-tier key architecture:
//!
//! 1. Master Key: Derived from user's password using Argon2id
//! 2. Entity Key: Random key per entity, encrypted with master key
//!
//! This allows changing the master password without re-encrypting all data.

use crate::cipher::{self, EncryptedData};
use crate::error::{CryptoError, CryptoResult};
use crate::key::{generate_random_key, DerivedKey, KEY_SIZE};
use serde::{Deserialize, Serialize};

/// An encrypted entity with all metadata needed for decryption.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedDocument {
    /// Entity ID (stored in plaintext for indexing).
    pub id: String,
    /// The entity key, encrypted with the master key.
    pub encrypted_key: EncryptedData,
    /// The entity content, encrypted with the entity key.
    pub encrypted_content: EncryptedData,
    /// Version of the encryption format.
    pub version: u8,
}

impl EncryptedDocument {
    /// Current encryption format version.
    pub const CURRENT_VERSION: u8 = 1;
}

/// Encrypts arbitrary data using a master key.
///
/// # Process
/// 1. Generate a random entity key
/// 2. Encrypt the data with the entity key
/// 3. Encrypt the entity key with the master key
pub fn encrypt_document(
    id: &str,
    data: &[u8],
    master_key: &DerivedKey,
) -> CryptoResult<EncryptedDocument> {
    let entity_key = generate_random_key();

    let encrypted_content = cipher::encrypt(&entity_key, data)?;
    let encrypted_key = cipher::encrypt(master_key, entity_key.as_bytes())?;

    Ok(EncryptedDocument {
        id: id.to_string(),
        encrypted_key,
        encrypted_content,
        version: EncryptedDocument::CURRENT_VERSION,
    })
}

/// Decrypts entity data using a master key.
///
/// Returns the raw decrypted bytes.
pub fn decrypt_document(
    encrypted: &EncryptedDocument,
    master_key: &DerivedKey,
) -> CryptoResult<Vec<u8>> {
    let doc_key_bytes = cipher::decrypt(master_key, &encrypted.encrypted_key)?;

    if doc_key_bytes.len() != KEY_SIZE {
        return Err(CryptoError::InvalidKeyLength {
            expected: KEY_SIZE,
            actual: doc_key_bytes.len(),
        });
    }

    let mut key_array = [0u8; KEY_SIZE];
    key_array.copy_from_slice(&doc_key_bytes);
    let doc_key = DerivedKey::from_bytes(key_array);

    cipher::decrypt(&doc_key, &encrypted.encrypted_content)
}

/// Re-encrypts an entity's key with a new master key.
///
/// The content is not re-encrypted; only the key wrapper is updated.
/// Used when the user changes their password.
pub fn reencrypt_document_key(
    encrypted: &EncryptedDocument,
    old_key: &DerivedKey,
    new_key: &DerivedKey,
) -> CryptoResult<EncryptedDocument> {
    let doc_key_bytes = cipher::decrypt(old_key, &encrypted.encrypted_key)?;
    let encrypted_key = cipher::encrypt(new_key, &doc_key_bytes)?;

    Ok(EncryptedDocument {
        id: encrypted.id.clone(),
        encrypted_key,
        encrypted_content: encrypted.encrypted_content.clone(),
        version: encrypted.version,
    })
}

/// Metadata about an encrypted entity (for display without decryption).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedDocumentMetadata {
    /// Entity ID.
    pub id: String,
    /// Encryption format version.
    pub version: u8,
    /// Approximate encrypted size in bytes.
    pub encrypted_size: usize,
}

impl From<&EncryptedDocument> for EncryptedDocumentMetadata {
    fn from(doc: &EncryptedDocument) -> Self {
        Self {
            id: doc.id.clone(),
            version: doc.version,
            encrypted_size: doc.encrypted_content.len() + doc.encrypted_key.len(),
        }
    }
}
