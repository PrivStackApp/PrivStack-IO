//! Integration tests for host_impl.rs — exercises the WIT host trait
//! implementations on PluginState directly via PluginSandbox.

use privstack_plugin_host::*;
use std::sync::Arc;

fn test_stores() -> (
    Arc<privstack_storage::EntityStore>,
    Arc<privstack_storage::EventStore>,
) {
    let es = privstack_storage::EntityStore::open_in_memory().unwrap();
    let ev = privstack_storage::EventStore::open_in_memory().unwrap();
    (Arc::new(es), Arc::new(ev))
}

fn test_metadata() -> WitPluginMetadata {
    WitPluginMetadata {
        id: "host-test.plugin".into(),
        name: "Host Test Plugin".into(),
        description: "Tests host_impl".into(),
        version: "0.1.0".into(),
        author: "test".into(),
        icon: None,
        navigation_order: 100,
        category: WitPluginCategory::Utility,
        can_disable: true,
        is_experimental: false,
    }
}

fn test_schemas() -> Vec<WitEntitySchema> {
    vec![WitEntitySchema {
        entity_type: "test_note".into(),
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

fn make_sandbox(perms: PermissionSet) -> PluginSandbox {
    let (es, ev) = test_stores();
    PluginSandbox::new(
        test_metadata(),
        test_schemas(),
        perms,
        ResourceLimits::first_party(),
        es,
        ev,
    )
    .unwrap()
}

// ================================================================
// SDK send — entity type scoping
// ================================================================

#[test]
fn sdk_send_allowed_entity_type_succeeds() {
    let sandbox = make_sandbox(PermissionSet::default_first_party());
    let msg = WitSdkMessage {
        action: WitSdkAction::Create,
        entity_type: "test_note".into(),
        entity_id: None,
        payload: Some(r#"{"title":"hello"}"#.into()),
        parameters: vec![],
        source: None,
    };
    let resp = sandbox.handle_sdk_send(&msg);
    assert!(resp.success);
}

#[test]
fn sdk_send_disallowed_entity_type_returns_403() {
    let sandbox = make_sandbox(PermissionSet::default_first_party());
    let msg = WitSdkMessage {
        action: WitSdkAction::Create,
        entity_type: "forbidden".into(),
        entity_id: None,
        payload: Some("{}".into()),
        parameters: vec![],
        source: None,
    };
    let resp = sandbox.handle_sdk_send(&msg);
    assert!(!resp.success);
    assert_eq!(resp.error_code, Some(403));
}

// ================================================================
// Settings host trait (via sandbox wrappers)
// ================================================================

#[test]
fn settings_get_returns_default_when_unset() {
    let sandbox = make_sandbox(PermissionSet::default_first_party());
    assert_eq!(sandbox.handle_settings_get("foo", "bar"), "bar");
}

#[test]
fn settings_set_and_get_roundtrip() {
    let mut sandbox = make_sandbox(PermissionSet::default_first_party());
    sandbox.handle_settings_set("theme", "dark");
    assert_eq!(sandbox.handle_settings_get("theme", "light"), "dark");
}

#[test]
fn settings_remove_clears_value() {
    let mut sandbox = make_sandbox(PermissionSet::default_first_party());
    sandbox.handle_settings_set("key", "value");
    sandbox.handle_settings_remove("key");
    assert_eq!(sandbox.handle_settings_get("key", "fallback"), "fallback");
}

// ================================================================
// Permission checks (vault, linking, dialog)
// ================================================================

#[test]
fn vault_access_denied_without_permission() {
    let sandbox = make_sandbox(PermissionSet::default_first_party());
    assert!(sandbox.check_vault_access().is_err());
}

#[test]
fn vault_access_granted_with_all_permissions() {
    let sandbox = make_sandbox(PermissionSet::all_granted());
    assert!(sandbox.check_vault_access().is_ok());
}

#[test]
fn linking_access_denied_without_permission() {
    let sandbox = make_sandbox(PermissionSet::default_first_party());
    assert!(sandbox.check_linking_access().is_err());
}

#[test]
fn linking_access_granted_with_all_permissions() {
    let sandbox = make_sandbox(PermissionSet::all_granted());
    assert!(sandbox.check_linking_access().is_ok());
}

#[test]
fn dialog_access_denied_without_permission() {
    let sandbox = make_sandbox(PermissionSet::default_first_party());
    assert!(sandbox.check_dialog_access().is_err());
}

#[test]
fn dialog_access_granted_with_all_permissions() {
    let sandbox = make_sandbox(PermissionSet::all_granted());
    assert!(sandbox.check_dialog_access().is_ok());
}

// ================================================================
// State dirty flag
// ================================================================

#[test]
fn state_dirty_flag_lifecycle() {
    let mut sandbox = make_sandbox(PermissionSet::default_first_party());
    assert!(!sandbox.is_state_dirty());

    sandbox.state_mut().state_dirty = true;
    assert!(sandbox.is_state_dirty());

    sandbox.state_mut().state_dirty = false;
    assert!(!sandbox.is_state_dirty());
}

// ================================================================
// Pending navigation
// ================================================================

#[test]
fn pending_navigation_lifecycle() {
    let mut sandbox = make_sandbox(PermissionSet::default_first_party());
    assert!(sandbox.state().pending_navigation.is_none());

    sandbox.state_mut().pending_navigation = Some("target.plugin".into());
    assert_eq!(
        sandbox.state().pending_navigation.as_deref(),
        Some("target.plugin")
    );

    sandbox.state_mut().pending_navigation = Some("__back__".into());
    assert_eq!(
        sandbox.state().pending_navigation.as_deref(),
        Some("__back__")
    );
}

// ================================================================
// Update permissions at runtime
// ================================================================

#[test]
fn update_permissions_grants_new_access() {
    let mut sandbox = make_sandbox(PermissionSet::default_first_party());
    assert!(sandbox.check_vault_access().is_err());

    let mut perms = PermissionSet::default_first_party();
    perms.grant(Permission::Vault);
    sandbox.update_permissions(perms);

    assert!(sandbox.check_vault_access().is_ok());
}

#[test]
fn update_permissions_can_revoke_access() {
    let mut sandbox = make_sandbox(PermissionSet::all_granted());
    assert!(sandbox.check_vault_access().is_ok());

    sandbox.update_permissions(PermissionSet::default_first_party());
    assert!(sandbox.check_vault_access().is_err());
}

// ================================================================
// Multiple entity types
// ================================================================

#[test]
fn multiple_entity_types_scoping() {
    let (es, ev) = test_stores();
    let schemas = vec![
        WitEntitySchema {
            entity_type: "note".into(),
            indexed_fields: vec![WitIndexedField {
                field_path: "/title".into(),
                field_type: WitFieldType::Text,
                searchable: true,
                vector_dim: None,
                enum_options: None,
            }],
            merge_strategy: WitMergeStrategy::LwwPerField,
        },
        WitEntitySchema {
            entity_type: "task".into(),
            indexed_fields: vec![WitIndexedField {
                field_path: "/name".into(),
                field_type: WitFieldType::Text,
                searchable: true,
                vector_dim: None,
                enum_options: None,
            }],
            merge_strategy: WitMergeStrategy::LwwDocument,
        },
    ];

    let sandbox = PluginSandbox::new(
        test_metadata(),
        schemas,
        PermissionSet::default_first_party(),
        ResourceLimits::first_party(),
        es,
        ev,
    )
    .unwrap();

    // Both entity types allowed
    let msg_note = WitSdkMessage {
        action: WitSdkAction::List,
        entity_type: "note".into(),
        entity_id: None,
        payload: None,
        parameters: vec![],
        source: None,
    };
    assert!(sandbox.handle_sdk_send(&msg_note).success);

    let msg_task = WitSdkMessage {
        action: WitSdkAction::List,
        entity_type: "task".into(),
        entity_id: None,
        payload: None,
        parameters: vec![],
        source: None,
    };
    assert!(sandbox.handle_sdk_send(&msg_task).success);

    // Other type denied
    let msg_cal = WitSdkMessage {
        action: WitSdkAction::List,
        entity_type: "calendar".into(),
        entity_id: None,
        payload: None,
        parameters: vec![],
        source: None,
    };
    assert!(!sandbox.handle_sdk_send(&msg_cal).success);
}

// ================================================================
// PluginHostManager integration via load_plugin + route
// ================================================================

#[test]
fn manager_load_route_unload_lifecycle() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::new(es, ev);

    mgr.load_plugin(
        test_metadata(),
        test_schemas(),
        PermissionSet::default_first_party(),
        ResourceLimits::first_party(),
    )
    .unwrap();

    assert!(mgr.is_loaded("host-test.plugin"));
    assert_eq!(mgr.plugin_count(), 1);

    let msg = WitSdkMessage {
        action: WitSdkAction::List,
        entity_type: "test_note".into(),
        entity_id: None,
        payload: None,
        parameters: vec![],
        source: None,
    };
    let resp = mgr.route_sdk_message("host-test.plugin", &msg).unwrap();
    assert!(resp.success);

    mgr.unload_plugin("host-test.plugin").unwrap();
    assert!(!mgr.is_loaded("host-test.plugin"));
    assert_eq!(mgr.plugin_count(), 0);
}

// ================================================================
// WitSdkResponse helpers
// ================================================================

#[test]
fn wit_sdk_response_ok_helper() {
    let resp = WitSdkResponse::ok(Some("data".into()));
    assert!(resp.success);
    assert!(resp.error_code.is_none());
    assert!(resp.error_message.is_none());
    assert_eq!(resp.data.as_deref(), Some("data"));
}

#[test]
fn wit_sdk_response_ok_none() {
    let resp = WitSdkResponse::ok(None);
    assert!(resp.success);
    assert!(resp.data.is_none());
}

#[test]
fn wit_sdk_response_err_helper() {
    let resp = WitSdkResponse::err(500, "server error");
    assert!(!resp.success);
    assert_eq!(resp.error_code, Some(500));
    assert_eq!(resp.error_message.as_deref(), Some("server error"));
}

#[test]
fn wit_sdk_response_permission_denied() {
    let resp = WitSdkResponse::permission_denied("vault");
    assert!(!resp.success);
    assert_eq!(resp.error_code, Some(403));
    assert!(resp.error_message.unwrap().contains("vault"));
}
