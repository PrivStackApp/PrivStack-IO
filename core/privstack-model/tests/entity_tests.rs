use privstack_model::Entity;
use serde_json::json;

fn make_entity(data: serde_json::Value) -> Entity {
    Entity {
        id: "ent-1".to_string(),
        entity_type: "note".to_string(),
        data,
        created_at: 1000,
        modified_at: 2000,
        created_by: "peer-abc".to_string(),
    }
}

// ── Construction & fields ────────────────────────────────────────

#[test]
fn entity_fields_accessible() {
    let e = make_entity(json!({"title": "Hello"}));
    assert_eq!(e.id, "ent-1");
    assert_eq!(e.entity_type, "note");
    assert_eq!(e.created_at, 1000);
    assert_eq!(e.modified_at, 2000);
    assert_eq!(e.created_by, "peer-abc");
}

#[test]
fn entity_data_is_json_value() {
    let e = make_entity(json!({"count": 42, "nested": {"a": true}}));
    assert_eq!(e.data["count"], 42);
    assert_eq!(e.data["nested"]["a"], true);
}

// ── JSON pointer helpers ─────────────────────────────────────────

#[test]
fn get_str_returns_string_field() {
    let e = make_entity(json!({"title": "My Note", "count": 5}));
    assert_eq!(e.get_str("/title"), Some("My Note"));
}

#[test]
fn get_str_returns_none_for_non_string() {
    let e = make_entity(json!({"count": 5}));
    assert_eq!(e.get_str("/count"), None);
}

#[test]
fn get_str_returns_none_for_missing_path() {
    let e = make_entity(json!({"title": "x"}));
    assert_eq!(e.get_str("/nonexistent"), None);
}

#[test]
fn get_str_with_nested_path() {
    let e = make_entity(json!({"meta": {"author": "Alice"}}));
    assert_eq!(e.get_str("/meta/author"), Some("Alice"));
}

#[test]
fn get_bool_returns_boolean_field() {
    let e = make_entity(json!({"done": true, "archived": false}));
    assert_eq!(e.get_bool("/done"), Some(true));
    assert_eq!(e.get_bool("/archived"), Some(false));
}

#[test]
fn get_bool_returns_none_for_non_bool() {
    let e = make_entity(json!({"title": "x"}));
    assert_eq!(e.get_bool("/title"), None);
}

#[test]
fn get_bool_returns_none_for_missing_path() {
    let e = make_entity(json!({}));
    assert_eq!(e.get_bool("/missing"), None);
}

#[test]
fn get_number_returns_numeric_field() {
    let e = make_entity(json!({"price": 19.99, "count": 3}));
    assert_eq!(e.get_number("/price"), Some(19.99));
    assert_eq!(e.get_number("/count"), Some(3.0));
}

#[test]
fn get_number_returns_none_for_non_number() {
    let e = make_entity(json!({"title": "x"}));
    assert_eq!(e.get_number("/title"), None);
}

#[test]
fn get_number_returns_none_for_missing_path() {
    let e = make_entity(json!({}));
    assert_eq!(e.get_number("/missing"), None);
}

// ── Serialization roundtrip ──────────────────────────────────────

#[test]
fn serde_roundtrip() {
    let original = make_entity(json!({
        "title": "Test",
        "tags": ["a", "b"],
        "nested": {"x": 1}
    }));

    let json_str = serde_json::to_string(&original).unwrap();
    let parsed: Entity = serde_json::from_str(&json_str).unwrap();

    assert_eq!(parsed.id, original.id);
    assert_eq!(parsed.entity_type, original.entity_type);
    assert_eq!(parsed.data, original.data);
    assert_eq!(parsed.created_at, original.created_at);
    assert_eq!(parsed.modified_at, original.modified_at);
    assert_eq!(parsed.created_by, original.created_by);
}

#[test]
fn deserialize_from_known_json() {
    let json_str = r#"{
        "id": "abc",
        "entity_type": "task",
        "data": {"done": false},
        "created_at": 100,
        "modified_at": 200,
        "created_by": "peer-1"
    }"#;
    let e: Entity = serde_json::from_str(json_str).unwrap();
    assert_eq!(e.id, "abc");
    assert_eq!(e.entity_type, "task");
    assert_eq!(e.get_bool("/done"), Some(false));
}

// ── Clone ────────────────────────────────────────────────────────

#[test]
fn entity_clone_is_independent() {
    let e = make_entity(json!({"title": "original"}));
    let mut cloned = e.clone();
    cloned.data["title"] = json!("modified");

    assert_eq!(e.get_str("/title"), Some("original"));
    assert_eq!(cloned.get_str("/title"), Some("modified"));
}

// ── Edge cases ───────────────────────────────────────────────────

#[test]
fn entity_with_empty_data() {
    let e = make_entity(json!({}));
    assert_eq!(e.get_str("/anything"), None);
    assert_eq!(e.get_bool("/anything"), None);
    assert_eq!(e.get_number("/anything"), None);
}

#[test]
fn entity_with_null_data() {
    let e = make_entity(json!(null));
    assert_eq!(e.get_str("/anything"), None);
}

#[test]
fn entity_with_array_data() {
    let e = make_entity(json!([1, 2, 3]));
    // JSON pointer /0 accesses array index
    assert_eq!(e.get_number("/0"), Some(1.0));
}

#[test]
fn get_str_with_empty_string_value() {
    let e = make_entity(json!({"title": ""}));
    assert_eq!(e.get_str("/title"), Some(""));
}
