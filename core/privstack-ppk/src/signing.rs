//! Ed25519 signing and verification for .ppk packages.

use ed25519_dalek::{
    Signer as _, Verifier as _,
    SigningKey as DalekSigningKey,
    VerifyingKey as DalekVerifyingKey,
    Signature as DalekSignature,
};
use rand::rngs::OsRng;

use crate::PpkError;

/// Ed25519 signing key (secret). Used to sign .ppk packages.
pub struct SigningKey(DalekSigningKey);

/// Ed25519 verifying key (public). Used to verify .ppk package signatures.
pub struct VerifyingKey(DalekVerifyingKey);

/// Ed25519 signature bytes.
pub struct Signature(DalekSignature);

/// A keypair for signing and verification.
pub struct KeyPair {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl KeyPair {
    /// Generates a new random Ed25519 keypair.
    pub fn generate() -> Self {
        let signing = DalekSigningKey::generate(&mut OsRng);
        let verifying = signing.verifying_key();
        Self {
            signing_key: SigningKey(signing),
            verifying_key: VerifyingKey(verifying),
        }
    }
}

impl SigningKey {
    /// Creates a signing key from raw 32-byte secret.
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self(DalekSigningKey::from_bytes(bytes))
    }

    /// Returns the raw 32-byte secret key.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Signs a message and returns the signature.
    pub fn sign(&self, message: &[u8]) -> Signature {
        Signature(self.0.sign(message))
    }

    /// Returns the corresponding verifying key.
    pub fn verifying_key(&self) -> VerifyingKey {
        VerifyingKey(self.0.verifying_key())
    }
}

impl VerifyingKey {
    /// Creates a verifying key from raw 32-byte public key.
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, PpkError> {
        DalekVerifyingKey::from_bytes(bytes)
            .map(Self)
            .map_err(|_| PpkError::InvalidPublicKey)
    }

    /// Returns the raw 32-byte public key.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Verifies a signature against a message.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<(), PpkError> {
        self.0
            .verify(message, &signature.0)
            .map_err(|_| PpkError::SignatureInvalid)
    }
}

impl Signature {
    /// Creates a signature from raw 64-byte value.
    pub fn from_bytes(bytes: &[u8; 64]) -> Self {
        Self(DalekSignature::from_bytes(bytes))
    }

    /// Returns the raw 64-byte signature.
    pub fn to_bytes(&self) -> [u8; 64] {
        self.0.to_bytes()
    }
}
