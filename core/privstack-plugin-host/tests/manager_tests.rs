use privstack_plugin_host::*;
use std::sync::Arc;

fn test_stores() -> (Arc<privstack_storage::EntityStore>, Arc<privstack_storage::EventStore>) {
    let es = privstack_storage::EntityStore::open_in_memory().unwrap();
    let ev = privstack_storage::EventStore::open_in_memory().unwrap();
    (Arc::new(es), Arc::new(ev))
}

fn test_metadata(id: &str) -> WitPluginMetadata {
    WitPluginMetadata {
        id: id.into(),
        name: format!("Test {}", id),
        description: "test".into(),
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
fn load_and_list_plugins() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    mgr.load_plugin(test_metadata("p2"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert_eq!(mgr.plugin_count(), 2);
    assert!(mgr.is_loaded("p1"));
    assert!(mgr.is_loaded("p2"));
}

#[test]
fn duplicate_load_rejected() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    let result = mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party());
    assert!(result.is_err());
}

#[test]
fn unload_plugin() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert!(mgr.is_loaded("p1"));
    mgr.unload_plugin("p1").unwrap();
    assert!(!mgr.is_loaded("p1"));
}

#[test]
fn sdk_message_routing_enforces_entity_type() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    let msg = WitSdkMessage { action: WitSdkAction::List, entity_type: "test_item".into(), entity_id: None, payload: None, parameters: vec![], source: None };
    let resp = mgr.route_sdk_message("p1", &msg).unwrap();
    assert!(resp.success);
    let bad_msg = WitSdkMessage { entity_type: "other_type".into(), ..msg };
    let resp = mgr.route_sdk_message("p1", &bad_msg).unwrap();
    assert!(!resp.success);
}

#[test]
fn navigation_items_sorted_by_order() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    let mut m1 = test_metadata("p1"); m1.navigation_order = 200;
    let mut m2 = test_metadata("p2"); m2.navigation_order = 100;
    mgr.load_plugin(m1, test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    mgr.load_plugin(m2, test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    let items = mgr.get_navigation_items();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].id, "p2");
    assert_eq!(items[1].id, "p1");
}

#[test]
fn policy_blocks_disallowed_plugin() {
    let (es, ev) = test_stores();
    let policy = PolicyEngine::with_config(PolicyConfig {
        mode: PolicyMode::Allowlist,
        allowed_plugin_ids: vec!["allowed.plugin".into()],
        ..PolicyConfig::default()
    });
    let mut mgr = PluginHostManager::with_policy(es, ev, policy);
    let result = mgr.load_plugin(test_metadata("blocked.plugin"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party());
    assert!(matches!(result, Err(PluginHostError::PolicyDenied(_))));
}

#[test]
fn send_command_metadata_only_fails() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert!(mgr.send_command("p1", "do_thing", "{}").is_err());
}

#[test]
fn send_command_plugin_not_found() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    assert!(matches!(mgr.send_command("nonexistent", "cmd", "{}"), Err(PluginHostError::PluginNotFound(_))));
}

#[test]
fn get_view_state_metadata_only_fails() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert!(mgr.get_view_state("p1").is_err());
}

#[test]
fn get_view_data_metadata_only_fails() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert!(mgr.get_view_data("p1").is_err());
}

#[test]
fn get_view_state_plugin_not_found() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    assert!(matches!(mgr.get_view_state("missing"), Err(PluginHostError::PluginNotFound(_))));
}

#[test]
fn initialize_plugin_metadata_only_fails() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert!(mgr.initialize_plugin("p1").is_err());
}

#[test]
fn activate_plugin_metadata_only_fails() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert!(mgr.activate_plugin("p1").is_err());
}

#[test]
fn initialize_plugin_not_found() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    assert!(matches!(mgr.initialize_plugin("missing"), Err(PluginHostError::PluginNotFound(_))));
}

#[test]
fn notify_navigated_to_metadata_only_fails() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert!(mgr.notify_navigated_to("p1").is_err());
}

#[test]
fn notify_navigated_from_metadata_only_fails() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert!(mgr.notify_navigated_from("p1").is_err());
}

#[test]
fn notify_navigated_to_not_found() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    assert!(matches!(mgr.notify_navigated_to("missing"), Err(PluginHostError::PluginNotFound(_))));
}

#[test]
fn query_all_linkable_items_no_providers() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    let items = mgr.query_all_linkable_items("test", 10);
    assert!(items.is_empty());
}

#[test]
fn get_all_link_providers_empty_for_metadata_only() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert!(mgr.get_all_link_providers().is_empty());
}

#[test]
fn get_commands_returns_empty_vec() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert!(mgr.get_commands("p1").unwrap().is_empty());
}

#[test]
fn get_commands_not_found() {
    let (es, ev) = test_stores();
    let mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    assert!(matches!(mgr.get_commands("missing"), Err(PluginHostError::PluginNotFound(_))));
}

#[test]
fn get_all_commands_returns_entries_for_each_plugin() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    mgr.load_plugin(test_metadata("p2"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert_eq!(mgr.get_all_commands().len(), 2);
}

#[test]
fn update_plugin_permissions_success() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert!(mgr.get_plugin("p1").unwrap().check_vault_access().is_err());
    mgr.update_plugin_permissions("p1", PermissionSet::all_granted()).unwrap();
    assert!(mgr.get_plugin("p1").unwrap().check_vault_access().is_ok());
}

#[test]
fn update_plugin_permissions_not_found() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    assert!(matches!(mgr.update_plugin_permissions("missing", PermissionSet::all_granted()), Err(PluginHostError::PluginNotFound(_))));
}

#[test]
fn fetch_url_permission_denied_without_network() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert!(matches!(mgr.fetch_url_for_plugin("p1", "https://example.com"), Err(PluginHostError::PermissionDenied { .. })));
}

#[test]
fn fetch_url_plugin_not_found() {
    let (es, ev) = test_stores();
    let mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    assert!(matches!(mgr.fetch_url_for_plugin("missing", "https://example.com"), Err(PluginHostError::PluginNotFound(_))));
}

#[test]
fn unload_nonexistent_plugin() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    assert!(matches!(mgr.unload_plugin("missing"), Err(PluginHostError::PluginNotFound(_))));
}

#[test]
fn list_plugins_returns_all_metadata() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    let plugins = mgr.list_plugins();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].id, "p1");
}

#[test]
fn route_sdk_message_plugin_not_found() {
    let (es, ev) = test_stores();
    let mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    let msg = WitSdkMessage { action: WitSdkAction::List, entity_type: "test".into(), entity_id: None, payload: None, parameters: vec![], source: None };
    assert!(matches!(mgr.route_sdk_message("missing", &msg), Err(PluginHostError::PluginNotFound(_))));
}

#[test]
fn policy_engine_accessor() {
    let (es, ev) = test_stores();
    let mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    assert!(mgr.policy_engine().is_plugin_allowed("anything", None));
}

#[test]
fn plugin_count_starts_at_zero() {
    let (es, ev) = test_stores();
    let mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    assert_eq!(mgr.plugin_count(), 0);
    assert!(!mgr.is_loaded("anything"));
}

#[test]
fn navigate_to_item_capability_not_supported() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert!(matches!(mgr.navigate_to_item("p1", "item-1"), Err(PluginHostError::CapabilityNotSupported { .. })));
}

#[test]
fn navigate_to_item_plugin_not_found() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    assert!(matches!(mgr.navigate_to_item("missing", "item-1"), Err(PluginHostError::PluginNotFound(_))));
}

#[test]
fn get_entity_view_data_capability_not_supported() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert!(matches!(mgr.get_entity_view_data("p1", "item-1"), Err(PluginHostError::CapabilityNotSupported { .. })));
}

#[test]
fn get_entity_view_data_plugin_not_found() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    assert!(matches!(mgr.get_entity_view_data("missing", "item-1"), Err(PluginHostError::PluginNotFound(_))));
}

#[test]
fn get_plugin_metrics_success() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    let metrics = mgr.get_plugin_metrics("p1").unwrap();
    assert_eq!(metrics.memory_limit_bytes, 64 * 1024 * 1024);
    assert_eq!(metrics.fuel_budget_per_call, 1_000_000_000);
    assert_eq!(metrics.entity_count, 0);
}

#[test]
fn get_plugin_metrics_not_found() {
    let (es, ev) = test_stores();
    let mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    assert!(matches!(mgr.get_plugin_metrics("missing"), Err(PluginHostError::PluginNotFound(_))));
}

#[test]
fn get_all_plugin_metrics_returns_all() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    mgr.load_plugin(test_metadata("p2"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::third_party()).unwrap();
    let all_metrics = mgr.get_all_plugin_metrics();
    assert_eq!(all_metrics.len(), 2);
    let p2_metrics = all_metrics.iter().find(|(id, _)| id == "p2").unwrap();
    assert_eq!(p2_metrics.1.memory_limit_bytes, 32 * 1024 * 1024);
}

#[test]
fn get_all_plugin_metrics_empty() {
    let (es, ev) = test_stores();
    let mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    assert!(mgr.get_all_plugin_metrics().is_empty());
}

#[test]
fn get_plugin_success() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert_eq!(mgr.get_plugin("p1").unwrap().plugin_id(), "p1");
}

#[test]
fn get_plugin_not_found() {
    let (es, ev) = test_stores();
    let mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    assert!(matches!(mgr.get_plugin("missing"), Err(PluginHostError::PluginNotFound(_))));
}

#[test]
fn get_plugin_mut_success() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    assert_eq!(mgr.get_plugin_mut("p1").unwrap().plugin_id(), "p1");
}

#[test]
fn get_plugin_mut_not_found() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    assert!(matches!(mgr.get_plugin_mut("missing"), Err(PluginHostError::PluginNotFound(_))));
}

#[test]
fn load_plugin_from_wasm_nonexistent_file() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    assert!(mgr.load_plugin_from_wasm(std::path::Path::new("/nonexistent/plugin.wasm"), PermissionSet::default_first_party(), ResourceLimits::first_party()).is_err());
}

#[test]
fn navigation_items_include_icons() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    let mut m1 = test_metadata("p1");
    m1.icon = Some("FileText".into());
    mgr.load_plugin(m1, test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    let items = mgr.get_navigation_items();
    assert_eq!(items[0].icon.as_deref(), Some("FileText"));
    assert!(items[0].subtitle.is_none());
    assert!(items[0].tooltip.is_none());
    assert!(!items[0].show_badge);
    assert_eq!(items[0].badge_count, 0);
    assert!(items[0].shortcut_hint.is_none());
}

#[test]
fn with_policy_uses_custom_policy() {
    let (es, ev) = test_stores();
    let policy = PolicyEngine::with_config(PolicyConfig {
        mode: PolicyMode::Allowlist,
        allowed_plugin_ids: vec!["allowed".into()],
        ..PolicyConfig::default()
    });
    let mut mgr = PluginHostManager::with_policy(es, ev, policy);
    mgr.load_plugin(test_metadata("allowed"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party()).unwrap();
    let result = mgr.load_plugin(test_metadata("blocked"), test_schemas(), PermissionSet::default_first_party(), ResourceLimits::first_party());
    assert!(matches!(result, Err(PluginHostError::PolicyDenied(_))));
}

#[test]
fn fetch_url_network_error() {
    let (es, ev) = test_stores();
    let mut mgr = PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()));
    mgr.load_plugin(test_metadata("p1"), test_schemas(), PermissionSet::all_granted(), ResourceLimits::first_party()).unwrap();
    let result = mgr.fetch_url_for_plugin("p1", "http://[::1]:1/nonexistent");
    assert!(matches!(result, Err(PluginHostError::NetworkError(_))));
}
