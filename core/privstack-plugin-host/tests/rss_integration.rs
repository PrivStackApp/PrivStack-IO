//! Integration tests verifying RSS plugin metadata/schemas work correctly
//! with the plugin host manager — validates Phase 4a acceptance criteria
//! at the host boundary level.

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

fn rss_metadata() -> WitPluginMetadata {
    WitPluginMetadata {
        id: "privstack.rss".into(),
        name: "RSS".into(),
        description: "RSS/Atom feed reader with reader mode".into(),
        version: "1.0.0".into(),
        author: "PrivStack".into(),
        icon: Some("Rss".into()),
        navigation_order: 350,
        category: WitPluginCategory::Utility,
        can_disable: true,
        is_experimental: false,
    }
}

fn rss_schemas() -> Vec<WitEntitySchema> {
    vec![
        WitEntitySchema {
            entity_type: "feed".into(),
            indexed_fields: vec![
                WitIndexedField {
                    field_path: "/title".into(),
                    field_type: WitFieldType::Text,
                    searchable: true,
                    vector_dim: None,
                    enum_options: None,
                },
                WitIndexedField {
                    field_path: "/url".into(),
                    field_type: WitFieldType::Text,
                    searchable: false,
                    vector_dim: None,
                    enum_options: None,
                },
                WitIndexedField {
                    field_path: "/description".into(),
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
                WitIndexedField {
                    field_path: "/last_fetched".into(),
                    field_type: WitFieldType::DateTime,
                    searchable: false,
                    vector_dim: None,
                    enum_options: None,
                },
                WitIndexedField {
                    field_path: "/created_at".into(),
                    field_type: WitFieldType::DateTime,
                    searchable: false,
                    vector_dim: None,
                    enum_options: None,
                },
            ],
            merge_strategy: WitMergeStrategy::LwwPerField,
        },
        WitEntitySchema {
            entity_type: "feed_item".into(),
            indexed_fields: vec![
                WitIndexedField {
                    field_path: "/title".into(),
                    field_type: WitFieldType::Text,
                    searchable: true,
                    vector_dim: None,
                    enum_options: None,
                },
                WitIndexedField {
                    field_path: "/summary".into(),
                    field_type: WitFieldType::Text,
                    searchable: true,
                    vector_dim: None,
                    enum_options: None,
                },
                WitIndexedField {
                    field_path: "/author".into(),
                    field_type: WitFieldType::Text,
                    searchable: true,
                    vector_dim: None,
                    enum_options: None,
                },
                WitIndexedField {
                    field_path: "/url".into(),
                    field_type: WitFieldType::Text,
                    searchable: false,
                    vector_dim: None,
                    enum_options: None,
                },
                WitIndexedField {
                    field_path: "/feed_id".into(),
                    field_type: WitFieldType::Relation,
                    searchable: false,
                    vector_dim: None,
                    enum_options: None,
                },
                WitIndexedField {
                    field_path: "/read".into(),
                    field_type: WitFieldType::Boolean,
                    searchable: false,
                    vector_dim: None,
                    enum_options: None,
                },
                WitIndexedField {
                    field_path: "/starred".into(),
                    field_type: WitFieldType::Boolean,
                    searchable: false,
                    vector_dim: None,
                    enum_options: None,
                },
                WitIndexedField {
                    field_path: "/published_at".into(),
                    field_type: WitFieldType::DateTime,
                    searchable: false,
                    vector_dim: None,
                    enum_options: None,
                },
                WitIndexedField {
                    field_path: "/fetched_at".into(),
                    field_type: WitFieldType::DateTime,
                    searchable: false,
                    vector_dim: None,
                    enum_options: None,
                },
            ],
            merge_strategy: WitMergeStrategy::LwwPerField,
        },
    ]
}

// ---- Phase 4a Acceptance Criteria Tests ----

/// P4a: RSS plugin loads and returns correct metadata.
#[test]
fn rss_loads_with_correct_metadata() {
    let (es, ev) = test_stores();
    let mut manager = PluginHostManager::new(es, ev);

    manager
        .load_plugin(
            rss_metadata(),
            rss_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

    let plugin = manager.get_plugin("privstack.rss").unwrap();
    assert_eq!(plugin.metadata.id, "privstack.rss");
    assert_eq!(plugin.metadata.name, "RSS");
    assert_eq!(plugin.metadata.version, "1.0.0");
    assert_eq!(plugin.metadata.navigation_order, 350);
}

/// P4a: Entity schemas register `feed` and `feed_item` types.
#[test]
fn rss_registers_both_entity_types() {
    let (es, ev) = test_stores();
    let mut manager = PluginHostManager::new(es, ev);

    manager
        .load_plugin(
            rss_metadata(),
            rss_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

    let plugin = manager.get_plugin("privstack.rss").unwrap();
    let types = plugin.declared_entity_types();
    assert!(types.contains("feed"));
    assert!(types.contains("feed_item"));
    assert_eq!(types.len(), 2);
}

/// P4a: Full CRUD lifecycle — create, read, update, delete on `feed`.
#[test]
fn rss_crud_on_declared_entity_type() {
    let (es, ev) = test_stores();
    let mut manager = PluginHostManager::new(es, ev);

    manager
        .load_plugin(
            rss_metadata(),
            rss_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

    // Create
    let create_msg = WitSdkMessage {
        action: WitSdkAction::Create,
        entity_type: "feed".into(),
        entity_id: Some("f1".into()),
        payload: Some(r#"{"title":"Test Feed","url":"https://example.com"}"#.into()),
        parameters: vec![],
        source: Some("rss-plugin:test".into()),
    };
    let resp = manager.route_sdk_message("privstack.rss", &create_msg).unwrap();
    assert!(resp.success);

    // Read
    let read_msg = WitSdkMessage {
        action: WitSdkAction::Read,
        entity_type: "feed".into(),
        entity_id: Some("f1".into()),
        payload: None,
        parameters: vec![],
        source: None,
    };
    let resp = manager.route_sdk_message("privstack.rss", &read_msg).unwrap();
    assert!(resp.success);

    // Update
    let update_msg = WitSdkMessage {
        action: WitSdkAction::Update,
        entity_type: "feed".into(),
        entity_id: Some("f1".into()),
        payload: Some(r#"{"title":"Updated Feed"}"#.into()),
        parameters: vec![],
        source: None,
    };
    let resp = manager.route_sdk_message("privstack.rss", &update_msg).unwrap();
    assert!(resp.success);

    // Delete
    let delete_msg = WitSdkMessage {
        action: WitSdkAction::Delete,
        entity_type: "feed".into(),
        entity_id: Some("f1".into()),
        payload: None,
        parameters: vec![],
        source: None,
    };
    let resp = manager.route_sdk_message("privstack.rss", &delete_msg).unwrap();
    assert!(resp.success);
}

/// P4a: Entity-type scoping enforced — RSS plugin cannot CRUD entity types it didn't declare.
#[test]
fn rss_entity_type_scoping_enforced() {
    let (es, ev) = test_stores();
    let mut manager = PluginHostManager::new(es, ev);

    manager
        .load_plugin(
            rss_metadata(),
            rss_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

    // Try to read a 'note' entity — should be denied
    let msg = WitSdkMessage {
        action: WitSdkAction::Read,
        entity_type: "note".into(),
        entity_id: Some("n1".into()),
        payload: None,
        parameters: vec![],
        source: None,
    };
    let resp = manager.route_sdk_message("privstack.rss", &msg).unwrap();
    assert!(!resp.success);
    assert_eq!(resp.error_code, Some(403));

    // Try 'task' — also denied
    let msg2 = WitSdkMessage {
        entity_type: "task".into(),
        ..msg
    };
    let resp2 = manager.route_sdk_message("privstack.rss", &msg2).unwrap();
    assert!(!resp2.success);

    // But 'feed' works
    let msg3 = WitSdkMessage {
        entity_type: "feed".into(),
        ..msg2
    };
    let resp3 = manager.route_sdk_message("privstack.rss", &msg3).unwrap();
    assert!(resp3.success);

    // And 'feed_item' works
    let msg4 = WitSdkMessage {
        entity_type: "feed_item".into(),
        ..msg3
    };
    let resp4 = manager.route_sdk_message("privstack.rss", &msg4).unwrap();
    assert!(resp4.success);
}

/// P4a: Permission denied for unconfigured Tier 2/3 capabilities.
#[test]
fn rss_permission_denied_for_vault() {
    let (es, ev) = test_stores();
    let mut manager = PluginHostManager::new(es, ev);

    manager
        .load_plugin(
            rss_metadata(),
            rss_schemas(),
            PermissionSet::default_first_party(), // No vault permission
            ResourceLimits::first_party(),
        )
        .unwrap();

    let plugin = manager.get_plugin("privstack.rss").unwrap();
    assert!(plugin.check_vault_access().is_err());
    assert!(plugin.check_linking_access().is_err());
    assert!(plugin.check_dialog_access().is_err());
}

/// P4a: Resource limits are configured correctly.
#[test]
fn rss_resource_limits() {
    let (es, ev) = test_stores();
    let mut manager = PluginHostManager::new(es, ev);

    manager
        .load_plugin(
            rss_metadata(),
            rss_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

    let plugin = manager.get_plugin("privstack.rss").unwrap();
    assert_eq!(plugin.resource_limits.max_memory_bytes, 64 * 1024 * 1024);
    assert!(plugin.resource_limits.fuel_per_call > 0);
    assert_eq!(plugin.resource_limits.shutdown_deadline_ms, 2000);
}

/// P4a: Schema conversion to core model works for both entity types.
#[test]
fn rss_schemas_convert_to_core() {
    for schema in rss_schemas() {
        let core = schema.to_core_schema().unwrap();
        assert!(!core.entity_type.is_empty());
        assert!(!core.indexed_fields.is_empty());
    }
}

/// P4a: Navigation items include RSS with correct ordering.
#[test]
fn rss_appears_in_navigation() {
    let (es, ev) = test_stores();
    let mut manager = PluginHostManager::new(es, ev);

    manager
        .load_plugin(
            rss_metadata(),
            rss_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

    let nav = manager.get_navigation_items();
    assert_eq!(nav.len(), 1);
    assert_eq!(nav[0].display_name, "RSS");
    assert_eq!(nav[0].order, 350);
}

/// P4a: Multiple plugins can coexist with entity-type isolation.
#[test]
fn rss_isolated_from_other_plugins() {
    let (es, ev) = test_stores();
    let mut manager = PluginHostManager::new(es, ev);

    // Load RSS
    manager
        .load_plugin(
            rss_metadata(),
            rss_schemas(),
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

    // Load a hypothetical Notes plugin
    manager
        .load_plugin(
            WitPluginMetadata {
                id: "privstack.notes".into(),
                name: "Notes".into(),
                description: "Note taking".into(),
                version: "1.0.0".into(),
                author: "PrivStack".into(),
                icon: Some("FileText".into()),
                navigation_order: 100,
                category: WitPluginCategory::Productivity,
                can_disable: false,
                is_experimental: false,
            },
            vec![WitEntitySchema {
                entity_type: "note".into(),
                indexed_fields: vec![WitIndexedField {
                    field_path: "/title".into(),
                    field_type: WitFieldType::Text,
                    searchable: true,
                    vector_dim: None,
                    enum_options: None,
                }],
                merge_strategy: WitMergeStrategy::LwwPerField,
            }],
            PermissionSet::default_first_party(),
            ResourceLimits::first_party(),
        )
        .unwrap();

    assert_eq!(manager.plugin_count(), 2);

    // RSS can't touch notes
    let msg = WitSdkMessage {
        action: WitSdkAction::Read,
        entity_type: "note".into(),
        entity_id: Some("n1".into()),
        payload: None,
        parameters: vec![],
        source: None,
    };
    let resp = manager.route_sdk_message("privstack.rss", &msg).unwrap();
    assert!(!resp.success);

    // Notes can't touch feeds
    let msg2 = WitSdkMessage {
        entity_type: "feed".into(),
        ..msg
    };
    let resp2 = manager.route_sdk_message("privstack.notes", &msg2).unwrap();
    assert!(!resp2.success);
}

/// P4a: Policy enforcement blocks unauthorized plugins.
#[test]
fn rss_blocked_by_allowlist_policy() {
    let (es, ev) = test_stores();
    let config = privstack_plugin_host::PolicyConfig {
        mode: privstack_plugin_host::PolicyMode::Allowlist,
        allowed_plugin_ids: vec!["privstack.notes".to_string()], // RSS not in list
        ..Default::default()
    };
    let policy = privstack_plugin_host::PolicyEngine::with_config(config);
    let mut manager = PluginHostManager::with_policy(es, ev, policy);

    let result = manager.load_plugin(
        rss_metadata(),
        rss_schemas(),
        PermissionSet::default_first_party(),
        ResourceLimits::first_party(),
    );
    assert!(result.is_err());
}
