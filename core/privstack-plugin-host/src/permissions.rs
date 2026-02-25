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
