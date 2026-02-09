use privstack_model::{Entity, PluginDomainHandler};
use serde_json::json;

fn make_entity(id: &str, modified_at: i64, data: serde_json::Value) -> Entity {
    Entity {
        id: id.to_string(),
        entity_type: "test".to_string(),
        data,
        created_at: 1000,
        modified_at,
        created_by: "peer-1".to_string(),
    }
}

// ── Default implementations ──────────────────────────────────────

struct NoOpHandler;
impl PluginDomainHandler for NoOpHandler {}

#[test]
fn default_validate_accepts_any_entity() {
    let handler = NoOpHandler;
    let entity = make_entity("e1", 1000, json!({"anything": "goes"}));
    assert!(handler.validate(&entity).is_ok());
}

#[test]
fn default_validate_accepts_empty_data() {
    let handler = NoOpHandler;
    let entity = make_entity("e1", 1000, json!({}));
    assert!(handler.validate(&entity).is_ok());
}

#[test]
fn default_on_after_load_is_noop() {
    let handler = NoOpHandler;
    let mut entity = make_entity("e1", 1000, json!({"title": "before"}));
    handler.on_after_load(&mut entity);
    assert_eq!(entity.get_str("/title"), Some("before"));
}

#[test]
fn default_merge_picks_higher_modified_at() {
    let handler = NoOpHandler;
    let local = make_entity("e1", 1000, json!({"v": "local"}));
    let remote = make_entity("e1", 2000, json!({"v": "remote"}));

    let result = handler.merge(&local, &remote);
    assert_eq!(result.get_str("/v"), Some("remote"));
    assert_eq!(result.modified_at, 2000);
}

#[test]
fn default_merge_picks_local_when_newer() {
    let handler = NoOpHandler;
    let local = make_entity("e1", 3000, json!({"v": "local"}));
    let remote = make_entity("e1", 1000, json!({"v": "remote"}));

    let result = handler.merge(&local, &remote);
    assert_eq!(result.get_str("/v"), Some("local"));
    assert_eq!(result.modified_at, 3000);
}

#[test]
fn default_merge_picks_remote_on_tie() {
    let handler = NoOpHandler;
    let local = make_entity("e1", 1000, json!({"v": "local"}));
    let remote = make_entity("e1", 1000, json!({"v": "remote"}));

    // >= means remote wins on tie
    let result = handler.merge(&local, &remote);
    assert_eq!(result.get_str("/v"), Some("remote"));
}

// ── Custom handler implementations ──────────────────────────────

struct ValidatingHandler;
impl PluginDomainHandler for ValidatingHandler {
    fn validate(&self, entity: &Entity) -> Result<(), String> {
        let title = entity
            .get_str("/title")
            .ok_or_else(|| "title is required".to_string())?;
        if title.is_empty() {
            return Err("title cannot be empty".to_string());
        }
        if title.len() > 100 {
            return Err("title too long (max 100)".to_string());
        }
        Ok(())
    }
}

#[test]
fn custom_validate_rejects_missing_title() {
    let handler = ValidatingHandler;
    let entity = make_entity("e1", 1000, json!({"body": "no title"}));
    let err = handler.validate(&entity).unwrap_err();
    assert_eq!(err, "title is required");
}

#[test]
fn custom_validate_rejects_empty_title() {
    let handler = ValidatingHandler;
    let entity = make_entity("e1", 1000, json!({"title": ""}));
    let err = handler.validate(&entity).unwrap_err();
    assert_eq!(err, "title cannot be empty");
}

#[test]
fn custom_validate_rejects_long_title() {
    let handler = ValidatingHandler;
    let long = "x".repeat(101);
    let entity = make_entity("e1", 1000, json!({"title": long}));
    let err = handler.validate(&entity).unwrap_err();
    assert_eq!(err, "title too long (max 100)");
}

#[test]
fn custom_validate_accepts_valid_title() {
    let handler = ValidatingHandler;
    let entity = make_entity("e1", 1000, json!({"title": "Good title"}));
    assert!(handler.validate(&entity).is_ok());
}

struct EnrichingHandler;
impl PluginDomainHandler for EnrichingHandler {
    fn on_after_load(&self, entity: &mut Entity) {
        // Compute a derived "display_name" field
        let first = entity.get_str("/first_name").unwrap_or("").to_string();
        let last = entity.get_str("/last_name").unwrap_or("").to_string();
        let display = format!("{} {}", first, last).trim().to_string();
        entity.data["display_name"] = json!(display);
    }
}

#[test]
fn custom_on_after_load_enriches_entity() {
    let handler = EnrichingHandler;
    let mut entity = make_entity("e1", 1000, json!({
        "first_name": "Alice",
        "last_name": "Smith"
    }));
    handler.on_after_load(&mut entity);
    assert_eq!(entity.get_str("/display_name"), Some("Alice Smith"));
}

#[test]
fn custom_on_after_load_handles_missing_fields() {
    let handler = EnrichingHandler;
    let mut entity = make_entity("e1", 1000, json!({}));
    handler.on_after_load(&mut entity);
    assert_eq!(entity.get_str("/display_name"), Some(""));
}

struct CustomMergeHandler;
impl PluginDomainHandler for CustomMergeHandler {
    fn merge(&self, local: &Entity, remote: &Entity) -> Entity {
        // Custom merge: sum the "balance" fields instead of LWW
        let local_balance = local.get_number("/balance").unwrap_or(0.0);
        let remote_balance = remote.get_number("/balance").unwrap_or(0.0);

        let mut result = if remote.modified_at >= local.modified_at {
            remote.clone()
        } else {
            local.clone()
        };
        result.data["balance"] = json!(local_balance + remote_balance);
        result
    }
}

#[test]
fn custom_merge_sums_balances() {
    let handler = CustomMergeHandler;
    let local = make_entity("e1", 1000, json!({"balance": 100.0, "name": "Account"}));
    let remote = make_entity("e1", 2000, json!({"balance": 50.0, "name": "Updated"}));

    let result = handler.merge(&local, &remote);
    assert_eq!(result.get_number("/balance"), Some(150.0));
    // Name comes from the newer (remote) entity
    assert_eq!(result.get_str("/name"), Some("Updated"));
}

// ── Trait object safety ──────────────────────────────────────────

#[test]
fn handler_works_as_trait_object() {
    let handler: Box<dyn PluginDomainHandler> = Box::new(ValidatingHandler);
    let entity = make_entity("e1", 1000, json!({"title": "Works"}));
    assert!(handler.validate(&entity).is_ok());
}

#[test]
fn handler_trait_object_in_vec() {
    let handlers: Vec<Box<dyn PluginDomainHandler>> = vec![
        Box::new(NoOpHandler),
        Box::new(ValidatingHandler),
    ];

    let entity = make_entity("e1", 1000, json!({"title": "Test"}));
    for h in &handlers {
        assert!(h.validate(&entity).is_ok());
    }
}
