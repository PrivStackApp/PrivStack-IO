//! Rust representations of WIT types, used for serialization across
//! the Wasm boundary. These mirror the WIT definitions in `wit/types.wit`.

use serde::{Deserialize, Serialize};

/// Plugin metadata returned by `get_metadata()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitPluginMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub icon: Option<String>,
    pub navigation_order: u32,
    pub category: WitPluginCategory,
    pub can_disable: bool,
    pub is_experimental: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WitPluginCategory {
    Productivity,
    Security,
    Communication,
    Information,
    Utility,
    Extension,
}

/// Navigation item for sidebar display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitNavigationItem {
    pub id: String,
    pub display_name: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub order: u32,
    pub show_badge: bool,
    pub badge_count: u32,
    pub shortcut_hint: Option<String>,
}

/// Entity schema for storage indexing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitEntitySchema {
    pub entity_type: String,
    pub indexed_fields: Vec<WitIndexedField>,
    pub merge_strategy: WitMergeStrategy,
}

/// Indexed field with optional extension fields for parameterized types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitIndexedField {
    pub field_path: String,
    pub field_type: WitFieldType,
    pub searchable: bool,
    pub vector_dim: Option<u16>,
    pub enum_options: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WitFieldType {
    Text,
    Tag,
    DateTime,
    Number,
    Boolean,
    Vector,
    Counter,
    Relation,
    Decimal,
    Json,
    Enumeration,
    GeoPoint,
    Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WitMergeStrategy {
    LwwDocument,
    LwwPerField,
    Custom,
}

/// SDK message sent from plugin to host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitSdkMessage {
    pub action: WitSdkAction,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub payload: Option<String>,
    pub parameters: Vec<(String, String)>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WitSdkAction {
    Create,
    Read,
    Update,
    Delete,
    List,
    Query,
    Trash,
    Restore,
    Link,
    Unlink,
    GetLinks,
    SemanticSearch,
}

/// SDK response returned from host to plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitSdkResponse {
    pub success: bool,
    pub error_code: Option<u32>,
    pub error_message: Option<String>,
    pub data: Option<String>,
}

impl WitSdkResponse {
    pub fn ok(data: Option<String>) -> Self {
        Self {
            success: true,
            error_code: None,
            error_message: None,
            data,
        }
    }

    pub fn err(code: u32, message: impl Into<String>) -> Self {
        Self {
            success: false,
            error_code: Some(code),
            error_message: Some(message.into()),
            data: None,
        }
    }

    pub fn permission_denied(permission: &str) -> Self {
        Self::err(403, format!("permission denied: {permission}"))
    }
}

/// Linkable item for cross-plugin references.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitLinkableItem {
    pub id: String,
    pub link_type: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub modified_at: u64,
    /// Source plugin ID — stamped by the host after cross-plugin search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
}

/// Command definition for command palette.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitCommandDefinition {
    pub name: String,
    pub description: String,
    pub keywords: String,
    pub category: String,
    pub icon: Option<String>,
}

/// Link provider info for capability discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitLinkProviderInfo {
    pub plugin_id: String,
    pub link_type: String,
    pub display_name: String,
    pub icon: Option<String>,
}

/// Timer state for timer-capable plugins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitTimerState {
    pub is_active: bool,
    pub is_running: bool,
    pub elapsed_ms: u64,
    pub item_title: Option<String>,
}

/// Timer result returned when a timer is stopped.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitTimerResult {
    pub item_id: String,
    pub elapsed_ms: u64,
}

// ---- Conversion from WIT types to core model types ----

impl WitEntitySchema {
    /// Convert to the core `EntitySchema` used by storage.
    pub fn to_core_schema(&self) -> Result<privstack_model::EntitySchema, String> {
        let indexed_fields = self
            .indexed_fields
            .iter()
            .map(|f| f.to_core_field())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(privstack_model::EntitySchema {
            entity_type: self.entity_type.clone(),
            indexed_fields,
            merge_strategy: self.merge_strategy.to_core(),
        })
    }
}

impl WitIndexedField {
    pub fn to_core_field(&self) -> Result<privstack_model::IndexedField, String> {
        let field_type = match self.field_type {
            WitFieldType::Text => privstack_model::FieldType::Text,
            WitFieldType::Tag => privstack_model::FieldType::Tag,
            WitFieldType::DateTime => privstack_model::FieldType::DateTime,
            WitFieldType::Number => privstack_model::FieldType::Number,
            WitFieldType::Boolean => privstack_model::FieldType::Bool,
            WitFieldType::Vector => privstack_model::FieldType::Vector,
            WitFieldType::Counter => privstack_model::FieldType::Counter,
            WitFieldType::Relation => privstack_model::FieldType::Relation,
            WitFieldType::Decimal => privstack_model::FieldType::Decimal,
            WitFieldType::Json => privstack_model::FieldType::Json,
            WitFieldType::Enumeration => privstack_model::FieldType::Enum,
            WitFieldType::GeoPoint => privstack_model::FieldType::GeoPoint,
            WitFieldType::Duration => privstack_model::FieldType::Duration,
        };

        Ok(privstack_model::IndexedField {
            field_path: self.field_path.clone(),
            field_type,
            searchable: self.searchable,
            vector_dim: self.vector_dim,
            enum_options: self.enum_options.clone(),
        })
    }
}

impl WitMergeStrategy {
    pub fn to_core(&self) -> privstack_model::MergeStrategy {
        match self {
            Self::LwwDocument => privstack_model::MergeStrategy::LwwDocument,
            Self::LwwPerField => privstack_model::MergeStrategy::LwwPerField,
            Self::Custom => privstack_model::MergeStrategy::Custom,
        }
    }
}
