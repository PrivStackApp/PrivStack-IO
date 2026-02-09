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

#[cfg(test)]
mod tests {
    use super::*;

    // ── PluginMetadata ──────────────────────────────────────────────

    #[test]
    fn default_metadata() {
        let meta = PluginMetadata::default();
        assert_eq!(meta.id, "");
        assert_eq!(meta.name, "");
        assert_eq!(meta.description, "");
        assert_eq!(meta.version, "0.1.0");
        assert_eq!(meta.author, "");
        assert!(meta.icon.is_none());
        assert_eq!(meta.navigation_order, 1000);
        assert_eq!(meta.category, PluginCategory::Extension);
        assert!(meta.can_disable);
        assert!(!meta.is_experimental);
    }

    #[test]
    fn metadata_serde_roundtrip() {
        let meta = PluginMetadata {
            id: "test-plugin".into(),
            name: "Test".into(),
            description: "A test plugin".into(),
            version: "1.0.0".into(),
            author: "Author".into(),
            icon: Some("icon.png".into()),
            navigation_order: 100,
            category: PluginCategory::Productivity,
            can_disable: false,
            is_experimental: true,
            capabilities: vec!["filesystem".into(), "vault".into()],
        };
        let json = serde_json::to_string(&meta).unwrap();
        let deser: PluginMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.id, "test-plugin");
        assert_eq!(deser.category, PluginCategory::Productivity);
        assert!(!deser.can_disable);
        assert!(deser.is_experimental);
        assert_eq!(deser.capabilities, vec!["filesystem", "vault"]);
    }

    // ── PluginCategory ──────────────────────────────────────────────

    #[test]
    fn plugin_category_all_variants_serde() {
        let variants = [
            PluginCategory::Productivity,
            PluginCategory::Security,
            PluginCategory::Communication,
            PluginCategory::Information,
            PluginCategory::Utility,
            PluginCategory::Extension,
        ];
        for cat in &variants {
            let json = serde_json::to_string(cat).unwrap();
            let deser: PluginCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(*cat, deser);
        }
    }

    // ── NavigationItem ──────────────────────────────────────────────

    #[test]
    fn navigation_item_default() {
        let nav = NavigationItem::default();
        assert_eq!(nav.id, "");
        assert_eq!(nav.display_name, "");
        assert!(nav.subtitle.is_none());
        assert!(nav.icon.is_none());
        assert!(nav.tooltip.is_none());
        assert_eq!(nav.order, 1000);
        assert!(!nav.show_badge);
        assert_eq!(nav.badge_count, 0);
        assert!(nav.shortcut_hint.is_none());
    }

    #[test]
    fn navigation_item_serde_roundtrip() {
        let nav = NavigationItem {
            id: "nav-1".into(),
            display_name: "Notes".into(),
            subtitle: Some("Your notes".into()),
            icon: Some("notes-icon".into()),
            tooltip: Some("Open notes".into()),
            order: 100,
            show_badge: true,
            badge_count: 5,
            shortcut_hint: Some("Ctrl+N".into()),
        };
        let json = serde_json::to_string(&nav).unwrap();
        let deser: NavigationItem = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.id, "nav-1");
        assert_eq!(deser.badge_count, 5);
        assert!(deser.show_badge);
    }

    // ── IndexedField constructors ───────────────────────────────────

    #[test]
    fn indexed_field_constructors() {
        let text = IndexedField::text("/title", true);
        assert_eq!(text.field_type, FieldType::Text);
        assert!(text.searchable);
        assert!(text.vector_dim.is_none());
        assert!(text.enum_options.is_none());

        let tag = IndexedField::tag("/tags");
        assert_eq!(tag.field_type, FieldType::Tag);
        assert!(tag.searchable);

        let vec = IndexedField::vector("/embedding", 384);
        assert_eq!(vec.field_type, FieldType::Vector);
        assert_eq!(vec.vector_dim, Some(384));

        let enm = IndexedField::enumeration("/status", vec!["open".into(), "closed".into()]);
        assert_eq!(enm.field_type, FieldType::Enumeration);
        assert_eq!(enm.enum_options.unwrap().len(), 2);
    }

    #[test]
    fn indexed_field_remaining_constructors() {
        let dt = IndexedField::datetime("/created");
        assert_eq!(dt.field_type, FieldType::DateTime);
        assert!(!dt.searchable);

        let num = IndexedField::number("/count");
        assert_eq!(num.field_type, FieldType::Number);

        let b = IndexedField::boolean("/done");
        assert_eq!(b.field_type, FieldType::Boolean);

        let ctr = IndexedField::counter("/views");
        assert_eq!(ctr.field_type, FieldType::Counter);

        let rel = IndexedField::relation("/parent");
        assert_eq!(rel.field_type, FieldType::Relation);

        let dec = IndexedField::decimal("/price");
        assert_eq!(dec.field_type, FieldType::Decimal);

        let js = IndexedField::json("/meta");
        assert_eq!(js.field_type, FieldType::Json);

        let geo = IndexedField::geo_point("/location");
        assert_eq!(geo.field_type, FieldType::GeoPoint);

        let dur = IndexedField::duration("/elapsed");
        assert_eq!(dur.field_type, FieldType::Duration);
    }

    #[test]
    fn indexed_field_text_not_searchable() {
        let f = IndexedField::text("/body", false);
        assert!(!f.searchable);
    }

    #[test]
    fn indexed_field_serde_roundtrip() {
        let f = IndexedField::vector("/embed", 128);
        let json = serde_json::to_string(&f).unwrap();
        let deser: IndexedField = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.field_type, FieldType::Vector);
        assert_eq!(deser.vector_dim, Some(128));
    }

    // ── EntitySchema & MergeStrategy ────────────────────────────────

    #[test]
    fn entity_schema_serde_roundtrip() {
        let schema = EntitySchema {
            entity_type: "note".into(),
            indexed_fields: vec![IndexedField::text("/title", true)],
            merge_strategy: MergeStrategy::LwwPerField,
        };
        let json = serde_json::to_string(&schema).unwrap();
        let deser: EntitySchema = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.entity_type, "note");
        assert_eq!(deser.merge_strategy, MergeStrategy::LwwPerField);
        assert_eq!(deser.indexed_fields.len(), 1);
    }

    #[test]
    fn merge_strategy_all_variants_serde() {
        let variants = [
            MergeStrategy::LwwDocument,
            MergeStrategy::LwwPerField,
            MergeStrategy::Custom,
        ];
        for ms in &variants {
            let json = serde_json::to_string(ms).unwrap();
            let deser: MergeStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(*ms, deser);
        }
    }

    // ── SdkAction ───────────────────────────────────────────────────

    #[test]
    fn sdk_action_all_variants_serde() {
        let variants = [
            SdkAction::Create,
            SdkAction::Read,
            SdkAction::Update,
            SdkAction::Delete,
            SdkAction::List,
            SdkAction::Query,
            SdkAction::Trash,
            SdkAction::Restore,
            SdkAction::Link,
            SdkAction::Unlink,
            SdkAction::GetLinks,
            SdkAction::SemanticSearch,
        ];
        for action in &variants {
            let json = serde_json::to_string(action).unwrap();
            let deser: SdkAction = serde_json::from_str(&json).unwrap();
            assert_eq!(*action, deser);
        }
    }

    // ── SdkMessage ──────────────────────────────────────────────────

    #[test]
    fn sdk_message_construction_all_actions() {
        let actions = [
            SdkAction::Create, SdkAction::Read, SdkAction::Update,
            SdkAction::Delete, SdkAction::List, SdkAction::Query,
            SdkAction::Trash, SdkAction::Restore, SdkAction::Link,
            SdkAction::Unlink, SdkAction::GetLinks, SdkAction::SemanticSearch,
        ];
        for action in &actions {
            let msg = SdkMessage {
                action: *action,
                entity_type: "note".into(),
                entity_id: Some("abc-123".into()),
                payload: Some(r#"{"title":"hi"}"#.into()),
                parameters: vec![("limit".into(), "10".into())],
                source: Some("test-plugin".into()),
            };
            let json = serde_json::to_string(&msg).unwrap();
            let deser: SdkMessage = serde_json::from_str(&json).unwrap();
            assert_eq!(deser.action, *action);
            assert_eq!(deser.entity_type, "note");
            assert_eq!(deser.entity_id.as_deref(), Some("abc-123"));
        }
    }

    #[test]
    fn sdk_message_minimal() {
        let msg = SdkMessage {
            action: SdkAction::List,
            entity_type: "task".into(),
            entity_id: None,
            payload: None,
            parameters: vec![],
            source: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deser: SdkMessage = serde_json::from_str(&json).unwrap();
        assert!(deser.entity_id.is_none());
        assert!(deser.payload.is_none());
        assert!(deser.parameters.is_empty());
        assert!(deser.source.is_none());
    }

    // ── SdkResponse ─────────────────────────────────────────────────

    #[test]
    fn sdk_response_parse() {
        let resp = SdkResponse {
            success: true,
            error_code: None,
            error_message: None,
            data: Some(r#"{"count": 42}"#.into()),
        };
        assert!(resp.is_ok());

        #[derive(Deserialize)]
        struct Count {
            count: u32,
        }
        let parsed: Count = resp.parse_data().unwrap();
        assert_eq!(parsed.count, 42);
    }

    #[test]
    fn sdk_response_is_ok_false() {
        let resp = SdkResponse {
            success: false,
            error_code: Some(404),
            error_message: Some("not found".into()),
            data: None,
        };
        assert!(!resp.is_ok());
    }

    #[test]
    fn sdk_response_parse_data_none_when_no_data() {
        let resp = SdkResponse {
            success: true,
            error_code: None,
            error_message: None,
            data: None,
        };
        let parsed: Option<String> = resp.parse_data();
        assert!(parsed.is_none());
    }

    #[test]
    fn sdk_response_parse_data_malformed_json() {
        let resp = SdkResponse {
            success: true,
            error_code: None,
            error_message: None,
            data: Some("not valid json {{{".into()),
        };
        let parsed: Option<serde_json::Value> = resp.parse_data();
        assert!(parsed.is_none());
    }

    #[test]
    fn sdk_response_parse_data_wrong_type() {
        let resp = SdkResponse {
            success: true,
            error_code: None,
            error_message: None,
            data: Some(r#""a string""#.into()),
        };
        // Try to parse a string as a struct
        #[derive(Deserialize)]
        struct Foo { _x: u32 }
        let parsed: Option<Foo> = resp.parse_data();
        assert!(parsed.is_none());
    }

    #[test]
    fn sdk_response_serde_roundtrip() {
        let resp = SdkResponse {
            success: false,
            error_code: Some(500),
            error_message: Some("internal".into()),
            data: Some(r#"{"detail":"oops"}"#.into()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deser: SdkResponse = serde_json::from_str(&json).unwrap();
        assert!(!deser.is_ok());
        assert_eq!(deser.error_code, Some(500));
    }

    // ── LinkableItem ────────────────────────────────────────────────

    #[test]
    fn linkable_item_construction_and_serde() {
        let item = LinkableItem {
            id: "item-1".into(),
            link_type: "note".into(),
            title: "My Note".into(),
            subtitle: Some("A subtitle".into()),
            icon: Some("note-icon".into()),
            modified_at: 1700000000,
        };
        let json = serde_json::to_string(&item).unwrap();
        let deser: LinkableItem = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.id, "item-1");
        assert_eq!(deser.modified_at, 1700000000);
        assert_eq!(deser.subtitle.as_deref(), Some("A subtitle"));
    }

    #[test]
    fn linkable_item_no_optionals() {
        let item = LinkableItem {
            id: "x".into(),
            link_type: "task".into(),
            title: "T".into(),
            subtitle: None,
            icon: None,
            modified_at: 0,
        };
        let json = serde_json::to_string(&item).unwrap();
        let deser: LinkableItem = serde_json::from_str(&json).unwrap();
        assert!(deser.subtitle.is_none());
        assert!(deser.icon.is_none());
    }

    // ── CommandDefinition ───────────────────────────────────────────

    #[test]
    fn command_definition_construction_and_serde() {
        let cmd = CommandDefinition {
            name: "create-note".into(),
            description: "Create a new note".into(),
            keywords: "note new create".into(),
            category: "Notes".into(),
            icon: Some("plus".into()),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let deser: CommandDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.name, "create-note");
        assert_eq!(deser.icon.as_deref(), Some("plus"));
    }

    #[test]
    fn command_definition_no_icon() {
        let cmd = CommandDefinition {
            name: "cmd".into(),
            description: "d".into(),
            keywords: "k".into(),
            category: "c".into(),
            icon: None,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let deser: CommandDefinition = serde_json::from_str(&json).unwrap();
        assert!(deser.icon.is_none());
    }

    // ── TimerState ──────────────────────────────────────────────────

    #[test]
    fn timer_state_construction_and_serde() {
        let ts = TimerState {
            is_active: true,
            is_running: true,
            elapsed_ms: 5000,
            item_title: Some("Working on task".into()),
        };
        let json = serde_json::to_string(&ts).unwrap();
        let deser: TimerState = serde_json::from_str(&json).unwrap();
        assert!(deser.is_active);
        assert!(deser.is_running);
        assert_eq!(deser.elapsed_ms, 5000);
        assert_eq!(deser.item_title.as_deref(), Some("Working on task"));
    }

    #[test]
    fn timer_state_inactive() {
        let ts = TimerState {
            is_active: false,
            is_running: false,
            elapsed_ms: 0,
            item_title: None,
        };
        let json = serde_json::to_string(&ts).unwrap();
        let deser: TimerState = serde_json::from_str(&json).unwrap();
        assert!(!deser.is_active);
        assert!(deser.item_title.is_none());
    }

    // ── TimerResult ─────────────────────────────────────────────────

    #[test]
    fn timer_result_construction_and_serde() {
        let tr = TimerResult {
            item_id: "task-42".into(),
            elapsed_ms: 3600000,
        };
        let json = serde_json::to_string(&tr).unwrap();
        let deser: TimerResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.item_id, "task-42");
        assert_eq!(deser.elapsed_ms, 3600000);
    }

    // ── FieldType serde ─────────────────────────────────────────────

    #[test]
    fn field_type_all_variants_serde() {
        let variants = [
            FieldType::Text, FieldType::Tag, FieldType::DateTime,
            FieldType::Number, FieldType::Boolean, FieldType::Vector,
            FieldType::Counter, FieldType::Relation, FieldType::Decimal,
            FieldType::Json, FieldType::Enumeration, FieldType::GeoPoint,
            FieldType::Duration,
        ];
        for ft in &variants {
            let json = serde_json::to_string(ft).unwrap();
            let deser: FieldType = serde_json::from_str(&json).unwrap();
            assert_eq!(*ft, deser);
        }
    }

    // ── Plugin trait defaults ───────────────────────────────────────

    struct TestPlugin;

    impl Plugin for TestPlugin {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::default()
        }
        fn entity_schemas(&self) -> Vec<EntitySchema> {
            vec![]
        }
        fn initialize(&mut self) -> bool {
            true
        }
    }

    #[test]
    fn plugin_trait_default_navigation_item() {
        let p = TestPlugin;
        assert!(p.navigation_item().is_none());
    }

    #[test]
    fn plugin_trait_default_commands() {
        let p = TestPlugin;
        assert!(p.commands().is_empty());
    }

    #[test]
    fn plugin_trait_default_get_view_state() {
        let p = TestPlugin;
        assert_eq!(p.get_view_state(), "{}");
    }

    #[test]
    fn plugin_trait_default_handle_command() {
        let mut p = TestPlugin;
        assert_eq!(p.handle_command("anything", "{}"), "{}");
    }

    #[test]
    fn plugin_trait_lifecycle_defaults_do_not_panic() {
        let mut p = TestPlugin;
        p.activate();
        p.deactivate();
        p.on_navigated_to();
        p.on_navigated_from();
        p.dispose();
    }

    // ── TemplateDataProvider trait default ───────────────────────────

    struct TestTemplateProvider;

    impl TemplateDataProvider for TestTemplateProvider {
        // use default
    }

    #[test]
    fn template_data_provider_default() {
        let p = TestTemplateProvider;
        assert_eq!(p.get_view_data(), "{}");
    }
}
