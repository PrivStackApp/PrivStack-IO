//! Licensing and activation for PrivStack.
//!
//! This module handles:
//! - License key validation via Ed25519 signature verification
//! - One-time activation with server
//! - Hardware fingerprinting for device binding
//! - Offline license verification
//!
//! # Design Principles
//!
//! - **One-time activation**: Server is only contacted once during activation
//! - **Zero phoning home**: After activation, no network calls unless user opts in
//! - **Offline-first**: App works without network after initial activation
//! - **Device binding**: License tied to hardware fingerprint
//!
//! # License Key Format
//!
//! Keys are formatted as: `base64url(payload).base64url(signature)`
//! The payload is a JSON object signed with Ed25519, containing:
//! - User ID, email, plan, and issued-at timestamp

mod activation;
mod device;
mod error;
mod key;

pub use activation::{activate_offline, Activation, ActivationStore};
pub use device::{DeviceFingerprint, DeviceInfo};
pub use error::{LicenseError, LicenseResult};
pub use key::{LicenseKey, LicensePayload, LicensePlan, LicenseStatus, GRACE_PERIOD_SECS};

#[cfg(feature = "online")]
pub use activation::activate_online;
