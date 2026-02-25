//! SDK types for plugin authors. Mirrors WIT types with ergonomic Rust APIs.

use serde::{Deserialize, Serialize};

// ---- Plugin Metadata ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub icon: Option<String>,
    pub navigation_order: u32,
    pub category: PluginCategory,
    pub can_disable: bool,
    pub is_experimental: bool,
    /// Non-Tier-1 capabilities this plugin requests (e.g. "filesystem", "dialogs", "vault").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
}

impl Default for PluginMetadata {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            version: "0.1.0".into(),
            author: String::new(),
            icon: None,
            navigation_order: 1000,
            category: PluginCategory::Extension,
            can_disable: true,
            is_experimental: false,
            capabilities: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginCategory {
    Productivity,
    Security,
    Communication,
    Information,
    Utility,
    Extension,
}

// ---- Navigation ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationItem {
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

impl Default for NavigationItem {
    fn default() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            subtitle: None,
            icon: None,
            tooltip: None,
            order: 1000,
            show_badge: false,
            badge_count: 0,
            shortcut_hint: None,
        }
    }
}

// ---- Entity Schema ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySchema {
    pub entity_type: String,
    pub indexed_fields: Vec<IndexedField>,
    pub merge_strategy: MergeStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedField {
    pub field_path: String,
    pub field_type: FieldType,
    pub searchable: bool,
    pub vector_dim: Option<u16>,
    pub enum_options: Option<Vec<String>>,
}

impl IndexedField {
    /// Searchable text field.
    pub fn text(path: &str, searchable: bool) -> Self {
        Self {
            field_path: path.into(),
            field_type: FieldType::Text,
            searchable,
            vector_dim: None,
            enum_options: None,
        }
    }

    /// Tag array field (always searchable).
    pub fn tag(path: &str) -> Self {
        Self {
            field_path: path.into(),
            field_type: FieldType::Tag,
            searchable: true,
            vector_dim: None,
            enum_options: None,
        }
    }

    /// DateTime field.
    pub fn datetime(path: &str) -> Self {
        Self {
            field_path: path.into(),
            field_type: FieldType::DateTime,
            searchable: false,
            vector_dim: None,
            enum_options: None,
        }
    }

    /// Numeric field.
    pub fn number(path: &str) -> Self {
        Self {
            field_path: path.into(),
            field_type: FieldType::Number,
            searchable: false,
            vector_dim: None,
            enum_options: None,
        }
    }

    /// Boolean field.
    pub fn boolean(path: &str) -> Self {
        Self {
            field_path: path.into(),
            field_type: FieldType::Boolean,
            searchable: false,
            vector_dim: None,
            enum_options: None,
        }
    }

    /// Vector/embedding field with specified dimensions.
    pub fn vector(path: &str, dim: u16) -> Self {
        Self {
            field_path: path.into(),
            field_type: FieldType::Vector,
            searchable: false,
            vector_dim: Some(dim),
            enum_options: None,
        }
    }

    /// CRDT counter field.
    pub fn counter(path: &str) -> Self {
        Self {
            field_path: path.into(),
            field_type: FieldType::Counter,
            searchable: false,
            vector_dim: None,
            enum_options: None,
        }
    }

    /// Relation (entity link) field.
    pub fn relation(path: &str) -> Self {
        Self {
            field_path: path.into(),
            field_type: FieldType::Relation,
            searchable: false,
            vector_dim: None,
            enum_options: None,
        }
    }

    /// Decimal field.
    pub fn decimal(path: &str) -> Self {
        Self {
            field_path: path.into(),
            field_type: FieldType::Decimal,
            searchable: false,
            vector_dim: None,
            enum_options: None,
        }
    }

    /// JSON blob field.
    pub fn json(path: &str) -> Self {
        Self {
            field_path: path.into(),
            field_type: FieldType::Json,
            searchable: false,
            vector_dim: None,
            enum_options: None,
        }
    }

    /// Enumeration field with fixed options.
    pub fn enumeration(path: &str, options: Vec<String>) -> Self {
        Self {
            field_path: path.into(),
            field_type: FieldType::Enumeration,
            searchable: false,
            vector_dim: None,
            enum_options: Some(options),
        }
    }

    /// Geographic point field.
    pub fn geo_point(path: &str) -> Self {
        Self {
            field_path: path.into(),
            field_type: FieldType::GeoPoint,
            searchable: false,
            vector_dim: None,
            enum_options: None,
        }
    }

    /// Duration field.
    pub fn duration(path: &str) -> Self {
        Self {
            field_path: path.into(),
            field_type: FieldType::Duration,
            searchable: false,
            vector_dim: None,
            enum_options: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
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
pub enum MergeStrategy {
    LwwDocument,
    LwwPerField,
    Custom,
}

// ---- SDK Message / Response ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkMessage {
    pub action: SdkAction,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub payload: Option<String>,
    pub parameters: Vec<(String, String)>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SdkAction {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkResponse {
    pub success: bool,
    pub error_code: Option<u32>,
    pub error_message: Option<String>,
    pub data: Option<String>,
}

impl SdkResponse {
    pub fn is_ok(&self) -> bool {
        self.success
    }

    /// Parse the data field as a typed value.
    pub fn parse_data<T: serde::de::DeserializeOwned>(&self) -> Option<T> {
        self.data
            .as_ref()
            .and_then(|d| serde_json::from_str(d).ok())
    }
}

// ---- Linkable Item ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkableItem {
    pub id: String,
    pub link_type: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub modified_at: u64,
}

// ---- Command Definition ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDefinition {
    pub name: String,
    pub description: String,
    pub keywords: String,
    pub category: String,
    pub icon: Option<String>,
}

// ---- Timer ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerState {
    pub is_active: bool,
    pub is_running: bool,
    pub elapsed_ms: u64,
    pub item_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerResult {
    pub item_id: String,
    pub elapsed_ms: u64,
}

// ---- Plugin Trait ----

/// Core plugin contract. Every plugin must implement this.
pub trait Plugin {
    fn metadata(&self) -> PluginMetadata;
    fn entity_schemas(&self) -> Vec<EntitySchema>;
    fn navigation_item(&self) -> Option<NavigationItem> {
        None
    }
    fn commands(&self) -> Vec<CommandDefinition> {
        Vec::new()
    }

    fn initialize(&mut self) -> bool;
    fn activate(&mut self) {}
    fn deactivate(&mut self) {}
    fn on_navigated_to(&mut self) {}
    fn on_navigated_from(&mut self) {}
    fn dispose(&mut self) {}

    fn get_view_state(&self) -> String {
        "{}".to_string()
    }
    fn handle_command(&mut self, _name: &str, _args: &str) -> String {
        "{}".to_string()
    }
}

/// Optional: plugin provides linkable items for cross-plugin references.
pub trait LinkableItemProvider {
    fn link_type(&self) -> &str;
    fn link_type_display_name(&self) -> &str;
    fn link_type_icon(&self) -> &str;
    fn search_items(&self, query: &str, max_results: u32) -> Vec<LinkableItem>;
    fn get_item_by_id(&self, id: &str) -> Option<LinkableItem>;
}

/// Optional: plugin can be navigated to via deep links.
pub trait DeepLinkTarget {
    fn link_type(&self) -> &str;
    fn navigate_to_item(&mut self, item_id: &str);
}

/// Optional: plugin has timer functionality.
pub trait TimerBehavior {
    fn start_timer(&mut self, item_id: &str);
    fn pause_timer(&mut self);
    fn resume_timer(&mut self);
    fn stop_timer(&mut self) -> TimerResult;
    fn get_timer_state(&self) -> TimerState;
}

/// Optional: plugin supports shutdown-aware cleanup.
pub trait ShutdownAware {
    fn on_shutdown(&mut self);
}

/// Optional: plugin provides raw view data for host-side template evaluation.
/// Plugins that ship a `template.json` sidecar implement this instead of
/// building the component tree in `get_view_state()`.
pub trait TemplateDataProvider {
    fn get_view_data(&self) -> String {
        "{}".to_string()
    }
}

