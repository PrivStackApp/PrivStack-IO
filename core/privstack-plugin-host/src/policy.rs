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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrestricted_allows_all() {
        let engine = PolicyEngine::with_config(PolicyConfig::default());
        assert!(engine.is_plugin_allowed("anything", None));
        assert!(!engine.is_permission_denied_by_policy(Permission::Vault));
    }

    #[test]
    fn allowlist_mode() {
        let config = PolicyConfig {
            mode: PolicyMode::Allowlist,
            allowed_plugin_ids: vec!["privstack.notes".to_string()],
            allowed_signing_keys: vec!["key123".to_string()],
            ..Default::default()
        };
        let engine = PolicyEngine::with_config(config);

        assert!(engine.is_plugin_allowed("privstack.notes", None));
        assert!(!engine.is_plugin_allowed("evil.plugin", None));
        assert!(engine.is_plugin_allowed("any.plugin", Some("key123")));
    }

    #[test]
    fn denied_permissions() {
        let mut denied = HashSet::new();
        denied.insert("network".to_string());
        denied.insert("filesystem".to_string());

        let config = PolicyConfig {
            denied_permissions: denied,
            ..Default::default()
        };
        let engine = PolicyEngine::with_config(config);

        assert!(engine.is_permission_denied_by_policy(Permission::Network));
        assert!(engine.is_permission_denied_by_policy(Permission::Filesystem));
        assert!(!engine.is_permission_denied_by_policy(Permission::Vault));
    }

    #[test]
    fn parse_policy_toml() {
        let toml_str = r#"
[policy]
mode = "allowlist"

[policy.allowed-plugins]
ids = ["privstack.notes", "privstack.tasks"]
keys = ["official-key"]

[policy.denied-permissions]
network = true
filesystem = true

[policy.audit]
enabled = true
export_format = "json"
export_path = "/tmp/audit"
"#;
        let file: PolicyFile = toml::from_str(toml_str).unwrap();
        let config = file.into_config();

        assert_eq!(config.mode, PolicyMode::Allowlist);
        assert_eq!(config.allowed_plugin_ids.len(), 2);
        assert!(config.denied_permissions.contains("network"));
        assert!(config.audit.enabled);
    }

    // ================================================================
    // Denylist mode
    // ================================================================

    #[test]
    fn denylist_mode_blocks_listed_allows_others() {
        let config = PolicyConfig {
            mode: PolicyMode::Denylist,
            allowed_plugin_ids: vec!["evil.plugin".to_string()],
            ..Default::default()
        };
        let engine = PolicyEngine::with_config(config);

        // "allowed_plugin_ids" in denylist mode acts as the block list
        assert!(!engine.is_plugin_allowed("evil.plugin", None));
        assert!(engine.is_plugin_allowed("good.plugin", None));
        assert!(engine.is_plugin_allowed("any.other", None));
    }

    // ================================================================
    // Signing key allowlisting
    // ================================================================

    #[test]
    fn allowlist_mode_signing_key_allows() {
        let config = PolicyConfig {
            mode: PolicyMode::Allowlist,
            allowed_plugin_ids: vec![],
            allowed_signing_keys: vec!["trusted-key-abc".to_string()],
            ..Default::default()
        };
        let engine = PolicyEngine::with_config(config);

        // Plugin not in allowed IDs, but has trusted signing key
        assert!(engine.is_plugin_allowed("unknown.plugin", Some("trusted-key-abc")));
        // Wrong key
        assert!(!engine.is_plugin_allowed("unknown.plugin", Some("wrong-key")));
        // No key
        assert!(!engine.is_plugin_allowed("unknown.plugin", None));
    }

    #[test]
    fn allowlist_mode_id_or_key_sufficient() {
        let config = PolicyConfig {
            mode: PolicyMode::Allowlist,
            allowed_plugin_ids: vec!["known.plugin".to_string()],
            allowed_signing_keys: vec!["trusted-key".to_string()],
            ..Default::default()
        };
        let engine = PolicyEngine::with_config(config);

        // By ID alone
        assert!(engine.is_plugin_allowed("known.plugin", None));
        // By key alone
        assert!(engine.is_plugin_allowed("other.plugin", Some("trusted-key")));
        // By both
        assert!(engine.is_plugin_allowed("known.plugin", Some("trusted-key")));
        // Neither
        assert!(!engine.is_plugin_allowed("other.plugin", None));
    }

    // ================================================================
    // load() fallback — no policy file means unrestricted
    // ================================================================

    #[test]
    fn load_from_missing_file_is_unrestricted() {
        let dir = tempfile::tempdir().unwrap();
        let fake_path = dir.path().join("nonexistent.toml");

        let engine = PolicyEngine::load_from(fake_path);
        assert!(!engine.has_policy_file());
        assert!(engine.is_plugin_allowed("anything", None));
        assert!(!engine.is_permission_denied_by_policy(Permission::Network));
    }

    /// Helper: write TOML content to a temp file and load via `load_from`.
    fn load_policy_from_str(toml_content: &str) -> PolicyEngine {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("policy.toml");
        std::fs::write(&path, toml_content).unwrap();
        PolicyEngine::load_from(path)
    }

    #[test]
    fn load_from_allowlist_file() {
        let engine = load_policy_from_str(r#"
[policy]
mode = "allowlist"

[policy.allowed-plugins]
ids = ["privstack.notes", "privstack.tasks"]
keys = ["official-signing-key"]
"#);
        assert!(engine.has_policy_file());
        assert_eq!(engine.config().mode, PolicyMode::Allowlist);
        // Allowed by ID
        assert!(engine.is_plugin_allowed("privstack.notes", None));
        assert!(engine.is_plugin_allowed("privstack.tasks", None));
        // Allowed by signing key
        assert!(engine.is_plugin_allowed("unknown.plugin", Some("official-signing-key")));
        // Denied — not in list, no key
        assert!(!engine.is_plugin_allowed("evil.plugin", None));
        assert!(!engine.is_plugin_allowed("evil.plugin", Some("wrong-key")));
    }

    #[test]
    fn load_from_denylist_file() {
        let engine = load_policy_from_str(r#"
[policy]
mode = "denylist"

[policy.allowed-plugins]
ids = ["blocked.plugin"]
"#);
        assert!(engine.has_policy_file());
        assert_eq!(engine.config().mode, PolicyMode::Denylist);
        assert!(!engine.is_plugin_allowed("blocked.plugin", None));
        assert!(engine.is_plugin_allowed("anything.else", None));
    }

    #[test]
    fn load_from_unrestricted_file() {
        let engine = load_policy_from_str(r#"
[policy]
mode = "unrestricted"
"#);
        assert!(engine.has_policy_file());
        assert_eq!(engine.config().mode, PolicyMode::Unrestricted);
        assert!(engine.is_plugin_allowed("anything", None));
    }

    #[test]
    fn load_from_file_with_denied_permissions() {
        let engine = load_policy_from_str(r#"
[policy]
mode = "unrestricted"

[policy.denied-permissions]
network = true
filesystem = true
vault = true
agent = true
linking = true
dialogs = true
"#);
        assert!(engine.is_permission_denied_by_policy(Permission::Network));
        assert!(engine.is_permission_denied_by_policy(Permission::Filesystem));
        assert!(engine.is_permission_denied_by_policy(Permission::Vault));
        assert!(engine.is_permission_denied_by_policy(Permission::Agent));
        assert!(engine.is_permission_denied_by_policy(Permission::Linking));
        assert!(engine.is_permission_denied_by_policy(Permission::Dialogs));
        assert!(!engine.is_permission_denied_by_policy(Permission::Sdk));
    }

    #[test]
    fn load_from_file_with_audit_config() {
        let engine = load_policy_from_str(r#"
[policy]
mode = "unrestricted"

[policy.audit]
enabled = true
export_format = "csv"
export_path = "/var/log/privstack"
"#);
        let audit = engine.audit_config();
        assert!(audit.enabled);
        assert_eq!(audit.export_format, "csv");
        assert_eq!(audit.export_path, "/var/log/privstack");
    }

    #[test]
    fn load_from_empty_policy_section_file() {
        let engine = load_policy_from_str("[policy]\n");
        assert!(engine.has_policy_file());
        assert_eq!(engine.config().mode, PolicyMode::Unrestricted);
        assert!(engine.config().allowed_plugin_ids.is_empty());
        assert!(engine.config().denied_permissions.is_empty());
    }

    #[test]
    fn load_from_malformed_file_falls_back_unrestricted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("policy.toml");
        std::fs::write(&path, "this is not valid toml {{{{").unwrap();

        let engine = PolicyEngine::load_from(path);
        assert!(engine.has_policy_file());
        assert!(engine.is_plugin_allowed("anything", None));
    }

    #[test]
    fn load_from_unreadable_path_falls_back_unrestricted() {
        // Point at a directory instead of a file — read_to_string will fail
        let dir = tempfile::tempdir().unwrap();
        let engine = PolicyEngine::load_from(dir.path().to_path_buf());
        // Path exists (it's a dir) so it tries to read, fails, falls back
        assert!(engine.is_plugin_allowed("anything", None));
    }

    // ================================================================
    // Audit config accessors
    // ================================================================

    #[test]
    fn audit_config_defaults() {
        let engine = PolicyEngine::with_config(PolicyConfig::default());
        let audit = engine.audit_config();
        assert!(!audit.enabled);
        assert_eq!(audit.export_format, "json");
        assert_eq!(audit.export_path, "~/.privstack/audit/");
    }

    #[test]
    fn audit_config_custom() {
        let config = PolicyConfig {
            audit: AuditConfig {
                enabled: true,
                export_format: "csv".to_string(),
                export_path: "/custom/path".to_string(),
            },
            ..Default::default()
        };
        let engine = PolicyEngine::with_config(config);
        let audit = engine.audit_config();
        assert!(audit.enabled);
        assert_eq!(audit.export_format, "csv");
        assert_eq!(audit.export_path, "/custom/path");
    }

    // ================================================================
    // has_policy_file / config accessors
    // ================================================================

    #[test]
    fn with_config_has_no_policy_file() {
        let engine = PolicyEngine::with_config(PolicyConfig::default());
        assert!(!engine.has_policy_file());
    }

    #[test]
    fn config_accessor_returns_current_config() {
        let config = PolicyConfig {
            mode: PolicyMode::Denylist,
            allowed_plugin_ids: vec!["test".to_string()],
            ..Default::default()
        };
        let engine = PolicyEngine::with_config(config);
        assert_eq!(engine.config().mode, PolicyMode::Denylist);
        assert_eq!(engine.config().allowed_plugin_ids.len(), 1);
    }

    // ================================================================
    // All denied permissions variants
    // ================================================================

    #[test]
    fn denied_permissions_all_variants() {
        let toml_str = r#"
[policy]
mode = "unrestricted"

[policy.denied-permissions]
network = true
filesystem = true
vault = true
agent = true
linking = true
dialogs = true
"#;
        let file: PolicyFile = toml::from_str(toml_str).unwrap();
        let config = file.into_config();
        let engine = PolicyEngine::with_config(config);

        assert!(engine.is_permission_denied_by_policy(Permission::Network));
        assert!(engine.is_permission_denied_by_policy(Permission::Filesystem));
        assert!(engine.is_permission_denied_by_policy(Permission::Vault));
        assert!(engine.is_permission_denied_by_policy(Permission::Agent));
        assert!(engine.is_permission_denied_by_policy(Permission::Linking));
        assert!(engine.is_permission_denied_by_policy(Permission::Dialogs));
        // Tier 1 permissions are not deniable via policy
        assert!(!engine.is_permission_denied_by_policy(Permission::Sdk));
    }

    // ================================================================
    // Empty / minimal TOML files
    // ================================================================

    #[test]
    fn empty_policy_section_defaults_to_unrestricted() {
        let toml_str = r#"
[policy]
"#;
        let file: PolicyFile = toml::from_str(toml_str).unwrap();
        let config = file.into_config();
        assert_eq!(config.mode, PolicyMode::Unrestricted);
        assert!(config.allowed_plugin_ids.is_empty());
        assert!(config.denied_permissions.is_empty());
    }

    #[test]
    fn completely_empty_toml_uses_policy_section_default() {
        // When the entire TOML is empty, PolicySection::default() is used,
        // which derives Default for PolicyMode (Allowlist).
        let toml_str = "";
        let file: PolicyFile = toml::from_str(toml_str).unwrap();
        let config = file.into_config();
        assert_eq!(config.mode, PolicyMode::Allowlist);
    }
}
