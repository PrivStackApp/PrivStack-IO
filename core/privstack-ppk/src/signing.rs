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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_verify_roundtrip() {
        let kp = KeyPair::generate();
        let msg = b"hello world";
        let sig = kp.signing_key.sign(msg);
        assert!(kp.verifying_key.verify(msg, &sig).is_ok());
    }

    #[test]
    fn wrong_message_fails() {
        let kp = KeyPair::generate();
        let sig = kp.signing_key.sign(b"correct");
        assert!(kp.verifying_key.verify(b"wrong", &sig).is_err());
    }

    #[test]
    fn wrong_key_fails() {
        let kp1 = KeyPair::generate();
        let kp2 = KeyPair::generate();
        let sig = kp1.signing_key.sign(b"message");
        assert!(kp2.verifying_key.verify(b"message", &sig).is_err());
    }

    #[test]
    fn key_bytes_roundtrip() {
        let kp = KeyPair::generate();
        let secret = kp.signing_key.to_bytes();
        let public = kp.verifying_key.to_bytes();

        let sk = SigningKey::from_bytes(&secret);
        let vk = VerifyingKey::from_bytes(&public).unwrap();

        let sig = sk.sign(b"test");
        assert!(vk.verify(b"test", &sig).is_ok());
    }

    #[test]
    fn signature_bytes_roundtrip() {
        let kp = KeyPair::generate();
        let sig = kp.signing_key.sign(b"data");
        let bytes = sig.to_bytes();
        let restored = Signature::from_bytes(&bytes);
        assert!(kp.verifying_key.verify(b"data", &restored).is_ok());
    }

    #[test]
    fn verifying_key_from_signing_key() {
        let kp = KeyPair::generate();
        let derived = kp.signing_key.verifying_key();
        let sig = kp.signing_key.sign(b"check");
        assert!(derived.verify(b"check", &sig).is_ok());
    }
}
