//! Device fingerprinting for license binding.
//!
//! Generates a stable hardware fingerprint that identifies this device.
//! Used to bind licenses to specific machines and prevent sharing.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;

/// Information about the current device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Operating system name.
    pub os_name: String,
    /// Operating system version.
    pub os_version: String,
    /// Hostname.
    pub hostname: String,
    /// CPU architecture.
    pub arch: String,
}

impl DeviceInfo {
    /// Collects information about the current device.
    #[must_use]
    pub fn collect() -> Self {
        Self {
            os_name: env::consts::OS.to_string(),
            os_version: get_os_version(),
            hostname: get_hostname(),
            arch: env::consts::ARCH.to_string(),
        }
    }
}

/// A stable fingerprint that identifies this device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceFingerprint {
    /// The fingerprint ID (hash of hardware identifiers).
    id: String,
    /// When the fingerprint was generated.
    generated_at: chrono::DateTime<chrono::Utc>,
}

impl DeviceFingerprint {
    /// Generates a fingerprint for the current device.
    ///
    /// This combines multiple hardware identifiers to create a stable ID
    /// that survives reboots but changes if hardware changes significantly.
    #[must_use]
    pub fn generate() -> Self {
        let components = collect_hardware_ids();
        let combined = components.join("|");

        let mut hasher = Sha256::new();
        hasher.update(combined.as_bytes());
        let hash = hasher.finalize();

        let id = BASE64.encode(&hash[..16]); // Use first 16 bytes

        Self {
            id,
            generated_at: chrono::Utc::now(),
        }
    }

    /// Returns the fingerprint ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Validates that this fingerprint matches the current device.
    #[must_use]
    pub fn matches_current(&self) -> bool {
        let current = Self::generate();
        self.id == current.id
    }
}

/// Collects hardware identifiers for fingerprinting.
fn collect_hardware_ids() -> Vec<String> {
    let mut ids = Vec::new();

    // OS and architecture (stable)
    ids.push(env::consts::OS.to_string());
    ids.push(env::consts::ARCH.to_string());

    // Hostname (can change but usually stable)
    ids.push(get_hostname());

    // Machine ID (platform-specific, very stable)
    if let Some(machine_id) = get_machine_id() {
        ids.push(machine_id);
    }

    // Username as fallback component
    if let Ok(user) = env::var("USER").or_else(|_| env::var("USERNAME")) {
        ids.push(user);
    }

    ids
}

/// Gets the machine hostname.
fn get_hostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Gets the OS version string.
fn get_os_version() -> String {
    // Platform-specific version detection
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }

    #[cfg(target_os = "windows")]
    {
        "windows".to_string() // Would use Windows API in production
    }

    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|l| l.starts_with("VERSION_ID="))
                    .map(|l| {
                        l.trim_start_matches("VERSION_ID=")
                            .trim_matches('"')
                            .to_string()
                    })
            })
            .unwrap_or_else(|| "unknown".to_string())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        "unknown".to_string()
    }
}

/// Gets the machine ID (platform-specific unique identifier).
fn get_machine_id() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        // Use IOPlatformSerialNumber or hardware UUID
        std::process::Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|output| {
                output
                    .lines()
                    .find(|l| l.contains("IOPlatformUUID"))
                    .and_then(|l| l.split('"').nth(3))
                    .map(String::from)
            })
    }

    #[cfg(target_os = "linux")]
    {
        // Try /etc/machine-id first, then /var/lib/dbus/machine-id
        std::fs::read_to_string("/etc/machine-id")
            .or_else(|_| std::fs::read_to_string("/var/lib/dbus/machine-id"))
            .ok()
            .map(|s| s.trim().to_string())
    }

    #[cfg(target_os = "windows")]
    {
        // Would use Windows registry MachineGuid in production
        None
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        None
    }
}
