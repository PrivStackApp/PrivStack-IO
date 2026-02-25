use privstack_plugin_host::*;
use std::collections::HashSet;

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
fn parse_policy_toml_via_load() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("policy.toml");
    std::fs::write(&path, r#"
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
"#).unwrap();
    let engine = PolicyEngine::load_from(path);
    assert_eq!(engine.config().mode, PolicyMode::Allowlist);
    assert_eq!(engine.config().allowed_plugin_ids.len(), 2);
    assert!(engine.config().denied_permissions.contains("network"));
    assert!(engine.audit_config().enabled);
}

#[test]
fn denylist_mode_blocks_listed_allows_others() {
    let config = PolicyConfig {
        mode: PolicyMode::Denylist,
        allowed_plugin_ids: vec!["evil.plugin".to_string()],
        ..Default::default()
    };
    let engine = PolicyEngine::with_config(config);
    assert!(!engine.is_plugin_allowed("evil.plugin", None));
    assert!(engine.is_plugin_allowed("good.plugin", None));
    assert!(engine.is_plugin_allowed("any.other", None));
}

#[test]
fn allowlist_mode_signing_key_allows() {
    let config = PolicyConfig {
        mode: PolicyMode::Allowlist,
        allowed_plugin_ids: vec![],
        allowed_signing_keys: vec!["trusted-key-abc".to_string()],
        ..Default::default()
    };
    let engine = PolicyEngine::with_config(config);
    assert!(engine.is_plugin_allowed("unknown.plugin", Some("trusted-key-abc")));
    assert!(!engine.is_plugin_allowed("unknown.plugin", Some("wrong-key")));
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
    assert!(engine.is_plugin_allowed("known.plugin", None));
    assert!(engine.is_plugin_allowed("other.plugin", Some("trusted-key")));
    assert!(engine.is_plugin_allowed("known.plugin", Some("trusted-key")));
    assert!(!engine.is_plugin_allowed("other.plugin", None));
}

#[test]
fn load_from_missing_file_is_unrestricted() {
    let dir = tempfile::tempdir().unwrap();
    let fake_path = dir.path().join("nonexistent.toml");
    let engine = PolicyEngine::load_from(fake_path);
    assert!(!engine.has_policy_file());
    assert!(engine.is_plugin_allowed("anything", None));
    assert!(!engine.is_permission_denied_by_policy(Permission::Network));
}

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
    assert!(engine.is_plugin_allowed("privstack.notes", None));
    assert!(engine.is_plugin_allowed("privstack.tasks", None));
    assert!(engine.is_plugin_allowed("unknown.plugin", Some("official-signing-key")));
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
    let dir = tempfile::tempdir().unwrap();
    let engine = PolicyEngine::load_from(dir.path().to_path_buf());
    assert!(engine.is_plugin_allowed("anything", None));
}

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
        audit: privstack_plugin_host::AuditConfig {
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

#[test]
fn denied_permissions_all_variants_via_load() {
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
fn empty_policy_section_defaults_to_unrestricted_via_load() {
    let engine = load_policy_from_str("[policy]\n");
    assert_eq!(engine.config().mode, PolicyMode::Unrestricted);
    assert!(engine.config().allowed_plugin_ids.is_empty());
    assert!(engine.config().denied_permissions.is_empty());
}

#[test]
fn completely_empty_toml_uses_default_via_load() {
    let engine = load_policy_from_str("");
    // When the entire TOML is empty, PolicySection::default() is used,
    // which derives Default for PolicyMode (Allowlist).
    assert_eq!(engine.config().mode, PolicyMode::Allowlist);
}
