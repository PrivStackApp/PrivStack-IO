//! Plugin manifest (manifest.toml) within a .ppk package.

use serde::{Deserialize, Serialize};

/// Top-level plugin manifest embedded in every .ppk package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PpkManifest {
    /// Unique plugin identifier (e.g., "privstack.rss").
    pub id: String,
    /// Human-readable plugin name.
    pub name: String,
    /// Short description.
    pub description: String,
    /// Semver version string.
    pub version: String,
    /// Author or organization.
    pub author: String,
    /// Lucide icon name (optional).
    pub icon: Option<String>,
    /// Navigation sidebar order (100-199 primary, 200-299 secondary, 300-399 utility, 1000+ third-party).
    pub navigation_order: u32,
    /// Plugin category.
    pub category: String,
    /// Whether the user can disable this plugin.
    pub can_disable: bool,
    /// Whether this is an experimental feature.
    pub is_experimental: bool,
    /// Minimum app version required (semver).
    pub min_app_version: Option<String>,
    /// Permissions requested by this plugin.
    #[serde(default)]
    pub permissions: Vec<PpkPermission>,
    /// Entity schemas declared by this plugin.
    #[serde(default)]
    pub schemas: Vec<PpkEntitySchema>,
}

/// Capability permission that a plugin can request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PpkPermission {
    /// Tier 1: CRUD on declared entity types.
    EntityCrud,
    /// Tier 1: Query own entity types.
    EntityQuery,
    /// Tier 1: Export view state JSON.
    ViewState,
    /// Tier 1: Register commands in the command palette.
    CommandPalette,
    /// Tier 2: Access encrypted vault storage (JIT prompt).
    VaultAccess,
    /// Tier 2: Link items across plugins (JIT prompt).
    CrossPluginLink,
    /// Tier 2: Show modal dialogs (JIT prompt).
    DialogDisplay,
    /// Tier 2: Access timer/pomodoro APIs (JIT prompt).
    TimerAccess,
    /// Tier 3: Network access (admin approval required).
    NetworkAccess,
}

/// Entity schema declared in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PpkEntitySchema {
    /// Entity type name (e.g., "feed", "note").
    pub entity_type: String,
    /// Indexed fields for search and filtering.
    pub indexed_fields: Vec<PpkIndexedField>,
    /// Conflict resolution strategy.
    pub merge_strategy: String,
}

/// A field to be indexed for search/filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PpkIndexedField {
    /// JSON pointer path (e.g., "/title").
    pub field_path: String,
    /// Field data type (text, tag, date_time, number, bool, vector, etc.).
    pub field_type: String,
    /// Whether this field is included in full-text search.
    pub searchable: bool,
}

impl PpkManifest {
    /// Validates the manifest for required fields and constraints.
    pub fn validate(&self) -> Result<(), crate::PpkError> {
        if self.id.is_empty() {
            return Err(crate::PpkError::ManifestInvalid("id is required".into()));
        }
        if self.name.is_empty() {
            return Err(crate::PpkError::ManifestInvalid("name is required".into()));
        }
        if self.version.is_empty() {
            return Err(crate::PpkError::ManifestInvalid("version is required".into()));
        }
        // Plugin IDs should follow reverse-domain convention
        if !self.id.contains('.') {
            return Err(crate::PpkError::ManifestInvalid(
                "id must use reverse-domain format (e.g., 'privstack.rss')".into(),
            ));
        }
        Ok(())
    }

    /// Returns true if this is a first-party PrivStack plugin.
    pub fn is_first_party(&self) -> bool {
        self.id.starts_with("privstack.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_valid_manifest() {
        let m = PpkManifest {
            id: "privstack.test".into(),
            name: "Test".into(),
            description: "".into(),
            version: "1.0.0".into(),
            author: "PrivStack".into(),
            icon: None,
            navigation_order: 100,
            category: "utility".into(),
            can_disable: true,
            is_experimental: false,
            min_app_version: None,
            permissions: vec![],
            schemas: vec![],
        };
        assert!(m.validate().is_ok());
    }

    #[test]
    fn validate_empty_id() {
        let m = PpkManifest {
            id: "".into(),
            name: "Test".into(),
            description: "".into(),
            version: "1.0.0".into(),
            author: "".into(),
            icon: None,
            navigation_order: 100,
            category: "utility".into(),
            can_disable: true,
            is_experimental: false,
            min_app_version: None,
            permissions: vec![],
            schemas: vec![],
        };
        assert!(m.validate().is_err());
    }

    #[test]
    fn validate_no_dot_in_id() {
        let m = PpkManifest {
            id: "invalid".into(),
            name: "Test".into(),
            description: "".into(),
            version: "1.0.0".into(),
            author: "".into(),
            icon: None,
            navigation_order: 100,
            category: "utility".into(),
            can_disable: true,
            is_experimental: false,
            min_app_version: None,
            permissions: vec![],
            schemas: vec![],
        };
        assert!(m.validate().is_err());
    }

    #[test]
    fn is_first_party() {
        let m = PpkManifest {
            id: "privstack.rss".into(),
            name: "RSS".into(),
            description: "".into(),
            version: "1.0.0".into(),
            author: "PrivStack".into(),
            icon: None,
            navigation_order: 100,
            category: "utility".into(),
            can_disable: true,
            is_experimental: false,
            min_app_version: None,
            permissions: vec![],
            schemas: vec![],
        };
        assert!(m.is_first_party());

        let m2 = PpkManifest {
            id: "community.weather".into(),
            ..m
        };
        assert!(!m2.is_first_party());
    }
}
