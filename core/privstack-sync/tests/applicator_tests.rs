use privstack_model::{Entity, EntitySchema, MergeStrategy, PluginDomainHandler};
use privstack_storage::EntityStore;
use privstack_sync::applicator::EventApplicator;
use privstack_types::{EntityId, Event, EventPayload, HybridTimestamp, PeerId};
use serde_json::json;

fn make_store() -> EntityStore {
    EntityStore::open_in_memory().unwrap()
}

fn make_schema(entity_type: &str, strategy: MergeStrategy) -> EntitySchema {
    EntitySchema {
        entity_type: entity_type.to_string(),
        indexed_fields: vec![],
        merge_strategy: strategy,
    }
}

fn make_create_event(entity_id: EntityId, peer_id: PeerId, entity_type: &str, data: &str) -> Event {
    Event::new(
        entity_id,
        peer_id,
        HybridTimestamp::now(),
        EventPayload::EntityCreated {
            entity_type: entity_type.to_string(),
            json_data: data.to_string(),
        },
    )
}

fn make_update_event(entity_id: EntityId, peer_id: PeerId, entity_type: &str, data: &str) -> Event {
    Event::new(
        entity_id,
        peer_id,
        HybridTimestamp::now(),
        EventPayload::EntityUpdated {
            entity_type: entity_type.to_string(),
            json_data: data.to_string(),
        },
    )
}

fn make_delete_event(entity_id: EntityId, peer_id: PeerId, entity_type: &str) -> Event {
    Event::new(
        entity_id,
        peer_id,
        HybridTimestamp::now(),
        EventPayload::EntityDeleted {
            entity_type: entity_type.to_string(),
        },
    )
}

fn make_snapshot_event(entity_id: EntityId, peer_id: PeerId, entity_type: &str, data: &str) -> Event {
    Event::new(
        entity_id,
        peer_id,
        HybridTimestamp::now(),
        EventPayload::FullSnapshot {
            entity_type: entity_type.to_string(),
            json_data: data.to_string(),
        },
    )
}

// ── Construction ─────────────────────────────────────────────────

#[test]
fn applicator_creation() {
    let peer = PeerId::new();
    let _applicator = EventApplicator::new(peer);
}

// ── EntityCreated ────────────────────────────────────────────────

#[test]
fn apply_entity_created_with_schema() {
    let store = make_store();
    let applicator = EventApplicator::new(PeerId::new());
    let eid = EntityId::new();
    let peer = PeerId::new();
    let schema = make_schema("note", MergeStrategy::LwwDocument);

    let event = make_create_event(eid, peer, "note", r#"{"title":"Hello"}"#);
    let result = applicator.apply_event(&event, &store, Some(&schema), None).unwrap();
    assert!(result);

    let entity = store.get_entity(&eid.to_string()).unwrap().unwrap();
    assert_eq!(entity.entity_type, "note");
    assert_eq!(entity.data["title"], "Hello");
    assert_eq!(entity.created_by, peer.to_string());
}

#[test]
fn apply_entity_created_without_schema() {
    let store = make_store();
    let applicator = EventApplicator::new(PeerId::new());
    let eid = EntityId::new();
    let peer = PeerId::new();

    let event = make_create_event(eid, peer, "unknown", r#"{"foo":"bar"}"#);
    let result = applicator.apply_event(&event, &store, None, None).unwrap();
    assert!(result);

    let entity = store.get_entity(&eid.to_string()).unwrap().unwrap();
    assert_eq!(entity.entity_type, "unknown");
    assert_eq!(entity.data["foo"], "bar");
}

#[test]
fn apply_entity_created_invalid_json() {
    let store = make_store();
    let applicator = EventApplicator::new(PeerId::new());
    let eid = EntityId::new();

    let event = make_create_event(eid, PeerId::new(), "note", "not json");
    let result = applicator.apply_event(&event, &store, None, None);
    assert!(result.is_err());
}

// ── EntityUpdated ────────────────────────────────────────────────

#[test]
fn apply_entity_updated_creates_when_missing() {
    let store = make_store();
    let applicator = EventApplicator::new(PeerId::new());
    let eid = EntityId::new();

    // No entity exists yet — update should create it
    let event = make_update_event(eid, PeerId::new(), "note", r#"{"title":"New"}"#);
    let result = applicator.apply_event(&event, &store, None, None).unwrap();
    assert!(result);

    let entity = store.get_entity(&eid.to_string()).unwrap().unwrap();
    assert_eq!(entity.data["title"], "New");
}

#[test]
fn apply_entity_updated_merges_with_existing() {
    let store = make_store();
    let applicator = EventApplicator::new(PeerId::new());
    let eid = EntityId::new();
    let schema = make_schema("note", MergeStrategy::LwwDocument);

    // Create the entity first
    let create = make_create_event(eid, PeerId::new(), "note", r#"{"title":"Old"}"#);
    applicator.apply_event(&create, &store, Some(&schema), None).unwrap();

    // Now update — remote is newer (events use HybridTimestamp::now())
    let update = make_update_event(eid, PeerId::new(), "note", r#"{"title":"Updated"}"#);
    let result = applicator.apply_event(&update, &store, Some(&schema), None).unwrap();
    assert!(result);

    let entity = store.get_entity(&eid.to_string()).unwrap().unwrap();
    assert_eq!(entity.data["title"], "Updated");
}

// ── EntityDeleted ────────────────────────────────────────────────

#[test]
fn apply_entity_deleted() {
    let store = make_store();
    let applicator = EventApplicator::new(PeerId::new());
    let eid = EntityId::new();

    // Create then delete
    let create = make_create_event(eid, PeerId::new(), "note", r#"{"title":"Doomed"}"#);
    applicator.apply_event(&create, &store, None, None).unwrap();

    let delete = make_delete_event(eid, PeerId::new(), "note");
    let result = applicator.apply_event(&delete, &store, None, None).unwrap();
    assert!(result);

    let entity = store.get_entity(&eid.to_string()).unwrap();
    assert!(entity.is_none());
}

// ── FullSnapshot ─────────────────────────────────────────────────

#[test]
fn apply_full_snapshot_creates_entity() {
    let store = make_store();
    let applicator = EventApplicator::new(PeerId::new());
    let eid = EntityId::new();

    let event = make_snapshot_event(eid, PeerId::new(), "note", r#"{"title":"Snapshot"}"#);
    let result = applicator.apply_event(&event, &store, None, None).unwrap();
    assert!(result);

    let entity = store.get_entity(&eid.to_string()).unwrap().unwrap();
    assert_eq!(entity.data["title"], "Snapshot");
}

#[test]
fn apply_full_snapshot_merges_with_existing() {
    let store = make_store();
    let applicator = EventApplicator::new(PeerId::new());
    let eid = EntityId::new();
    let schema = make_schema("note", MergeStrategy::LwwDocument);

    let create = make_create_event(eid, PeerId::new(), "note", r#"{"title":"First"}"#);
    applicator.apply_event(&create, &store, Some(&schema), None).unwrap();

    let snapshot = make_snapshot_event(eid, PeerId::new(), "note", r#"{"title":"Snapshot"}"#);
    applicator.apply_event(&snapshot, &store, Some(&schema), None).unwrap();

    let entity = store.get_entity(&eid.to_string()).unwrap().unwrap();
    assert_eq!(entity.data["title"], "Snapshot");
}

// ── Merge strategies ─────────────────────────────────────────────

#[test]
fn merge_lww_document_remote_wins() {
    let applicator = EventApplicator::new(PeerId::new());
    let local = Entity {
        id: "e1".into(),
        entity_type: "note".into(),
        data: json!({"title": "old"}),
        created_at: 1000,
        modified_at: 1000,
        created_by: "peer-a".into(),
    };
    let remote = Entity {
        id: "e1".into(),
        entity_type: "note".into(),
        data: json!({"title": "new"}),
        created_at: 1000,
        modified_at: 2000,
        created_by: "peer-b".into(),
    };

    let schema = make_schema("note", MergeStrategy::LwwDocument);
    let merged = applicator.merge_entities(&local, &remote, Some(&schema), None);
    assert_eq!(merged.data["title"], "new");
    assert_eq!(merged.modified_at, 2000);
}

#[test]
fn merge_lww_document_local_wins() {
    let applicator = EventApplicator::new(PeerId::new());
    let local = Entity {
        id: "e1".into(),
        entity_type: "note".into(),
        data: json!({"title": "local"}),
        created_at: 1000,
        modified_at: 3000,
        created_by: "peer-a".into(),
    };
    let remote = Entity {
        id: "e1".into(),
        entity_type: "note".into(),
        data: json!({"title": "remote"}),
        created_at: 1000,
        modified_at: 2000,
        created_by: "peer-b".into(),
    };

    let schema = make_schema("note", MergeStrategy::LwwDocument);
    let merged = applicator.merge_entities(&local, &remote, Some(&schema), None);
    assert_eq!(merged.data["title"], "local");
}

#[test]
fn merge_lww_document_tie_goes_to_remote() {
    let applicator = EventApplicator::new(PeerId::new());
    let local = Entity {
        id: "e1".into(),
        entity_type: "note".into(),
        data: json!({"v": "local"}),
        created_at: 1000,
        modified_at: 1000,
        created_by: "a".into(),
    };
    let remote = Entity {
        id: "e1".into(),
        entity_type: "note".into(),
        data: json!({"v": "remote"}),
        created_at: 1000,
        modified_at: 1000,
        created_by: "b".into(),
    };

    let schema = make_schema("note", MergeStrategy::LwwDocument);
    let merged = applicator.merge_entities(&local, &remote, Some(&schema), None);
    assert_eq!(merged.data["v"], "remote");
}

#[test]
fn merge_lww_per_field_remote_newer_merges_fields() {
    let applicator = EventApplicator::new(PeerId::new());
    let local = Entity {
        id: "e1".into(),
        entity_type: "note".into(),
        data: json!({"title": "local-title", "local_only": "preserved"}),
        created_at: 1000,
        modified_at: 1000,
        created_by: "a".into(),
    };
    let remote = Entity {
        id: "e1".into(),
        entity_type: "note".into(),
        data: json!({"title": "remote-title", "body": "remote-body"}),
        created_at: 1000,
        modified_at: 2000,
        created_by: "b".into(),
    };

    let schema = make_schema("note", MergeStrategy::LwwPerField);
    let merged = applicator.merge_entities(&local, &remote, Some(&schema), None);

    // Remote fields overwrite local
    assert_eq!(merged.data["title"], "remote-title");
    assert_eq!(merged.data["body"], "remote-body");
    // Local-only fields preserved
    assert_eq!(merged.data["local_only"], "preserved");
    // Uses remote's modified_at
    assert_eq!(merged.modified_at, 2000);
}

#[test]
fn merge_lww_per_field_local_newer_keeps_local() {
    let applicator = EventApplicator::new(PeerId::new());
    let local = Entity {
        id: "e1".into(),
        entity_type: "note".into(),
        data: json!({"title": "local"}),
        created_at: 1000,
        modified_at: 3000,
        created_by: "a".into(),
    };
    let remote = Entity {
        id: "e1".into(),
        entity_type: "note".into(),
        data: json!({"title": "remote"}),
        created_at: 1000,
        modified_at: 1000,
        created_by: "b".into(),
    };

    let schema = make_schema("note", MergeStrategy::LwwPerField);
    let merged = applicator.merge_entities(&local, &remote, Some(&schema), None);
    assert_eq!(merged.data["title"], "local");
}

#[test]
fn merge_custom_with_handler() {
    struct SumHandler;
    impl PluginDomainHandler for SumHandler {
        fn merge(&self, local: &Entity, remote: &Entity) -> Entity {
            let l = local.data["count"].as_i64().unwrap_or(0);
            let r = remote.data["count"].as_i64().unwrap_or(0);
            let mut result = remote.clone();
            result.data["count"] = json!(l + r);
            result
        }
    }

    let applicator = EventApplicator::new(PeerId::new());
    let local = Entity {
        id: "e1".into(),
        entity_type: "counter".into(),
        data: json!({"count": 10}),
        created_at: 1000,
        modified_at: 1000,
        created_by: "a".into(),
    };
    let remote = Entity {
        id: "e1".into(),
        entity_type: "counter".into(),
        data: json!({"count": 5}),
        created_at: 1000,
        modified_at: 2000,
        created_by: "b".into(),
    };

    let schema = make_schema("counter", MergeStrategy::Custom);
    let handler = SumHandler;
    let merged = applicator.merge_entities(&local, &remote, Some(&schema), Some(&handler));
    assert_eq!(merged.data["count"], 15);
}

#[test]
fn merge_custom_without_handler_falls_back_to_lww() {
    let applicator = EventApplicator::new(PeerId::new());
    let local = Entity {
        id: "e1".into(),
        entity_type: "note".into(),
        data: json!({"v": "local"}),
        created_at: 1000,
        modified_at: 1000,
        created_by: "a".into(),
    };
    let remote = Entity {
        id: "e1".into(),
        entity_type: "note".into(),
        data: json!({"v": "remote"}),
        created_at: 1000,
        modified_at: 2000,
        created_by: "b".into(),
    };

    let schema = make_schema("note", MergeStrategy::Custom);
    // No handler provided — should fallback to LWW
    let merged = applicator.merge_entities(&local, &remote, Some(&schema), None);
    assert_eq!(merged.data["v"], "remote");
}

#[test]
fn merge_no_schema_defaults_to_lww_document() {
    let applicator = EventApplicator::new(PeerId::new());
    let local = Entity {
        id: "e1".into(),
        entity_type: "note".into(),
        data: json!({"v": "local"}),
        created_at: 1000,
        modified_at: 1000,
        created_by: "a".into(),
    };
    let remote = Entity {
        id: "e1".into(),
        entity_type: "note".into(),
        data: json!({"v": "remote"}),
        created_at: 1000,
        modified_at: 2000,
        created_by: "b".into(),
    };

    let merged = applicator.merge_entities(&local, &remote, None, None);
    assert_eq!(merged.data["v"], "remote");
}

// ── create_event helper ──────────────────────────────────────────

#[test]
fn create_event_helper() {
    let eid = EntityId::new();
    let peer = PeerId::new();
    let event = privstack_sync::applicator::create_event(eid, peer, "note", r#"{"title":"x"}"#);

    assert_eq!(event.entity_id, eid);
    assert_eq!(event.peer_id, peer);
    match &event.payload {
        EventPayload::FullSnapshot { entity_type, json_data } => {
            assert_eq!(entity_type, "note");
            assert_eq!(json_data, r#"{"title":"x"}"#);
        }
        _ => panic!("Expected FullSnapshot"),
    }
}
