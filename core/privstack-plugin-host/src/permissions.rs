//! Capability-based permission model for plugin sandboxes.
//!
//! Four tiers:
//! - Tier 1: Always granted (sdk, settings, logger, navigation)
//! - Tier 2: Just-in-time prompted (linking, dialogs, vault)
//! - Tier 3: Install-time reviewed (filesystem, network, agent)
//! - Tier 4: Enterprise policy overrides

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Individual permission a plugin may hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Permission {
    // Tier 1 — always granted
    Sdk,
    Settings,
    Logger,
    Navigation,
    StateNotify,

    // Tier 2 — just-in-time
    Linking,
    Dialogs,
    Vault,
    CrossEntityRead,
    CrossPluginCommand,

    // Tier 3 — install-time
    Filesystem,
    Network,
    Agent,
}

impl Permission {
    /// Returns the tier for this permission.
    pub fn tier(&self) -> PermissionTier {
        match self {
            Self::Sdk | Self::Settings | Self::Logger | Self::Navigation | Self::StateNotify => {
                PermissionTier::AlwaysGranted
            }
            Self::Linking | Self::Dialogs | Self::Vault | Self::CrossEntityRead | Self::CrossPluginCommand => {
                PermissionTier::JustInTime
            }
            Self::Filesystem | Self::Network | Self::Agent => PermissionTier::InstallTime,
        }
    }

    /// Returns the WIT interface name this permission gates.
    pub fn interface_name(&self) -> &'static str {
        match self {
            Self::Sdk => "sdk",
            Self::Settings => "settings",
            Self::Logger => "logger",
            Self::Navigation => "navigation",
            Self::StateNotify => "state-notify",
            Self::Linking => "linking",
            Self::Dialogs => "dialogs",
            Self::Vault => "vault",
            Self::CrossEntityRead => "cross-entity-read",
            Self::CrossPluginCommand => "cross-plugin-command",
            Self::Filesystem => "filesystem",
            Self::Network => "network",
            Self::Agent => "agent",
        }
    }
}

/// Permission tier classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionTier {
    AlwaysGranted,
    JustInTime,
    InstallTime,
}

/// Set of permissions granted to a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionSet {
    granted: HashSet<Permission>,
    /// Permissions that were denied by user or policy (never prompt again).
    denied: HashSet<Permission>,
    /// Permissions awaiting JIT decision (not yet prompted).
    pending_jit: HashSet<Permission>,
}

impl PermissionSet {
    /// Creates a default set with Tier 1 permissions granted.
    pub fn default_first_party() -> Self {
        let mut granted = HashSet::new();
        granted.insert(Permission::Sdk);
        granted.insert(Permission::Settings);
        granted.insert(Permission::Logger);
        granted.insert(Permission::Navigation);
        granted.insert(Permission::StateNotify);

        Self {
            granted,
            denied: HashSet::new(),
            pending_jit: HashSet::new(),
        }
    }

    /// Creates a set with only Tier 1 permissions for third-party plugins.
    pub fn default_third_party() -> Self {
        Self::default_first_party()
    }

    /// Creates a set with all permissions granted (for testing).
    pub fn all_granted() -> Self {
        let granted: HashSet<Permission> = [
            Permission::Sdk,
            Permission::Settings,
            Permission::Logger,
            Permission::Navigation,
            Permission::StateNotify,
            Permission::Linking,
            Permission::Dialogs,
            Permission::Vault,
            Permission::CrossEntityRead,
            Permission::CrossPluginCommand,
            Permission::Filesystem,
            Permission::Network,
            Permission::Agent,
        ]
        .into_iter()
        .collect();

        Self {
            granted,
            denied: HashSet::new(),
            pending_jit: HashSet::new(),
        }
    }

    pub fn is_granted(&self, permission: Permission) -> bool {
        self.granted.contains(&permission)
    }

    pub fn is_denied(&self, permission: Permission) -> bool {
        self.denied.contains(&permission)
    }

    pub fn grant(&mut self, permission: Permission) {
        self.denied.remove(&permission);
        self.pending_jit.remove(&permission);
        self.granted.insert(permission);
    }

    pub fn deny(&mut self, permission: Permission) {
        self.granted.remove(&permission);
        self.pending_jit.remove(&permission);
        self.denied.insert(permission);
    }

    /// Check if a permission needs JIT prompting (Tier 2, not yet decided).
    pub fn needs_jit_prompt(&self, permission: Permission) -> bool {
        permission.tier() == PermissionTier::JustInTime
            && !self.granted.contains(&permission)
            && !self.denied.contains(&permission)
    }

    /// Returns all granted permissions.
    pub fn granted_permissions(&self) -> &HashSet<Permission> {
        &self.granted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(!perms.needs_jit_prompt(Permission::Sdk)); // Tier 1, already granted
        assert!(!perms.needs_jit_prompt(Permission::Agent)); // Tier 3, not JIT
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

    // ================================================================
    // interface_name for all permissions
    // ================================================================

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

    // ================================================================
    // All tier classifications
    // ================================================================

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

    // ================================================================
    // default_third_party same as first_party
    // ================================================================

    #[test]
    fn default_third_party_matches_first_party() {
        let fp = PermissionSet::default_first_party();
        let tp = PermissionSet::default_third_party();
        assert_eq!(fp.is_granted(Permission::Sdk), tp.is_granted(Permission::Sdk));
        assert_eq!(fp.is_granted(Permission::Vault), tp.is_granted(Permission::Vault));
    }

    // ================================================================
    // granted_permissions accessor
    // ================================================================

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

    // ================================================================
    // JIT prompt edge cases
    // ================================================================

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

    // ================================================================
    // Debug and Clone traits
    // ================================================================

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
}
