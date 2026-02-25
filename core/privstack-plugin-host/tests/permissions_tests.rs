use privstack_plugin_host::*;

#[test]
fn default_first_party_has_tier1() {
    let perms = PermissionSet::default_first_party();
    assert!(perms.is_granted(Permission::Sdk));
    assert!(perms.is_granted(Permission::Settings));
    assert!(perms.is_granted(Permission::Logger));
    assert!(perms.is_granted(Permission::Navigation));
    assert!(perms.is_granted(Permission::StateNotify));
    assert!(!perms.is_granted(Permission::Vault));
    assert!(!perms.is_granted(Permission::Linking));
    assert!(!perms.is_granted(Permission::Agent));
}

#[test]
fn grant_and_deny() {
    let mut perms = PermissionSet::default_first_party();
    assert!(!perms.is_granted(Permission::Vault));
    perms.grant(Permission::Vault);
    assert!(perms.is_granted(Permission::Vault));
    perms.deny(Permission::Vault);
    assert!(!perms.is_granted(Permission::Vault));
    assert!(perms.is_denied(Permission::Vault));
}

#[test]
fn jit_prompt_needed_for_tier2() {
    let perms = PermissionSet::default_first_party();
    assert!(perms.needs_jit_prompt(Permission::Vault));
    assert!(perms.needs_jit_prompt(Permission::Linking));
    assert!(!perms.needs_jit_prompt(Permission::Sdk));
    assert!(!perms.needs_jit_prompt(Permission::Agent));
}

#[test]
fn all_granted_has_everything() {
    let perms = PermissionSet::all_granted();
    assert!(perms.is_granted(Permission::Sdk));
    assert!(perms.is_granted(Permission::Vault));
    assert!(perms.is_granted(Permission::Agent));
    assert!(perms.is_granted(Permission::Filesystem));
}

#[test]
fn permission_tiers() {
    assert_eq!(Permission::Sdk.tier(), PermissionTier::AlwaysGranted);
    assert_eq!(Permission::Vault.tier(), PermissionTier::JustInTime);
    assert_eq!(Permission::Agent.tier(), PermissionTier::InstallTime);
}

#[test]
fn all_permission_interface_names() {
    assert_eq!(Permission::Sdk.interface_name(), "sdk");
    assert_eq!(Permission::Settings.interface_name(), "settings");
    assert_eq!(Permission::Logger.interface_name(), "logger");
    assert_eq!(Permission::Navigation.interface_name(), "navigation");
    assert_eq!(Permission::StateNotify.interface_name(), "state-notify");
    assert_eq!(Permission::Linking.interface_name(), "linking");
    assert_eq!(Permission::Dialogs.interface_name(), "dialogs");
    assert_eq!(Permission::Vault.interface_name(), "vault");
    assert_eq!(Permission::CrossEntityRead.interface_name(), "cross-entity-read");
    assert_eq!(Permission::CrossPluginCommand.interface_name(), "cross-plugin-command");
    assert_eq!(Permission::Filesystem.interface_name(), "filesystem");
    assert_eq!(Permission::Network.interface_name(), "network");
    assert_eq!(Permission::Agent.interface_name(), "agent");
}

#[test]
fn all_tier1_permissions() {
    assert_eq!(Permission::Settings.tier(), PermissionTier::AlwaysGranted);
    assert_eq!(Permission::Logger.tier(), PermissionTier::AlwaysGranted);
    assert_eq!(Permission::Navigation.tier(), PermissionTier::AlwaysGranted);
    assert_eq!(Permission::StateNotify.tier(), PermissionTier::AlwaysGranted);
}

#[test]
fn all_tier2_permissions() {
    assert_eq!(Permission::Linking.tier(), PermissionTier::JustInTime);
    assert_eq!(Permission::Dialogs.tier(), PermissionTier::JustInTime);
    assert_eq!(Permission::CrossEntityRead.tier(), PermissionTier::JustInTime);
    assert_eq!(Permission::CrossPluginCommand.tier(), PermissionTier::JustInTime);
}

#[test]
fn all_tier3_permissions() {
    assert_eq!(Permission::Filesystem.tier(), PermissionTier::InstallTime);
    assert_eq!(Permission::Network.tier(), PermissionTier::InstallTime);
}

#[test]
fn default_third_party_matches_first_party() {
    let fp = PermissionSet::default_first_party();
    let tp = PermissionSet::default_third_party();
    assert_eq!(fp.is_granted(Permission::Sdk), tp.is_granted(Permission::Sdk));
    assert_eq!(fp.is_granted(Permission::Vault), tp.is_granted(Permission::Vault));
}

#[test]
fn granted_permissions_returns_all_granted() {
    let perms = PermissionSet::all_granted();
    let granted = perms.granted_permissions();
    assert!(granted.contains(&Permission::Sdk));
    assert!(granted.contains(&Permission::Vault));
    assert!(granted.contains(&Permission::Agent));
    assert_eq!(granted.len(), 13);
}

#[test]
fn granted_permissions_first_party_has_five() {
    let perms = PermissionSet::default_first_party();
    assert_eq!(perms.granted_permissions().len(), 5);
}

#[test]
fn jit_prompt_not_needed_after_grant() {
    let mut perms = PermissionSet::default_first_party();
    assert!(perms.needs_jit_prompt(Permission::Vault));
    perms.grant(Permission::Vault);
    assert!(!perms.needs_jit_prompt(Permission::Vault));
}

#[test]
fn jit_prompt_not_needed_after_deny() {
    let mut perms = PermissionSet::default_first_party();
    assert!(perms.needs_jit_prompt(Permission::Linking));
    perms.deny(Permission::Linking);
    assert!(!perms.needs_jit_prompt(Permission::Linking));
}

#[test]
fn permission_set_debug() {
    let perms = PermissionSet::default_first_party();
    let debug_str = format!("{:?}", perms);
    assert!(debug_str.contains("PermissionSet"));
}

#[test]
fn permission_set_clone() {
    let perms = PermissionSet::all_granted();
    let cloned = perms.clone();
    assert_eq!(
        cloned.granted_permissions().len(),
        perms.granted_permissions().len()
    );
}

#[test]
fn permission_debug() {
    let perm = Permission::Vault;
    let debug_str = format!("{:?}", perm);
    assert!(debug_str.contains("Vault"));
}
