use privstack_types::{EntityId, Event, EventId, EventPayload, HybridTimestamp, PeerId};
use std::str::FromStr;

// ── EventId ───────────────────────────────────────────────────────

#[test]
fn event_id_unique() {
    let a = EventId::new();
    let b = EventId::new();
    assert_ne!(a, b);
}

#[test]
fn event_id_default_unique() {
    let a = EventId::default();
    let b = EventId::default();
    assert_ne!(a, b);
}

#[test]
fn event_id_display_roundtrip() {
    let id = EventId::new();
    let s = id.to_string();
    let parsed: EventId = s.parse().unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn event_id_from_str_invalid() {
    assert!(EventId::from_str("bad").is_err());
}

#[test]
fn event_id_serde_roundtrip() {
    let id = EventId::new();
    let json = serde_json::to_string(&id).unwrap();
    let parsed: EventId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn event_id_hash_eq() {
    use std::collections::HashSet;
    let id = EventId::new();
    let mut set = HashSet::new();
    set.insert(id);
    set.insert(id);
    assert_eq!(set.len(), 1);
}

// ── EventPayload serde ───────────────────────────────────────────

#[test]
fn payload_entity_created_serde() {
    let payload = EventPayload::EntityCreated {
        entity_type: "note".into(),
        json_data: r#"{"title":"hi"}"#.into(),
    };
    let json = serde_json::to_string(&payload).unwrap();
    let parsed: EventPayload = serde_json::from_str(&json).unwrap();
    assert_eq!(payload, parsed);
}

#[test]
fn payload_entity_updated_serde() {
    let payload = EventPayload::EntityUpdated {
        entity_type: "task".into(),
        json_data: r#"{"done":true}"#.into(),
    };
    let json = serde_json::to_string(&payload).unwrap();
    let parsed: EventPayload = serde_json::from_str(&json).unwrap();
    assert_eq!(payload, parsed);
}

#[test]
fn payload_entity_deleted_serde() {
    let payload = EventPayload::EntityDeleted {
        entity_type: "note".into(),
    };
    let json = serde_json::to_string(&payload).unwrap();
    let parsed: EventPayload = serde_json::from_str(&json).unwrap();
    assert_eq!(payload, parsed);
}

#[test]
fn payload_full_snapshot_serde() {
    let payload = EventPayload::FullSnapshot {
        entity_type: "calendar".into(),
        json_data: r#"{"date":"2025-01-01"}"#.into(),
    };
    let json = serde_json::to_string(&payload).unwrap();
    let parsed: EventPayload = serde_json::from_str(&json).unwrap();
    assert_eq!(payload, parsed);
}

// ── Event factories ──────────────────────────────────────────────

#[test]
fn event_entity_created() {
    let eid = EntityId::new();
    let pid = PeerId::new();
    let event = Event::entity_created(eid, pid, "bookmark", r#"{"url":"x"}"#);

    assert_eq!(event.entity_id, eid);
    assert_eq!(event.peer_id, pid);
    assert!(event.dependencies.is_empty());
    match &event.payload {
        EventPayload::EntityCreated { entity_type, json_data } => {
            assert_eq!(entity_type, "bookmark");
            assert_eq!(json_data, r#"{"url":"x"}"#);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn event_entity_updated() {
    let eid = EntityId::new();
    let pid = PeerId::new();
    let event = Event::entity_updated(eid, pid, "task", r#"{"done":true}"#);

    match &event.payload {
        EventPayload::EntityUpdated { entity_type, json_data } => {
            assert_eq!(entity_type, "task");
            assert_eq!(json_data, r#"{"done":true}"#);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn event_entity_deleted() {
    let eid = EntityId::new();
    let pid = PeerId::new();
    let event = Event::entity_deleted(eid, pid, "note");

    match &event.payload {
        EventPayload::EntityDeleted { entity_type } => {
            assert_eq!(entity_type, "note");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn event_full_snapshot() {
    let eid = EntityId::new();
    let pid = PeerId::new();
    let event = Event::full_snapshot(eid, pid, "entry", r#"{"x":1}"#);

    match &event.payload {
        EventPayload::FullSnapshot { entity_type, json_data } => {
            assert_eq!(entity_type, "entry");
            assert_eq!(json_data, r#"{"x":1}"#);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn event_new_with_explicit_timestamp() {
    let ts = HybridTimestamp::new(5000, 3);
    let event = Event::new(
        EntityId::new(),
        PeerId::new(),
        ts,
        EventPayload::EntityDeleted { entity_type: "x".into() },
    );
    assert_eq!(event.timestamp, ts);
    assert!(event.dependencies.is_empty());
}

// ── Dependencies ─────────────────────────────────────────────────

#[test]
fn event_with_single_dependency() {
    let e1 = Event::entity_created(EntityId::new(), PeerId::new(), "a", "{}");
    let e2 = Event::entity_deleted(EntityId::new(), PeerId::new(), "a")
        .with_dependency(e1.id);
    assert_eq!(e2.dependencies, vec![e1.id]);
}

#[test]
fn event_with_multiple_dependencies() {
    let id1 = EventId::new();
    let id2 = EventId::new();
    let event = Event::entity_created(EntityId::new(), PeerId::new(), "b", "{}")
        .with_dependency(id1)
        .with_dependency(id2);
    assert_eq!(event.dependencies.len(), 2);
    assert_eq!(event.dependencies[0], id1);
    assert_eq!(event.dependencies[1], id2);
}

// ── Event serde roundtrip ────────────────────────────────────────

#[test]
fn event_full_serde_roundtrip() {
    let event = Event::entity_updated(EntityId::new(), PeerId::new(), "task", r#"{"a":1}"#)
        .with_dependency(EventId::new());

    let json = serde_json::to_string(&event).unwrap();
    let parsed: Event = serde_json::from_str(&json).unwrap();

    assert_eq!(event.id, parsed.id);
    assert_eq!(event.entity_id, parsed.entity_id);
    assert_eq!(event.peer_id, parsed.peer_id);
    assert_eq!(event.timestamp, parsed.timestamp);
    assert_eq!(event.payload, parsed.payload);
    assert_eq!(event.dependencies, parsed.dependencies);
}

#[test]
fn event_deserialize_without_dependencies_field() {
    // dependencies has #[serde(default)], so missing field should default to empty vec
    let event = Event::entity_created(EntityId::new(), PeerId::new(), "x", "{}");
    let mut json: serde_json::Value = serde_json::to_value(&event).unwrap();
    json.as_object_mut().unwrap().remove("dependencies");
    let parsed: Event = serde_json::from_value(json).unwrap();
    assert!(parsed.dependencies.is_empty());
}
