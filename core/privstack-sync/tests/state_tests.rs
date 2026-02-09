use privstack_crdt::VectorClock;
use privstack_sync::state::{EntitySyncState, PeerSyncStatus, SyncState};
use privstack_types::{EntityId, Event, EventId, EventPayload, HybridTimestamp, PeerId};
use std::collections::HashSet;

fn make_event(entity_id: EntityId, peer_id: PeerId) -> Event {
    Event::new(
        entity_id,
        peer_id,
        HybridTimestamp::now(),
        EventPayload::FullSnapshot {
            entity_type: "test".to_string(),
            json_data: "{}".to_string(),
        },
    )
}

// ── SyncState ────────────────────────────────────────────────────

#[test]
fn sync_state_creation() {
    let peer_id = PeerId::new();
    let state = SyncState::new(peer_id);

    assert_eq!(state.local_peer_id(), Some(peer_id));
    assert_eq!(state.entity_ids().count(), 0);
}

#[test]
fn sync_state_default_has_no_peer() {
    let state = SyncState::default();
    assert_eq!(state.local_peer_id(), None);
    assert_eq!(state.entity_ids().count(), 0);
}

#[test]
fn set_local_peer_id() {
    let mut state = SyncState::default();
    let peer = PeerId::new();
    state.set_local_peer_id(peer);
    assert_eq!(state.local_peer_id(), Some(peer));
}

#[test]
fn get_entity_missing() {
    let state = SyncState::new(PeerId::new());
    assert!(state.get_entity(&EntityId::new()).is_none());
}

#[test]
fn get_or_create_entity() {
    let mut state = SyncState::new(PeerId::new());
    let eid = EntityId::new();

    let entity_state = state.get_or_create_entity(eid);
    assert_eq!(entity_state.event_count, 0);

    // Second call returns same state
    state.get_or_create_entity(eid).event_count = 5;
    assert_eq!(state.get_entity(&eid).unwrap().event_count, 5);
}

#[test]
fn remove_entity() {
    let mut state = SyncState::new(PeerId::new());
    let eid = EntityId::new();
    state.get_or_create_entity(eid);
    assert!(state.get_entity(&eid).is_some());

    state.remove_entity(&eid);
    assert!(state.get_entity(&eid).is_none());
}

#[test]
fn remove_nonexistent_entity_is_noop() {
    let mut state = SyncState::new(PeerId::new());
    state.remove_entity(&EntityId::new()); // should not panic
}

#[test]
fn entity_ids_tracks_entities() {
    let mut state = SyncState::new(PeerId::new());
    let e1 = EntityId::new();
    let e2 = EntityId::new();
    state.get_or_create_entity(e1);
    state.get_or_create_entity(e2);

    let ids: HashSet<_> = state.entity_ids().cloned().collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&e1));
    assert!(ids.contains(&e2));
}

#[test]
fn record_event_creates_entity_state() {
    let mut state = SyncState::new(PeerId::new());
    let eid = EntityId::new();
    let peer = PeerId::new();

    let event = make_event(eid, peer);
    state.record_event(eid, &event);

    let entity_state = state.get_entity(&eid).unwrap();
    assert_eq!(entity_state.event_count, 1);
    assert_eq!(entity_state.clock.get(&peer), 1);
    assert!(entity_state.seen_event_ids.contains(&event.id));
}

#[test]
fn record_event_deduplicates() {
    let mut state = SyncState::new(PeerId::new());
    let eid = EntityId::new();
    let peer = PeerId::new();

    let event = make_event(eid, peer);
    state.record_event(eid, &event);
    state.record_event(eid, &event); // duplicate

    let entity_state = state.get_entity(&eid).unwrap();
    assert_eq!(entity_state.event_count, 1); // not 2
    assert_eq!(entity_state.clock.get(&peer), 1); // not 2
}

#[test]
fn record_multiple_events_from_same_peer() {
    let mut state = SyncState::new(PeerId::new());
    let eid = EntityId::new();
    let peer = PeerId::new();

    let e1 = make_event(eid, peer);
    let e2 = make_event(eid, peer);
    state.record_event(eid, &e1);
    state.record_event(eid, &e2);

    let entity_state = state.get_entity(&eid).unwrap();
    assert_eq!(entity_state.event_count, 2);
    assert_eq!(entity_state.clock.get(&peer), 2);
}

#[test]
fn record_events_from_multiple_peers() {
    let mut state = SyncState::new(PeerId::new());
    let eid = EntityId::new();
    let peer_a = PeerId::new();
    let peer_b = PeerId::new();

    state.record_event(eid, &make_event(eid, peer_a));
    state.record_event(eid, &make_event(eid, peer_b));
    state.record_event(eid, &make_event(eid, peer_a));

    let entity_state = state.get_entity(&eid).unwrap();
    assert_eq!(entity_state.event_count, 3);
    assert_eq!(entity_state.clock.get(&peer_a), 2);
    assert_eq!(entity_state.clock.get(&peer_b), 1);
}

#[test]
fn get_clock() {
    let mut state = SyncState::new(PeerId::new());
    let eid = EntityId::new();
    let peer = PeerId::new();

    assert!(state.get_clock(&eid).is_none());

    state.record_event(eid, &make_event(eid, peer));
    let clock = state.get_clock(&eid).unwrap();
    assert_eq!(clock.get(&peer), 1);
}

// ── compute_missing_events ───────────────────────────────────────

#[test]
fn compute_missing_events_peer_knows_nothing() {
    let state = SyncState::new(PeerId::new());
    let eid = EntityId::new();
    let peer = PeerId::new();

    let events: Vec<Event> = (0..3).map(|_| make_event(eid, peer)).collect();
    let known = HashSet::new();

    let missing = state.compute_missing_events(&eid, &known, events.iter());
    assert_eq!(missing.len(), 3);
}

#[test]
fn compute_missing_events_peer_knows_some() {
    let state = SyncState::new(PeerId::new());
    let eid = EntityId::new();
    let peer = PeerId::new();

    let events: Vec<Event> = (0..5).map(|_| make_event(eid, peer)).collect();
    let known: HashSet<EventId> = events[..2].iter().map(|e| e.id).collect();

    let missing = state.compute_missing_events(&eid, &known, events.iter());
    assert_eq!(missing.len(), 3);

    let missing_ids: HashSet<_> = missing.iter().map(|e| e.id).collect();
    for e in &events[2..] {
        assert!(missing_ids.contains(&e.id));
    }
}

#[test]
fn compute_missing_events_peer_knows_all() {
    let state = SyncState::new(PeerId::new());
    let eid = EntityId::new();
    let peer = PeerId::new();

    let events: Vec<Event> = (0..3).map(|_| make_event(eid, peer)).collect();
    let known: HashSet<EventId> = events.iter().map(|e| e.id).collect();

    let missing = state.compute_missing_events(&eid, &known, events.iter());
    assert!(missing.is_empty());
}

#[test]
fn compute_missing_events_empty_events() {
    let state = SyncState::new(PeerId::new());
    let events: Vec<Event> = vec![];
    let missing = state.compute_missing_events(&EntityId::new(), &HashSet::new(), events.iter());
    assert!(missing.is_empty());
}

// ── known_event_ids ──────────────────────────────────────────────

#[test]
fn known_event_ids_helper() {
    let eid = EntityId::new();
    let peer = PeerId::new();
    let events: Vec<Event> = (0..3).map(|_| make_event(eid, peer)).collect();

    let ids = SyncState::known_event_ids(&events);
    assert_eq!(ids.len(), 3);
    for e in &events {
        assert!(ids.contains(&e.id));
    }
}

#[test]
fn known_event_ids_empty() {
    let ids = SyncState::known_event_ids(&[]);
    assert!(ids.is_empty());
}

// ── SyncState serde ──────────────────────────────────────────────

#[test]
fn sync_state_serde_roundtrip() {
    let mut state = SyncState::new(PeerId::new());
    let eid = EntityId::new();
    let peer = PeerId::new();

    state.record_event(eid, &make_event(eid, peer));
    state.record_event(eid, &make_event(eid, peer));

    let json = serde_json::to_string(&state).unwrap();
    let parsed: SyncState = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.local_peer_id(), state.local_peer_id());
    let parsed_entity = parsed.get_entity(&eid).unwrap();
    assert_eq!(parsed_entity.event_count, 2);
    assert_eq!(parsed_entity.clock.get(&peer), 2);
}

// ── EntitySyncState ──────────────────────────────────────────────

#[test]
fn entity_sync_state_new() {
    let state = EntitySyncState::new();
    assert_eq!(state.event_count, 0);
    assert!(state.seen_event_ids.is_empty());
    assert!(state.last_sync.is_empty());
}

#[test]
fn entity_sync_state_record_event() {
    let mut state = EntitySyncState::new();
    let eid = EntityId::new();
    let peer = PeerId::new();

    let event = make_event(eid, peer);
    state.record_event(&event);

    assert_eq!(state.event_count, 1);
    assert_eq!(state.clock.get(&peer), 1);
    assert!(state.seen_event_ids.contains(&event.id));
}

#[test]
fn entity_sync_state_record_duplicate_event() {
    let mut state = EntitySyncState::new();
    let eid = EntityId::new();
    let peer = PeerId::new();

    let event = make_event(eid, peer);
    state.record_event(&event);
    state.record_event(&event);

    assert_eq!(state.event_count, 1);
    assert_eq!(state.clock.get(&peer), 1);
}

#[test]
fn entity_sync_state_record_sync() {
    let mut state = EntitySyncState::new();
    let peer = PeerId::new();
    let ts = HybridTimestamp::now();

    state.record_sync(peer, ts);
    assert_eq!(state.last_sync.get(&peer), Some(&ts));
}

#[test]
fn entity_sync_state_merge_clock() {
    let peer1 = PeerId::new();
    let peer2 = PeerId::new();

    let mut state = EntitySyncState::new();
    let mut clock1 = VectorClock::new();
    for _ in 0..5 {
        clock1.increment(peer1);
    }
    state.clock = clock1;

    let mut clock2 = VectorClock::new();
    for _ in 0..3 {
        clock2.increment(peer2);
    }

    state.merge_clock(&clock2);

    assert_eq!(state.clock.get(&peer1), 5);
    assert_eq!(state.clock.get(&peer2), 3);
}

#[test]
fn entity_sync_state_merge_clock_higher_wins() {
    let peer = PeerId::new();

    let mut state = EntitySyncState::new();
    let mut clock1 = VectorClock::new();
    for _ in 0..5 {
        clock1.increment(peer);
    }
    state.clock = clock1;

    let mut clock2 = VectorClock::new();
    for _ in 0..3 {
        clock2.increment(peer);
    }

    state.merge_clock(&clock2);
    // Merge should keep the higher value (5)
    assert_eq!(state.clock.get(&peer), 5);
}

// ── PeerSyncStatus ───────────────────────────────────────────────

#[test]
fn peer_sync_status_new() {
    let peer = PeerId::new();
    let status = PeerSyncStatus::new(peer, "My Device");

    assert_eq!(status.peer_id, peer);
    assert_eq!(status.device_name, "My Device");
    assert!(status.shared_entities.is_empty());
    assert!(!status.connected);
    assert!(status.last_sync.is_none());
}

#[test]
fn peer_sync_status_clone() {
    let peer = PeerId::new();
    let mut status = PeerSyncStatus::new(peer, "Dev");
    status.connected = true;
    status.shared_entities.push(EntityId::new());

    let cloned = status.clone();
    assert_eq!(cloned.peer_id, peer);
    assert!(cloned.connected);
    assert_eq!(cloned.shared_entities.len(), 1);
}

#[test]
fn peer_sync_status_serde_roundtrip() {
    let peer = PeerId::new();
    let mut status = PeerSyncStatus::new(peer, "Dev");
    status.connected = true;

    let json = serde_json::to_string(&status).unwrap();
    let parsed: PeerSyncStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.peer_id, peer);
    assert_eq!(parsed.device_name, "Dev");
    assert!(parsed.connected);
}
