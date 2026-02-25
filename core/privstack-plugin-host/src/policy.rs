//! Enterprise policy engine — reads `~/.privstack/policy.toml` and enforces
//! admin-managed plugin allowlists, permission overrides, and audit settings.

use crate::permissions::Permission;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Enterprise policy mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicyMode {
    #[default]
    /// Only explicitly listed plugins can be installed.
    Allowlist,
    /// All plugins except explicitly listed ones can be installed.
    Denylist,
    /// No restrictions on plugin installation.
    Unrestricted,
}

/// Audit configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    pub enabled: bool,
    #[serde(default = "default_audit_format")]
    pub export_format: String,
    #[serde(default = "default_audit_path")]
    pub export_path: String,
}

fn default_audit_format() -> String {
    "json".to_string()
}

fn default_audit_path() -> String {
    "~/.privstack/audit/".to_string()
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            export_format: default_audit_format(),
            export_path: default_audit_path(),
        }
    }
}

/// Policy configuration parsed from `policy.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    #[serde(default = "default_policy_mode")]
    pub mode: PolicyMode,
    #[serde(default)]
    pub allowed_plugin_ids: Vec<String>,
    #[serde(default)]
    pub allowed_signing_keys: Vec<String>,
    #[serde(default)]
    pub denied_permissions: HashSet<String>,
    #[serde(default)]
    pub audit: AuditConfig,
}

fn default_policy_mode() -> PolicyMode {
    PolicyMode::Unrestricted
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            mode: PolicyMode::Unrestricted,
            allowed_plugin_ids: Vec::new(),
            allowed_signing_keys: Vec::new(),
            denied_permissions: HashSet::new(),
            audit: AuditConfig::default(),
        }
    }
}

/// Enforces enterprise policy decisions.
pub struct PolicyEngine {
    config: PolicyConfig,
    policy_path: Option<PathBuf>,
}

impl PolicyEngine {
    /// Loads policy from `~/.privstack/policy.toml` if it exists.
    /// Falls back to unrestricted mode with a warning on parse errors.
    pub fn load() -> Self {
        Self::load_from(dirs_path().join("policy.toml"))
    }

    /// Loads policy from an explicit path.
    pub fn load_from(policy_path: PathBuf) -> Self {
        if !policy_path.exists() {
            info!("No policy file found at {:?}, running unrestricted", policy_path);
            return Self {
                config: PolicyConfig::default(),
                policy_path: None,
            };
        }

        match std::fs::read_to_string(&policy_path) {
            Ok(contents) => match toml::from_str::<PolicyFile>(&contents) {
                Ok(file) => {
                    info!("Loaded enterprise policy from {:?}", policy_path);
                    Self {
                        config: file.into_config(),
                        policy_path: Some(policy_path),
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to parse policy file {:?}: {}. Falling back to unrestricted mode.",
                        policy_path, e
                    );
                    Self {
                        config: PolicyConfig::default(),
                        policy_path: Some(policy_path),
                    }
                }
            },
            Err(e) => {
                warn!("Failed to read policy file {:?}: {}", policy_path, e);
                Self {
                    config: PolicyConfig::default(),
                    policy_path: Some(policy_path),
                }
            }
        }
    }

    /// Creates a policy engine with explicit config (for testing).
    pub fn with_config(config: PolicyConfig) -> Self {
        Self {
            config,
            policy_path: None,
        }
    }

    /// Check if a plugin is allowed to be installed.
    pub fn is_plugin_allowed(&self, plugin_id: &str, signing_key: Option<&str>) -> bool {
        match self.config.mode {
            PolicyMode::Unrestricted => true,
            PolicyMode::Allowlist => {
                self.config.allowed_plugin_ids.iter().any(|id| id == plugin_id)
                    || signing_key
                        .map(|k| self.config.allowed_signing_keys.iter().any(|ak| ak == k))
                        .unwrap_or(false)
            }
            PolicyMode::Denylist => {
                // In denylist mode, check the denied list is repurposed as a block list.
                // For simplicity: allowlist IDs act as denylist in this mode.
                !self.config.allowed_plugin_ids.iter().any(|id| id == plugin_id)
            }
        }
    }

    /// Check if a permission is denied by enterprise policy.
    /// If denied by policy, the JIT prompt is never shown.
    pub fn is_permission_denied_by_policy(&self, permission: Permission) -> bool {
        self.config
            .denied_permissions
            .contains(permission.interface_name())
    }

    /// Returns audit config.
    pub fn audit_config(&self) -> &AuditConfig {
        &self.config.audit
    }

    /// Returns whether a policy file was found.
    pub fn has_policy_file(&self) -> bool {
        self.policy_path.is_some()
    }

    /// Returns the active policy config.
    pub fn config(&self) -> &PolicyConfig {
        &self.config
    }
}

/// Raw TOML structure matching the policy.toml format.
#[derive(Deserialize)]
struct PolicyFile {
    #[serde(default)]
    policy: PolicySection,
}

#[derive(Deserialize, Default)]
struct PolicySection {
    #[serde(default = "default_policy_mode")]
    mode: PolicyMode,
    #[serde(default, rename = "allowed-plugins")]
    allowed_plugins: AllowedPlugins,
    #[serde(default, rename = "denied-permissions")]
    denied_permissions: DeniedPermissions,
    #[serde(default)]
    audit: AuditConfig,
}

#[derive(Deserialize, Default)]
struct AllowedPlugins {
    #[serde(default)]
    ids: Vec<String>,
    #[serde(default)]
    keys: Vec<String>,
}

#[derive(Deserialize, Default)]
struct DeniedPermissions {
    #[serde(default)]
    network: bool,
    #[serde(default)]
    filesystem: bool,
    #[serde(default)]
    vault: bool,
    #[serde(default)]
    agent: bool,
    #[serde(default)]
    linking: bool,
    #[serde(default)]
    dialogs: bool,
}

impl PolicyFile {
    fn into_config(self) -> PolicyConfig {
        let mut denied = HashSet::new();
        if self.policy.denied_permissions.network {
            denied.insert("network".to_string());
        }
        if self.policy.denied_permissions.filesystem {
            denied.insert("filesystem".to_string());
        }
        if self.policy.denied_permissions.vault {
            denied.insert("vault".to_string());
        }
        if self.policy.denied_permissions.agent {
            denied.insert("agent".to_string());
        }
        if self.policy.denied_permissions.linking {
            denied.insert("linking".to_string());
        }
        if self.policy.denied_permissions.dialogs {
            denied.insert("dialogs".to_string());
        }

        PolicyConfig {
            mode: self.policy.mode,
            allowed_plugin_ids: self.policy.allowed_plugins.ids,
            allowed_signing_keys: self.policy.allowed_plugins.keys,
            denied_permissions: denied,
            audit: self.policy.audit,
        }
    }
}

/// Resolve the PrivStack config directory.
fn dirs_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        Path::new(&home).join(".privstack")
    } else if let Ok(home) = std::env::var("USERPROFILE") {
        Path::new(&home).join(".privstack")
    } else {
        PathBuf::from(".privstack")
    }
}
