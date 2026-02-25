use privstack_plugin_host::*;
use std::path::Path;
use std::sync::Arc;
use wasmtime::{Engine, ResourceLimiter};

fn test_stores() -> (Arc<privstack_storage::EntityStore>, Arc<privstack_storage::EventStore>) {
    let es = privstack_storage::EntityStore::open_in_memory().unwrap();
    let ev = privstack_storage::EventStore::open_in_memory().unwrap();
    (Arc::new(es), Arc::new(ev))
}

fn test_metadata() -> WitPluginMetadata {
    WitPluginMetadata {
        id: "test.plugin".into(),
        name: "Test Plugin".into(),
        description: "A test plugin".into(),
        version: "0.1.0".into(),
        author: "Test".into(),
        icon: None,
        navigation_order: 100,
        category: WitPluginCategory::Utility,
        can_disable: true,
        is_experimental: false,
    }
}

fn test_schemas() -> Vec<WitEntitySchema> {
    vec![WitEntitySchema {
        entity_type: "test_item".into(),
        indexed_fields: vec![WitIndexedField {
            field_path: "/title".into(),
            field_type: WitFieldType::Text,
            searchable: true,
            vector_dim: None,
            enum_options: None,
        }],
        merge_strategy: WitMergeStrategy::LwwPerField,
    }]
}

#[test]
fn sandbox_creation() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert_eq!(sandbox.plugin_id(), "test.plugin");
    assert!(sandbox.declared_entity_types().contains("test_item"));
}

#[test]
fn entity_type_scoping_enforced() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let msg = WitSdkMessage {
        action: WitSdkAction::Create, entity_type: "test_item".into(),
        entity_id: None, payload: Some("{}".into()), parameters: vec![], source: None,
    };
    let resp = sandbox.handle_sdk_send(&msg);
    assert!(resp.success);
    let bad_msg = WitSdkMessage { entity_type: "other_type".into(), ..msg };
    let resp = sandbox.handle_sdk_send(&bad_msg);
    assert!(!resp.success);
    assert_eq!(resp.error_code, Some(403));
}

#[test]
fn permission_checks() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert!(sandbox.check_vault_access().is_err());
    assert!(sandbox.check_linking_access().is_err());
}

#[test]
fn settings_crud() {
    let (es, ev) = test_stores();
    let mut sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert_eq!(sandbox.handle_settings_get("key", "default"), "default");
    sandbox.handle_settings_set("key", "value");
    assert_eq!(sandbox.handle_settings_get("key", "default"), "value");
    sandbox.handle_settings_remove("key");
    assert_eq!(sandbox.handle_settings_get("key", "default"), "default");
}

#[test]
fn vector_field_without_dim_accepted() {
    let (es, ev) = test_stores();
    let schemas = vec![WitEntitySchema {
        entity_type: "bad".into(),
        indexed_fields: vec![WitIndexedField {
            field_path: "/embedding".into(), field_type: WitFieldType::Vector,
            searchable: false, vector_dim: None, enum_options: None,
        }],
        merge_strategy: WitMergeStrategy::LwwDocument,
    }];
    let result = PluginSandbox::new(
        test_metadata(), schemas,
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    );
    assert!(result.is_ok());
}

#[test]
fn first_party_limits_values() {
    let limits = ResourceLimits::first_party();
    assert_eq!(limits.max_memory_bytes, 64 * 1024 * 1024);
    assert_eq!(limits.fuel_per_call, 1_000_000_000);
    assert_eq!(limits.call_timeout_ms, 5_000);
    assert_eq!(limits.shutdown_deadline_ms, 2_000);
}

#[test]
fn third_party_limits_values() {
    let limits = ResourceLimits::third_party();
    assert_eq!(limits.max_memory_bytes, 32 * 1024 * 1024);
    assert_eq!(limits.fuel_per_call, 500_000_000);
    assert_eq!(limits.call_timeout_ms, 3_000);
    assert_eq!(limits.shutdown_deadline_ms, 2_000);
}

#[test]
fn third_party_limits_stricter_than_first_party() {
    let fp = ResourceLimits::first_party();
    let tp = ResourceLimits::third_party();
    assert!(tp.max_memory_bytes < fp.max_memory_bytes);
    assert!(tp.fuel_per_call < fp.fuel_per_call);
    assert!(tp.call_timeout_ms <= fp.call_timeout_ms);
}

#[test]
fn check_entity_type_access_allowed() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert!(sandbox.state().check_entity_type_access("test_item").is_ok());
}

#[test]
fn check_entity_type_access_denied() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let result = sandbox.state().check_entity_type_access("forbidden_type");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("forbidden_type"));
    assert!(err.to_string().contains("test.plugin"));
}

#[test]
fn check_permission_granted() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::all_granted(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert!(sandbox.state().check_permission(Permission::Vault).is_ok());
    assert!(sandbox.state().check_permission(Permission::Network).is_ok());
    assert!(sandbox.state().check_permission(Permission::Linking).is_ok());
}

#[test]
fn check_permission_denied_returns_error() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let err = sandbox.state().check_permission(Permission::Network).unwrap_err();
    assert!(err.to_string().contains("network"));
    assert!(err.to_string().contains("test.plugin"));
}

#[test]
fn check_dialog_access() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert!(sandbox.check_dialog_access().is_err());
    let (es2, ev2) = test_stores();
    let sandbox2 = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::all_granted(), ResourceLimits::first_party(), es2, ev2,
    ).unwrap();
    assert!(sandbox2.check_dialog_access().is_ok());
}

#[test]
fn settings_overwrite_existing_key() {
    let (es, ev) = test_stores();
    let mut sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    sandbox.handle_settings_set("k", "v1");
    assert_eq!(sandbox.handle_settings_get("k", ""), "v1");
    sandbox.handle_settings_set("k", "v2");
    assert_eq!(sandbox.handle_settings_get("k", ""), "v2");
}

#[test]
fn settings_remove_nonexistent_key_is_noop() {
    let (es, ev) = test_stores();
    let mut sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    sandbox.handle_settings_remove("nonexistent");
}

#[test]
fn settings_multiple_keys() {
    let (es, ev) = test_stores();
    let mut sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    sandbox.handle_settings_set("a", "1");
    sandbox.handle_settings_set("b", "2");
    sandbox.handle_settings_set("c", "3");
    assert_eq!(sandbox.handle_settings_get("a", ""), "1");
    assert_eq!(sandbox.handle_settings_get("b", ""), "2");
    assert_eq!(sandbox.handle_settings_get("c", ""), "3");
}

#[test]
fn update_permissions_changes_access() {
    let (es, ev) = test_stores();
    let mut sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert!(sandbox.check_vault_access().is_err());
    sandbox.update_permissions(PermissionSet::all_granted());
    assert!(sandbox.check_vault_access().is_ok());
    assert!(sandbox.check_linking_access().is_ok());
}

#[test]
fn state_dirty_initially_false() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert!(!sandbox.is_state_dirty());
}

#[test]
fn state_dirty_set_via_state_mut() {
    let (es, ev) = test_stores();
    let mut sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    sandbox.state_mut().state_dirty = true;
    assert!(sandbox.is_state_dirty());
}

#[test]
fn plugin_id_accessor() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert_eq!(sandbox.plugin_id(), "test.plugin");
}

#[test]
fn declared_entity_types_accessor() {
    let (es, ev) = test_stores();
    let schemas = vec![
        WitEntitySchema {
            entity_type: "type_a".into(),
            indexed_fields: vec![WitIndexedField {
                field_path: "/name".into(), field_type: WitFieldType::Text,
                searchable: true, vector_dim: None, enum_options: None,
            }],
            merge_strategy: WitMergeStrategy::LwwPerField,
        },
        WitEntitySchema {
            entity_type: "type_b".into(),
            indexed_fields: vec![WitIndexedField {
                field_path: "/title".into(), field_type: WitFieldType::Text,
                searchable: true, vector_dim: None, enum_options: None,
            }],
            merge_strategy: WitMergeStrategy::LwwDocument,
        },
    ];
    let sandbox = PluginSandbox::new(
        test_metadata(), schemas,
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let types = sandbox.declared_entity_types();
    assert_eq!(types.len(), 2);
    assert!(types.contains("type_a"));
    assert!(types.contains("type_b"));
}

#[test]
fn metadata_only_sandbox_has_no_runtime_capabilities() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert!(!sandbox.has_linkable_item_provider);
    assert!(!sandbox.has_deep_link_target);
    assert!(!sandbox.has_timer);
    assert!(!sandbox.has_shutdown_aware);
}

#[test]
fn metadata_only_sandbox_runtime_calls_fail() {
    let (es, ev) = test_stores();
    let mut sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert!(sandbox.call_initialize().is_err());
    assert!(sandbox.call_activate().is_err());
    assert!(sandbox.call_deactivate().is_err());
    assert!(sandbox.call_dispose().is_err());
    assert!(sandbox.call_on_navigated_to().is_err());
    assert!(sandbox.call_on_navigated_from().is_err());
    assert!(sandbox.call_get_view_state().is_err());
    assert!(sandbox.call_get_view_data().is_err());
    assert!(sandbox.call_handle_command("test", "{}").is_err());
    assert!(sandbox.call_get_navigation_item().is_err());
    assert!(sandbox.call_get_commands().is_err());
    assert!(sandbox.call_search_linkable_items("query", 10).is_err());
}

#[test]
fn handle_sdk_send_with_valid_entity_type() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let msg = WitSdkMessage {
        action: WitSdkAction::List, entity_type: "test_item".into(),
        entity_id: None, payload: None, parameters: vec![], source: None,
    };
    let resp = sandbox.handle_sdk_send(&msg);
    assert!(resp.success);
}

#[test]
fn pending_navigation_initially_none() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert!(sandbox.state().pending_navigation.is_none());
}

#[test]
fn view_state_initially_none() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert!(sandbox.state().view_state.is_none());
}

#[test]
fn schemas_stored_in_state() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert_eq!(sandbox.state().schemas.len(), 1);
    assert_eq!(sandbox.state().schemas[0].entity_type, "test_item");
}

#[test]
fn resource_limits_debug_impl() {
    let limits = ResourceLimits::first_party();
    let debug_str = format!("{:?}", limits);
    assert!(debug_str.contains("ResourceLimits"));
    assert!(debug_str.contains("max_memory_bytes"));
}

#[test]
fn resource_limits_clone_impl() {
    let limits = ResourceLimits::first_party();
    let cloned = limits.clone();
    assert_eq!(cloned.max_memory_bytes, limits.max_memory_bytes);
    assert_eq!(cloned.fuel_per_call, limits.fuel_per_call);
    assert_eq!(cloned.call_timeout_ms, limits.call_timeout_ms);
    assert_eq!(cloned.shutdown_deadline_ms, limits.shutdown_deadline_ms);
}

#[test]
fn from_wasm_nonexistent_file_returns_error() {
    let (es, ev) = test_stores();
    let result = PluginSandbox::from_wasm(
        Path::new("/nonexistent/path/plugin.wasm"),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    );
    match result {
        Err(e) => assert!(e.to_string().contains("failed to read")),
        Ok(_) => panic!("expected error for nonexistent file"),
    }
}

#[test]
fn sandbox_with_empty_schemas() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), vec![],
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert!(sandbox.declared_entity_types().is_empty());
    assert_eq!(sandbox.state().schemas.len(), 0);
}

#[test]
fn sandbox_metadata_with_icon() {
    let (es, ev) = test_stores();
    let mut meta = test_metadata();
    meta.icon = Some("icon-notes".into());
    meta.is_experimental = true;
    meta.can_disable = false;
    let sandbox = PluginSandbox::new(
        meta, test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert_eq!(sandbox.metadata.icon.as_deref(), Some("icon-notes"));
    assert!(sandbox.metadata.is_experimental);
    assert!(!sandbox.metadata.can_disable);
}

#[test]
fn handle_sdk_send_with_read_action() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let msg = WitSdkMessage {
        action: WitSdkAction::Read, entity_type: "test_item".into(),
        entity_id: Some("some-id".into()), payload: None,
        parameters: vec![], source: Some("test".into()),
    };
    let resp = sandbox.handle_sdk_send(&msg);
    assert!(resp.success);
}

#[test]
fn handle_sdk_send_with_update_action() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let msg = WitSdkMessage {
        action: WitSdkAction::Update, entity_type: "test_item".into(),
        entity_id: Some("id".into()), payload: Some(r#"{"title":"updated"}"#.into()),
        parameters: vec![], source: None,
    };
    let resp = sandbox.handle_sdk_send(&msg);
    assert!(resp.success);
}

#[test]
fn handle_sdk_send_with_delete_action() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let msg = WitSdkMessage {
        action: WitSdkAction::Delete, entity_type: "test_item".into(),
        entity_id: Some("id".into()), payload: None, parameters: vec![], source: None,
    };
    let resp = sandbox.handle_sdk_send(&msg);
    assert!(resp.success);
}

#[test]
fn handle_sdk_send_with_query_action() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let msg = WitSdkMessage {
        action: WitSdkAction::Query, entity_type: "test_item".into(),
        entity_id: None, payload: Some(r#"{"filter":"active"}"#.into()),
        parameters: vec![("limit".into(), "10".into())], source: None,
    };
    let resp = sandbox.handle_sdk_send(&msg);
    assert!(resp.success);
}

#[test]
fn handle_sdk_send_with_trash_action() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let msg = WitSdkMessage {
        action: WitSdkAction::Trash, entity_type: "test_item".into(),
        entity_id: Some("id".into()), payload: None, parameters: vec![], source: None,
    };
    let resp = sandbox.handle_sdk_send(&msg);
    assert!(resp.success);
}

#[test]
fn handle_sdk_send_with_restore_action() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let msg = WitSdkMessage {
        action: WitSdkAction::Restore, entity_type: "test_item".into(),
        entity_id: Some("id".into()), payload: None, parameters: vec![], source: None,
    };
    let resp = sandbox.handle_sdk_send(&msg);
    assert!(resp.success);
}

#[test]
fn handle_sdk_send_with_link_action() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let msg = WitSdkMessage {
        action: WitSdkAction::Link, entity_type: "test_item".into(),
        entity_id: Some("id".into()), payload: None, parameters: vec![], source: None,
    };
    let resp = sandbox.handle_sdk_send(&msg);
    assert!(resp.success);
}

#[test]
fn handle_sdk_send_with_semantic_search_action() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let msg = WitSdkMessage {
        action: WitSdkAction::SemanticSearch, entity_type: "test_item".into(),
        entity_id: None, payload: Some("search query".into()),
        parameters: vec![], source: None,
    };
    let resp = sandbox.handle_sdk_send(&msg);
    assert!(resp.success);
}

#[test]
fn set_pending_navigation_via_state_mut() {
    let (es, ev) = test_stores();
    let mut sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    sandbox.state_mut().pending_navigation = Some("target.plugin".into());
    assert_eq!(sandbox.state().pending_navigation.as_deref(), Some("target.plugin"));
}

#[test]
fn set_view_state_via_state_mut() {
    let (es, ev) = test_stores();
    let mut sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    sandbox.state_mut().view_state = Some(r#"{"count":42}"#.into());
    assert_eq!(sandbox.state().view_state.as_deref(), Some(r#"{"count":42}"#));
}

#[test]
fn plugin_state_plugin_id_matches() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert_eq!(sandbox.state().plugin_id, "test.plugin");
}

#[test]
fn plugin_state_entity_store_accessible() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let _store_ref = &sandbox.state().entity_store;
}

#[test]
fn plugin_state_event_store_accessible() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let _store_ref = &sandbox.state().event_store;
}

#[test]
fn runtime_mut_error_contains_plugin_id() {
    let (es, ev) = test_stores();
    let mut sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let err = sandbox.call_initialize().unwrap_err();
    assert!(err.to_string().contains("test.plugin"));
    assert!(err.to_string().contains("metadata-only"));
}

#[test]
fn check_vault_access_granted() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::all_granted(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert!(sandbox.check_vault_access().is_ok());
}

#[test]
fn check_linking_access_granted() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::all_granted(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert!(sandbox.check_linking_access().is_ok());
}

fn make_schema_with_field_type(field_type: WitFieldType) -> Vec<WitEntitySchema> {
    vec![WitEntitySchema {
        entity_type: "ft_test".into(),
        indexed_fields: vec![WitIndexedField {
            field_path: "/field".into(),
            field_type,
            searchable: false,
            vector_dim: if matches!(field_type, WitFieldType::Vector) { Some(128) } else { None },
            enum_options: if matches!(field_type, WitFieldType::Enumeration) {
                Some(vec!["a".into(), "b".into()])
            } else { None },
        }],
        merge_strategy: WitMergeStrategy::LwwPerField,
    }]
}

#[test]
fn schema_with_tag_field_type() {
    let (es, ev) = test_stores();
    assert!(PluginSandbox::new(
        test_metadata(), make_schema_with_field_type(WitFieldType::Tag),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).is_ok());
}

#[test]
fn schema_with_datetime_field_type() {
    let (es, ev) = test_stores();
    assert!(PluginSandbox::new(
        test_metadata(), make_schema_with_field_type(WitFieldType::DateTime),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).is_ok());
}

#[test]
fn schema_with_number_field_type() {
    let (es, ev) = test_stores();
    assert!(PluginSandbox::new(
        test_metadata(), make_schema_with_field_type(WitFieldType::Number),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).is_ok());
}

#[test]
fn schema_with_boolean_field_type() {
    let (es, ev) = test_stores();
    assert!(PluginSandbox::new(
        test_metadata(), make_schema_with_field_type(WitFieldType::Boolean),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).is_ok());
}

#[test]
fn schema_with_counter_field_type() {
    let (es, ev) = test_stores();
    assert!(PluginSandbox::new(
        test_metadata(), make_schema_with_field_type(WitFieldType::Counter),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).is_ok());
}

#[test]
fn schema_with_relation_field_type() {
    let (es, ev) = test_stores();
    assert!(PluginSandbox::new(
        test_metadata(), make_schema_with_field_type(WitFieldType::Relation),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).is_ok());
}

#[test]
fn schema_with_decimal_field_type() {
    let (es, ev) = test_stores();
    assert!(PluginSandbox::new(
        test_metadata(), make_schema_with_field_type(WitFieldType::Decimal),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).is_ok());
}

#[test]
fn schema_with_json_field_type() {
    let (es, ev) = test_stores();
    assert!(PluginSandbox::new(
        test_metadata(), make_schema_with_field_type(WitFieldType::Json),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).is_ok());
}

#[test]
fn schema_with_enumeration_field_type() {
    let (es, ev) = test_stores();
    assert!(PluginSandbox::new(
        test_metadata(), make_schema_with_field_type(WitFieldType::Enumeration),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).is_ok());
}

#[test]
fn schema_with_geopoint_field_type() {
    let (es, ev) = test_stores();
    assert!(PluginSandbox::new(
        test_metadata(), make_schema_with_field_type(WitFieldType::GeoPoint),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).is_ok());
}

#[test]
fn schema_with_duration_field_type() {
    let (es, ev) = test_stores();
    assert!(PluginSandbox::new(
        test_metadata(), make_schema_with_field_type(WitFieldType::Duration),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).is_ok());
}

#[test]
fn schema_with_vector_field_type_and_dim() {
    let (es, ev) = test_stores();
    assert!(PluginSandbox::new(
        test_metadata(), make_schema_with_field_type(WitFieldType::Vector),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).is_ok());
}

#[test]
fn schema_with_custom_merge_strategy() {
    let (es, ev) = test_stores();
    let schemas = vec![WitEntitySchema {
        entity_type: "custom_merge".into(),
        indexed_fields: vec![WitIndexedField {
            field_path: "/data".into(), field_type: WitFieldType::Text,
            searchable: true, vector_dim: None, enum_options: None,
        }],
        merge_strategy: WitMergeStrategy::Custom,
    }];
    assert!(PluginSandbox::new(
        test_metadata(), schemas,
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).is_ok());
}

#[test]
fn sandbox_with_third_party_limits() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_third_party(), ResourceLimits::third_party(), es, ev,
    ).unwrap();
    assert_eq!(sandbox.resource_limits.max_memory_bytes, 32 * 1024 * 1024);
    assert_eq!(sandbox.plugin_id(), "test.plugin");
}

#[test]
fn handle_sdk_send_checks_correct_entity_type_in_multi_schema() {
    let (es, ev) = test_stores();
    let schemas = vec![
        WitEntitySchema {
            entity_type: "note".into(),
            indexed_fields: vec![WitIndexedField {
                field_path: "/title".into(), field_type: WitFieldType::Text,
                searchable: true, vector_dim: None, enum_options: None,
            }],
            merge_strategy: WitMergeStrategy::LwwPerField,
        },
        WitEntitySchema {
            entity_type: "task".into(),
            indexed_fields: vec![WitIndexedField {
                field_path: "/name".into(), field_type: WitFieldType::Text,
                searchable: true, vector_dim: None, enum_options: None,
            }],
            merge_strategy: WitMergeStrategy::LwwPerField,
        },
    ];
    let sandbox = PluginSandbox::new(
        test_metadata(), schemas,
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let msg_note = WitSdkMessage {
        action: WitSdkAction::Create, entity_type: "note".into(),
        entity_id: None, payload: Some("{}".into()), parameters: vec![], source: None,
    };
    assert!(sandbox.handle_sdk_send(&msg_note).success);
    let msg_task = WitSdkMessage {
        action: WitSdkAction::Create, entity_type: "task".into(),
        entity_id: None, payload: Some("{}".into()), parameters: vec![], source: None,
    };
    assert!(sandbox.handle_sdk_send(&msg_task).success);
    let msg_other = WitSdkMessage {
        action: WitSdkAction::Create, entity_type: "calendar".into(),
        entity_id: None, payload: Some("{}".into()), parameters: vec![], source: None,
    };
    assert!(!sandbox.handle_sdk_send(&msg_other).success);
}

#[test]
fn tracking_limiter_initial_memory_zero() {
    let limiter = TrackingLimiter::new(1024);
    assert_eq!(limiter.current_memory_bytes(), 0);
}

#[test]
fn tracking_limiter_memory_growing_allowed_within_limit() {
    let mut limiter = TrackingLimiter::new(1024);
    let result = limiter.memory_growing(0, 512, None).unwrap();
    assert!(result);
    assert_eq!(limiter.current_memory_bytes(), 512);
}

#[test]
fn tracking_limiter_memory_growing_allowed_at_limit() {
    let mut limiter = TrackingLimiter::new(1024);
    let result = limiter.memory_growing(0, 1024, None).unwrap();
    assert!(result);
    assert_eq!(limiter.current_memory_bytes(), 1024);
}

#[test]
fn tracking_limiter_memory_growing_denied_over_limit() {
    let mut limiter = TrackingLimiter::new(1024);
    let result = limiter.memory_growing(0, 1025, None).unwrap();
    assert!(!result);
    assert_eq!(limiter.current_memory_bytes(), 1025);
}

#[test]
fn tracking_limiter_memory_growing_tracks_incremental_growth() {
    let mut limiter = TrackingLimiter::new(4096);
    limiter.memory_growing(0, 1024, None).unwrap();
    assert_eq!(limiter.current_memory_bytes(), 1024);
    limiter.memory_growing(1024, 2048, None).unwrap();
    assert_eq!(limiter.current_memory_bytes(), 2048);
    limiter.memory_growing(2048, 3072, None).unwrap();
    assert_eq!(limiter.current_memory_bytes(), 3072);
}

#[test]
fn tracking_limiter_memory_growing_with_maximum_param() {
    let mut limiter = TrackingLimiter::new(2048);
    let result = limiter.memory_growing(0, 1024, Some(4096)).unwrap();
    assert!(result);
}

#[test]
fn tracking_limiter_table_growing_allowed() {
    let mut limiter = TrackingLimiter::new(1024);
    let result = limiter.table_growing(0, 100, None).unwrap();
    assert!(result);
}

#[test]
fn tracking_limiter_table_growing_denied_over_limit() {
    let mut limiter = TrackingLimiter::new(1024);
    let result = limiter.table_growing(0, 20_001, None).unwrap();
    assert!(!result);
}

#[test]
fn tracking_limiter_table_growing_allowed_at_limit() {
    let mut limiter = TrackingLimiter::new(1024);
    let result = limiter.table_growing(0, 20_000, None).unwrap();
    assert!(result);
}

#[test]
fn tracking_limiter_instances() {
    let limiter = TrackingLimiter::new(1024);
    assert_eq!(limiter.instances(), 50);
}

#[test]
fn tracking_limiter_tables() {
    let limiter = TrackingLimiter::new(1024);
    assert_eq!(limiter.tables(), 100);
}

#[test]
fn tracking_limiter_memories() {
    let limiter = TrackingLimiter::new(1024);
    assert_eq!(limiter.memories(), 50);
}

#[test]
fn has_runtime_false_for_metadata_only() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert!(!sandbox.has_runtime());
}

#[test]
fn get_resource_metrics_metadata_only() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let metrics = sandbox.get_resource_metrics();
    assert_eq!(metrics.memory_used_bytes, 0);
    assert_eq!(metrics.memory_limit_bytes, 64 * 1024 * 1024);
    assert!((metrics.memory_usage_ratio - 0.0).abs() < f64::EPSILON);
    assert_eq!(metrics.fuel_consumed_last_call, 0);
    assert_eq!(metrics.fuel_budget_per_call, 1_000_000_000);
    assert_eq!(metrics.fuel_average_last_1000, 0);
    assert_eq!(metrics.fuel_peak, 0);
    assert_eq!(metrics.fuel_history_count, 0);
    assert_eq!(metrics.entity_count, 0);
    assert_eq!(metrics.disk_usage_bytes, 0);
}

#[test]
fn get_resource_metrics_with_third_party_limits() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_third_party(), ResourceLimits::third_party(), es, ev,
    ).unwrap();
    let metrics = sandbox.get_resource_metrics();
    assert_eq!(metrics.memory_limit_bytes, 32 * 1024 * 1024);
    assert_eq!(metrics.fuel_budget_per_call, 500_000_000);
}

#[test]
fn get_resource_metrics_with_entities() {
    let (es, ev) = test_stores();
    let entity = privstack_model::Entity {
        id: "test-1".into(),
        entity_type: "test_item".into(),
        data: serde_json::json!({"title": "test"}),
        created_at: chrono::Utc::now().timestamp_millis(),
        modified_at: chrono::Utc::now().timestamp_millis(),
        created_by: "device-1".into(),
    };
    es.save_entity_raw(&entity).unwrap();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(),
        Arc::clone(&es), Arc::clone(&ev),
    ).unwrap();
    let metrics = sandbox.get_resource_metrics();
    assert!(metrics.entity_count >= 1);
    assert!(metrics.disk_usage_bytes > 0);
}

#[test]
fn resource_metrics_serialize() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let metrics = sandbox.get_resource_metrics();
    let json = serde_json::to_string(&metrics).unwrap();
    assert!(json.contains("memory_used_bytes"));
    assert!(json.contains("fuel_budget_per_call"));
}

#[test]
fn call_link_type_metadata_only_fails() {
    let (es, ev) = test_stores();
    let mut sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert!(sandbox.call_link_type().is_err());
}

#[test]
fn call_navigate_to_item_metadata_only_fails() {
    let (es, ev) = test_stores();
    let mut sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    assert!(sandbox.call_navigate_to_item("some-item").is_err());
}

#[test]
fn from_wasm_cached_nonexistent_file_returns_error() {
    let (es, ev) = test_stores();
    let mut config = wasmtime::Config::new();
    config.wasm_component_model(true);
    config.consume_fuel(true);
    let engine = Engine::new(&config).unwrap();
    let result = PluginSandbox::from_wasm_cached(
        Path::new("/nonexistent/path/plugin.wasm"),
        &engine,
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    );
    match result {
        Err(e) => assert!(e.to_string().contains("failed to read")),
        Ok(_) => panic!("expected error for nonexistent file"),
    }
}

#[test]
fn track_fuel_consumption_no_op_metadata_only() {
    let (es, ev) = test_stores();
    let mut sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    sandbox.track_fuel_consumption();
    assert_eq!(sandbox.last_fuel_consumed, 0);
}

#[test]
fn plugin_resource_metrics_debug() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let metrics = sandbox.get_resource_metrics();
    let debug_str = format!("{:?}", metrics);
    assert!(debug_str.contains("PluginResourceMetrics"));
    assert!(debug_str.contains("memory_used_bytes"));
}

#[test]
fn plugin_resource_metrics_clone() {
    let (es, ev) = test_stores();
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev,
    ).unwrap();
    let metrics = sandbox.get_resource_metrics();
    let cloned = metrics.clone();
    assert_eq!(cloned.memory_used_bytes, metrics.memory_used_bytes);
    assert_eq!(cloned.fuel_budget_per_call, metrics.fuel_budget_per_call);
}

#[test]
fn get_resource_metrics_zero_memory_limit() {
    let (es, ev) = test_stores();
    let limits = ResourceLimits {
        max_memory_bytes: 0,
        fuel_per_call: 1000,
        call_timeout_ms: 1000,
        shutdown_deadline_ms: 1000,
    };
    let sandbox = PluginSandbox::new(
        test_metadata(), test_schemas(),
        PermissionSet::default_first_party(), limits, es, ev,
    ).unwrap();
    let metrics = sandbox.get_resource_metrics();
    assert!((metrics.memory_usage_ratio - 0.0).abs() < f64::EPSILON);
    assert_eq!(metrics.memory_limit_bytes, 0);
}

#[test]
fn sandbox_with_productivity_category() {
    let (es, ev) = test_stores();
    let mut meta = test_metadata();
    meta.category = WitPluginCategory::Productivity;
    assert!(PluginSandbox::new(meta, test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev).is_ok());
}

#[test]
fn sandbox_with_security_category() {
    let (es, ev) = test_stores();
    let mut meta = test_metadata();
    meta.category = WitPluginCategory::Security;
    assert!(PluginSandbox::new(meta, test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev).is_ok());
}

#[test]
fn sandbox_with_communication_category() {
    let (es, ev) = test_stores();
    let mut meta = test_metadata();
    meta.category = WitPluginCategory::Communication;
    assert!(PluginSandbox::new(meta, test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev).is_ok());
}

#[test]
fn sandbox_with_information_category() {
    let (es, ev) = test_stores();
    let mut meta = test_metadata();
    meta.category = WitPluginCategory::Information;
    assert!(PluginSandbox::new(meta, test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev).is_ok());
}

#[test]
fn sandbox_with_extension_category() {
    let (es, ev) = test_stores();
    let mut meta = test_metadata();
    meta.category = WitPluginCategory::Extension;
    assert!(PluginSandbox::new(meta, test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party(), es, ev).is_ok());
}
