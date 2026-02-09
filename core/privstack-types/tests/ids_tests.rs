use privstack_types::{EntityId, PeerId};
use std::collections::HashSet;
use std::str::FromStr;

// ── EntityId ──────────────────────────────────────────────────────

#[test]
fn entity_id_new_is_unique() {
    let a = EntityId::new();
    let b = EntityId::new();
    assert_ne!(a, b);
}

#[test]
fn entity_id_from_uuid_roundtrip() {
    let uuid = uuid::Uuid::now_v7();
    let id = EntityId::from_uuid(uuid);
    assert_eq!(id.as_uuid(), uuid);
}

#[test]
fn entity_id_display_and_parse() {
    let id = EntityId::new();
    let s = id.to_string();
    let parsed = EntityId::parse(&s).unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn entity_id_from_str() {
    let id = EntityId::new();
    let s = id.to_string();
    let parsed: EntityId = EntityId::from_str(&s).unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn entity_id_parse_invalid() {
    assert!(EntityId::parse("not-a-uuid").is_err());
}

#[test]
fn entity_id_from_str_invalid() {
    assert!(EntityId::from_str("garbage").is_err());
}

#[test]
fn entity_id_default_is_unique() {
    let a = EntityId::default();
    let b = EntityId::default();
    assert_ne!(a, b);
}

#[test]
fn entity_id_hash_and_eq() {
    let id = EntityId::new();
    let mut set = HashSet::new();
    set.insert(id);
    set.insert(id); // duplicate
    assert_eq!(set.len(), 1);
}

#[test]
fn entity_id_clone_and_copy() {
    let id = EntityId::new();
    let cloned = id;
    assert_eq!(id, cloned);
}

#[test]
fn entity_id_serialization_roundtrip() {
    let id = EntityId::new();
    let json = serde_json::to_string(&id).unwrap();
    let parsed: EntityId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn entity_id_debug_contains_uuid() {
    let id = EntityId::new();
    let debug = format!("{:?}", id);
    assert!(debug.contains("EntityId"));
}

// ── PeerId ────────────────────────────────────────────────────────

#[test]
fn peer_id_new_is_unique() {
    let a = PeerId::new();
    let b = PeerId::new();
    assert_ne!(a, b);
}

#[test]
fn peer_id_from_uuid_roundtrip() {
    let uuid = uuid::Uuid::now_v7();
    let id = PeerId::from_uuid(uuid);
    assert_eq!(id.as_uuid(), uuid);
}

#[test]
fn peer_id_display_and_parse() {
    let id = PeerId::new();
    let s = id.to_string();
    let parsed = PeerId::parse(&s).unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn peer_id_from_str() {
    let id = PeerId::new();
    let s = id.to_string();
    let parsed: PeerId = PeerId::from_str(&s).unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn peer_id_parse_invalid() {
    assert!(PeerId::parse("not-a-uuid").is_err());
}

#[test]
fn peer_id_from_str_invalid() {
    assert!(PeerId::from_str("garbage").is_err());
}

#[test]
fn peer_id_default_is_unique() {
    let a = PeerId::default();
    let b = PeerId::default();
    assert_ne!(a, b);
}

#[test]
fn peer_id_hash_and_eq() {
    let id = PeerId::new();
    let mut set = HashSet::new();
    set.insert(id);
    set.insert(id);
    assert_eq!(set.len(), 1);
}

#[test]
fn peer_id_clone_and_copy() {
    let id = PeerId::new();
    let cloned = id;
    assert_eq!(id, cloned);
}

#[test]
fn peer_id_serialization_roundtrip() {
    let id = PeerId::new();
    let json = serde_json::to_string(&id).unwrap();
    let parsed: PeerId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn peer_id_debug_contains_peer_id() {
    let id = PeerId::new();
    let debug = format!("{:?}", id);
    assert!(debug.contains("PeerId"));
}
