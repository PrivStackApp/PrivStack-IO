use privstack_plugin_host::*;

#[test]
fn sdk_response_ok() {
    let resp = WitSdkResponse::ok(Some("data".into()));
    assert!(resp.success);
    assert_eq!(resp.data.as_deref(), Some("data"));
}

#[test]
fn sdk_response_err() {
    let resp = WitSdkResponse::err(404, "not found");
    assert!(!resp.success);
    assert_eq!(resp.error_code, Some(404));
}

#[test]
fn schema_conversion() {
    let wit_schema = WitEntitySchema {
        entity_type: "note".into(),
        indexed_fields: vec![
            WitIndexedField {
                field_path: "/title".into(),
                field_type: WitFieldType::Text,
                searchable: true,
                vector_dim: None,
                enum_options: None,
            },
            WitIndexedField {
                field_path: "/tags".into(),
                field_type: WitFieldType::Tag,
                searchable: true,
                vector_dim: None,
                enum_options: None,
            },
        ],
        merge_strategy: WitMergeStrategy::LwwPerField,
    };
    let core = wit_schema.to_core_schema().unwrap();
    assert_eq!(core.entity_type, "note");
    assert_eq!(core.indexed_fields.len(), 2);
}

#[test]
fn vector_field_carries_dim() {
    let field = WitIndexedField {
        field_path: "/embedding".into(),
        field_type: WitFieldType::Vector,
        searchable: false,
        vector_dim: Some(384),
        enum_options: None,
    };
    let core = field.to_core_field().unwrap();
    assert_eq!(core.field_type, privstack_model::FieldType::Vector);
    assert_eq!(core.vector_dim, Some(384));
}

#[test]
fn sdk_response_permission_denied() {
    let resp = WitSdkResponse::permission_denied("vault");
    assert!(!resp.success);
    assert_eq!(resp.error_code, Some(403));
    assert!(resp.error_message.as_ref().unwrap().contains("vault"));
    assert!(resp.error_message.as_ref().unwrap().contains("permission denied"));
    assert!(resp.data.is_none());
}

#[test]
fn sdk_response_ok_none_data() {
    let resp = WitSdkResponse::ok(None);
    assert!(resp.success);
    assert!(resp.data.is_none());
    assert!(resp.error_code.is_none());
    assert!(resp.error_message.is_none());
}

#[test]
fn timer_state_construction() {
    let state = WitTimerState {
        is_active: true,
        is_running: true,
        elapsed_ms: 5000,
        item_title: Some("My Task".into()),
    };
    assert!(state.is_active);
    assert!(state.is_running);
    assert_eq!(state.elapsed_ms, 5000);
    assert_eq!(state.item_title.as_deref(), Some("My Task"));
}

#[test]
fn timer_state_inactive() {
    let state = WitTimerState {
        is_active: false,
        is_running: false,
        elapsed_ms: 0,
        item_title: None,
    };
    assert!(!state.is_active);
    assert!(!state.is_running);
    assert_eq!(state.elapsed_ms, 0);
    assert!(state.item_title.is_none());
}

#[test]
fn timer_result_construction() {
    let result = WitTimerResult {
        item_id: "task-123".into(),
        elapsed_ms: 3600000,
    };
    assert_eq!(result.item_id, "task-123");
    assert_eq!(result.elapsed_ms, 3600000);
}

#[test]
fn all_field_type_conversions() {
    let field_types = vec![
        (WitFieldType::Text, privstack_model::FieldType::Text),
        (WitFieldType::Tag, privstack_model::FieldType::Tag),
        (WitFieldType::DateTime, privstack_model::FieldType::DateTime),
        (WitFieldType::Number, privstack_model::FieldType::Number),
        (WitFieldType::Boolean, privstack_model::FieldType::Bool),
        (WitFieldType::Vector, privstack_model::FieldType::Vector),
        (WitFieldType::Counter, privstack_model::FieldType::Counter),
        (WitFieldType::Relation, privstack_model::FieldType::Relation),
        (WitFieldType::Decimal, privstack_model::FieldType::Decimal),
        (WitFieldType::Json, privstack_model::FieldType::Json),
        (WitFieldType::Enumeration, privstack_model::FieldType::Enum),
        (WitFieldType::GeoPoint, privstack_model::FieldType::GeoPoint),
        (WitFieldType::Duration, privstack_model::FieldType::Duration),
    ];
    for (wit_type, expected_core) in field_types {
        let field = WitIndexedField {
            field_path: "/test".into(),
            field_type: wit_type,
            searchable: false,
            vector_dim: None,
            enum_options: None,
        };
        let core = field.to_core_field().unwrap();
        assert_eq!(core.field_type, expected_core, "Failed for {:?}", wit_type);
    }
}

#[test]
fn merge_strategy_lww_document() {
    assert_eq!(
        WitMergeStrategy::LwwDocument.to_core(),
        privstack_model::MergeStrategy::LwwDocument
    );
}

#[test]
fn merge_strategy_lww_per_field() {
    assert_eq!(
        WitMergeStrategy::LwwPerField.to_core(),
        privstack_model::MergeStrategy::LwwPerField
    );
}

#[test]
fn merge_strategy_custom() {
    assert_eq!(
        WitMergeStrategy::Custom.to_core(),
        privstack_model::MergeStrategy::Custom
    );
}

#[test]
fn link_provider_info_construction() {
    let info = WitLinkProviderInfo {
        plugin_id: "notes".into(),
        link_type: "note".into(),
        display_name: "Notes".into(),
        icon: Some("icon-note".into()),
    };
    assert_eq!(info.plugin_id, "notes");
    assert_eq!(info.link_type, "note");
    assert_eq!(info.display_name, "Notes");
    assert_eq!(info.icon.as_deref(), Some("icon-note"));
}

#[test]
fn navigation_item_construction() {
    let item = WitNavigationItem {
        id: "nav-1".into(),
        display_name: "Notes".into(),
        subtitle: Some("All notes".into()),
        icon: Some("icon-notes".into()),
        tooltip: Some("View notes".into()),
        order: 100,
        show_badge: true,
        badge_count: 5,
        shortcut_hint: Some("Ctrl+N".into()),
    };
    assert_eq!(item.id, "nav-1");
    assert_eq!(item.order, 100);
    assert!(item.show_badge);
    assert_eq!(item.badge_count, 5);
    assert_eq!(item.shortcut_hint.as_deref(), Some("Ctrl+N"));
}

#[test]
fn command_definition_construction() {
    let cmd = WitCommandDefinition {
        name: "create-note".into(),
        description: "Create a new note".into(),
        keywords: "note new create".into(),
        category: "notes".into(),
        icon: Some("plus".into()),
    };
    assert_eq!(cmd.name, "create-note");
    assert_eq!(cmd.category, "notes");
    assert_eq!(cmd.icon.as_deref(), Some("plus"));
}

#[test]
fn linkable_item_construction() {
    let item = WitLinkableItem {
        id: "item-1".into(),
        link_type: "note".into(),
        title: "My Note".into(),
        subtitle: Some("A subtitle".into()),
        icon: None,
        modified_at: 1700000000,
        plugin_id: None,
    };
    assert_eq!(item.id, "item-1");
    assert_eq!(item.modified_at, 1700000000);
    assert!(item.icon.is_none());
}

#[test]
fn sdk_action_debug() {
    let action = WitSdkAction::Create;
    let debug_str = format!("{:?}", action);
    assert!(debug_str.contains("Create"));
}

#[test]
fn sdk_action_clone_and_eq() {
    let action = WitSdkAction::Delete;
    let cloned = action;
    assert_eq!(action, cloned);
}

#[test]
fn plugin_category_all_variants_debug() {
    let categories = [
        WitPluginCategory::Productivity,
        WitPluginCategory::Security,
        WitPluginCategory::Communication,
        WitPluginCategory::Information,
        WitPluginCategory::Utility,
        WitPluginCategory::Extension,
    ];
    for cat in &categories {
        let _ = format!("{:?}", cat);
    }
}

#[test]
fn schema_with_enum_options_converts() {
    let schema = WitEntitySchema {
        entity_type: "priority_item".into(),
        indexed_fields: vec![WitIndexedField {
            field_path: "/priority".into(),
            field_type: WitFieldType::Enumeration,
            searchable: true,
            vector_dim: None,
            enum_options: Some(vec!["low".into(), "medium".into(), "high".into()]),
        }],
        merge_strategy: WitMergeStrategy::LwwPerField,
    };
    let core = schema.to_core_schema().unwrap();
    assert_eq!(core.indexed_fields[0].enum_options.as_ref().unwrap().len(), 3);
}

#[test]
fn sdk_message_debug() {
    let msg = WitSdkMessage {
        action: WitSdkAction::List,
        entity_type: "test".into(),
        entity_id: None,
        payload: None,
        parameters: vec![("key".into(), "val".into())],
        source: Some("plugin-a".into()),
    };
    let debug_str = format!("{:?}", msg);
    assert!(debug_str.contains("List"));
    assert!(debug_str.contains("test"));
}

#[test]
fn sdk_message_clone() {
    let msg = WitSdkMessage {
        action: WitSdkAction::Create,
        entity_type: "note".into(),
        entity_id: Some("id-1".into()),
        payload: Some("{}".into()),
        parameters: vec![],
        source: None,
    };
    let cloned = msg.clone();
    assert_eq!(cloned.entity_type, "note");
    assert_eq!(cloned.entity_id.as_deref(), Some("id-1"));
}

#[test]
fn sdk_response_serialize_deserialize() {
    let resp = WitSdkResponse::ok(Some("test data".into()));
    let json = serde_json::to_string(&resp).unwrap();
    let deserialized: WitSdkResponse = serde_json::from_str(&json).unwrap();
    assert!(deserialized.success);
    assert_eq!(deserialized.data.as_deref(), Some("test data"));
}

#[test]
fn timer_state_serialize_deserialize() {
    let state = WitTimerState {
        is_active: true,
        is_running: false,
        elapsed_ms: 1234,
        item_title: Some("Task".into()),
    };
    let json = serde_json::to_string(&state).unwrap();
    let deserialized: WitTimerState = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.elapsed_ms, 1234);
    assert!(deserialized.is_active);
    assert!(!deserialized.is_running);
}

#[test]
fn timer_result_serialize_deserialize() {
    let result = WitTimerResult {
        item_id: "t-1".into(),
        elapsed_ms: 60000,
    };
    let json = serde_json::to_string(&result).unwrap();
    let deserialized: WitTimerResult = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.item_id, "t-1");
}
