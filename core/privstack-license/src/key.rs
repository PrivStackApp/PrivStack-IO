//! License key parsing and Ed25519 signature verification.
//!
//! License keys use the format: `base64url(payload).base64url(signature)`
//!
//! The payload is a JSON object containing:
//! - `sub`: user ID (i64)
//! - `email`: user email
//! - `plan`: license plan (monthly/annual/perpetual)
//! - `iat`: issued-at timestamp (seconds since epoch)
//!
//! The signature covers `payload_b64.as_bytes()` (the base64url-encoded
//! payload string, not the decoded JSON), matching the server implementation.

use crate::error::{LicenseError, LicenseResult};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Grace period in seconds (30 days).
pub const GRACE_PERIOD_SECS: i64 = 30 * 24 * 60 * 60;

/// Embedded Ed25519 public key for production license verification (32 bytes).
const LICENSE_PUBLIC_KEY: [u8; 32] = [
    200, 117, 135, 135, 90, 37, 182, 86, 29, 134, 157, 3, 221, 181, 177, 210,
    150, 205, 208, 203, 165, 118, 231, 60, 111, 225, 138, 33, 96, 160, 80, 135,
];

/// The license plan (aligned with server).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LicensePlan {
    /// Trial (limited-time free access).
    Trial,
    /// Monthly subscription.
    Monthly,
    /// Annual subscription.
    Annual,
    /// Perpetual (one-time purchase, never expires).
    Perpetual,
}

impl LicensePlan {
    /// Returns the maximum number of devices for this plan.
    #[must_use]
    pub fn max_devices(&self) -> u32 {
        match self {
            Self::Trial => 1,
            Self::Monthly => 3,
            Self::Annual => 5,
            Self::Perpetual => 5,
        }
    }

    /// Returns true if this plan includes priority support.
    #[must_use]
    pub fn has_priority_support(&self) -> bool {
        matches!(self, Self::Annual | Self::Perpetual)
    }

    /// Returns the duration in seconds for this plan, or None for perpetual.
    #[must_use]
    pub fn duration_secs(&self) -> Option<i64> {
        match self {
            Self::Trial => Some(14 * 24 * 60 * 60),
            Self::Monthly => Some(30 * 24 * 60 * 60),
            Self::Annual => Some(365 * 24 * 60 * 60),
            Self::Perpetual => None,
        }
    }
}

/// The current status of a license.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LicenseStatus {
    /// License is valid and active.
    Active,
    /// License is in grace period (30 days post-expiry, full functionality).
    Grace {
        /// Days remaining in grace period.
        days_remaining: u32,
    },
    /// License is past grace period (view/export only).
    ReadOnly,
    /// License has fully expired.
    Expired,
    /// License not yet activated.
    NotActivated,
}

impl LicenseStatus {
    /// Returns true if the license allows full app usage (Active or Grace).
    #[must_use]
    pub fn is_usable(&self) -> bool {
        matches!(self, Self::Active | Self::Grace { .. })
    }

    /// Returns true if the license allows viewing data (Active, Grace, or ReadOnly).
    #[must_use]
    pub fn is_viewable(&self) -> bool {
        matches!(self, Self::Active | Self::Grace { .. } | Self::ReadOnly)
    }
}

/// The decoded license payload (matches server JSON structure).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicensePayload {
    /// User ID.
    pub sub: i64,
    /// User email.
    pub email: String,
    /// License plan.
    pub plan: LicensePlan,
    /// Issued-at timestamp (seconds since epoch).
    pub iat: i64,
}

/// A parsed and verified license key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseKey {
    /// The raw key string.
    raw: String,
    /// Decoded payload.
    payload: LicensePayload,
    /// Expiration timestamp (seconds since epoch), or None for perpetual.
    expires_at: Option<i64>,
}

impl LicenseKey {
    /// Parses and verifies a license key string using the embedded public key.
    ///
    /// # Errors
    ///
    /// Returns an error if the key format is invalid or signature verification fails.
    pub fn parse(key: &str) -> LicenseResult<Self> {
        Self::parse_with_key(key, &LICENSE_PUBLIC_KEY)
    }

    /// Parses and verifies a license key string using a custom public key.
    /// Used for testing with a generated key pair.
    pub fn parse_with_key(key: &str, pub_key_bytes: &[u8; 32]) -> LicenseResult<Self> {
        let key = key.trim();

        // Split into payload and signature parts
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() != 2 {
            return Err(LicenseError::InvalidKeyFormat(
                "key must have exactly two parts separated by a dot".to_string(),
            ));
        }

        let payload_b64 = parts[0];
        let signature_b64 = parts[1];

        // Decode signature
        let sig_bytes = URL_SAFE_NO_PAD.decode(signature_b64).map_err(|e| {
            LicenseError::InvalidKeyFormat(format!("invalid signature base64: {e}"))
        })?;

        let signature = Signature::from_slice(&sig_bytes).map_err(|_| {
            LicenseError::InvalidKeyFormat("invalid signature length".to_string())
        })?;

        // Build verifying key
        let verifying_key = VerifyingKey::from_bytes(pub_key_bytes).map_err(|_| {
            LicenseError::InvalidKeyFormat("invalid public key".to_string())
        })?;

        // Verify signature over the base64url-encoded payload bytes (matches server)
        verifying_key
            .verify(payload_b64.as_bytes(), &signature)
            .map_err(|_| LicenseError::InvalidSignature)?;

        // Decode payload JSON
        let payload_json = URL_SAFE_NO_PAD.decode(payload_b64).map_err(|e| {
            LicenseError::InvalidKeyFormat(format!("invalid payload base64: {e}"))
        })?;

        let payload: LicensePayload = serde_json::from_slice(&payload_json).map_err(|e| {
            LicenseError::InvalidPayload(format!("invalid payload JSON: {e}"))
        })?;

        // Compute expiration
        let expires_at = payload.plan.duration_secs().map(|d| payload.iat + d);

        Ok(Self {
            raw: key.to_string(),
            payload,
            expires_at,
        })
    }

    /// Returns the raw key string.
    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Returns the license plan.
    #[must_use]
    pub fn license_plan(&self) -> LicensePlan {
        self.payload.plan
    }

    /// Returns the decoded payload.
    #[must_use]
    pub fn payload(&self) -> &LicensePayload {
        &self.payload
    }

    /// Returns the issued-at timestamp (seconds since epoch).
    #[must_use]
    pub fn issued_at_secs(&self) -> i64 {
        self.payload.iat
    }

    /// Returns the expiration timestamp (seconds since epoch), or None for perpetual.
    #[must_use]
    pub fn expires_at_secs(&self) -> Option<i64> {
        self.expires_at
    }

    /// Returns the current license status based on expiration and grace period.
    #[must_use]
    pub fn status(&self) -> LicenseStatus {
        let now = chrono::Utc::now().timestamp();

        match self.expires_at {
            None => LicenseStatus::Active, // Perpetual
            Some(exp) => {
                if now < exp {
                    LicenseStatus::Active
                } else {
                    let secs_past_expiry = now - exp;
                    if secs_past_expiry < GRACE_PERIOD_SECS {
                        let days_remaining =
                            ((GRACE_PERIOD_SECS - secs_past_expiry) / (24 * 60 * 60)) as u32;
                        LicenseStatus::Grace { days_remaining }
                    } else {
                        LicenseStatus::ReadOnly
                    }
                }
            }
        }
    }
}
