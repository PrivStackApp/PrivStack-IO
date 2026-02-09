use privstack_storage::EventStore;
use privstack_types::{EntityId, Event, EventPayload, HybridTimestamp, PeerId};

fn make_event(entity_id: EntityId, peer_id: PeerId, wall: u64) -> Event {
    Event::new(
        entity_id,
        peer_id,
        HybridTimestamp::new(wall, 0),
        EventPayload::EntityCreated {
            entity_type: "note".into(),
            json_data: r#"{"title":"test"}"#.into(),
        },
    )
}

#[test]
fn save_and_retrieve_event() {
    let store = EventStore::open_in_memory().unwrap();
    let eid = EntityId::new();
    let pid = PeerId::new();
    let event = Event::entity_created(eid, pid, "bookmark", r#"{"title":"Test"}"#);

    store.save_event(&event).unwrap();
    let events = store.get_events_for_entity(&eid).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, event.id);
}

#[test]
fn save_duplicate_is_ignored() {
    let store = EventStore::open_in_memory().unwrap();
    let eid = EntityId::new();
    let pid = PeerId::new();
    let event = Event::entity_created(eid, pid, "note", "{}");
    store.save_event(&event).unwrap();
    store.save_event(&event).unwrap(); // INSERT OR IGNORE
    let events = store.get_events_for_entity(&eid).unwrap();
    assert_eq!(events.len(), 1);
}

#[test]
fn get_events_empty() {
    let store = EventStore::open_in_memory().unwrap();
    let events = store.get_events_for_entity(&EntityId::new()).unwrap();
    assert!(events.is_empty());
}

#[test]
fn events_ordered_by_timestamp() {
    let store = EventStore::open_in_memory().unwrap();
    let eid = EntityId::new();
    let pid = PeerId::new();

    let e1 = make_event(eid, pid, 100);
    let e2 = make_event(eid, pid, 300);
    let e3 = make_event(eid, pid, 200);

    // Save out of order
    store.save_event(&e2).unwrap();
    store.save_event(&e1).unwrap();
    store.save_event(&e3).unwrap();

    let events = store.get_events_for_entity(&eid).unwrap();
    assert_eq!(events.len(), 3);
    assert!(events[0].timestamp.wall_time() <= events[1].timestamp.wall_time());
    assert!(events[1].timestamp.wall_time() <= events[2].timestamp.wall_time());
}

#[test]
fn multiple_entities() {
    let store = EventStore::open_in_memory().unwrap();
    let pid = PeerId::new();
    let eid1 = EntityId::new();
    let eid2 = EntityId::new();

    store.save_event(&Event::entity_created(eid1, pid, "a", "{}")).unwrap();
    store.save_event(&Event::entity_created(eid1, pid, "a", r#"{"v":2}"#)).unwrap();
    store.save_event(&Event::entity_created(eid2, pid, "b", "{}")).unwrap();

    assert_eq!(store.get_events_for_entity(&eid1).unwrap().len(), 2);
    assert_eq!(store.get_events_for_entity(&eid2).unwrap().len(), 1);
}

// ── get_events_since ─────────────────────────────────────────────

#[test]
fn get_events_since() {
    let store = EventStore::open_in_memory().unwrap();
    let eid = EntityId::new();
    let pid = PeerId::new();

    store.save_event(&make_event(eid, pid, 100)).unwrap();
    store.save_event(&make_event(eid, pid, 200)).unwrap();
    store.save_event(&make_event(eid, pid, 300)).unwrap();

    let since = HybridTimestamp::new(150, 0);
    let events = store.get_events_since(&pid, &since).unwrap();
    assert_eq!(events.len(), 2); // wall_time 200 and 300
}

#[test]
fn get_events_since_empty() {
    let store = EventStore::open_in_memory().unwrap();
    let events = store.get_events_since(&PeerId::new(), &HybridTimestamp::new(0, 0)).unwrap();
    assert!(events.is_empty());
}

#[test]
fn get_events_since_filters_by_peer() {
    let store = EventStore::open_in_memory().unwrap();
    let eid = EntityId::new();
    let p1 = PeerId::new();
    let p2 = PeerId::new();

    store.save_event(&make_event(eid, p1, 100)).unwrap();
    store.save_event(&make_event(eid, p2, 200)).unwrap();

    let events = store.get_events_since(&p1, &HybridTimestamp::new(0, 0)).unwrap();
    assert_eq!(events.len(), 1);
}

// ── get_latest_timestamp_for_peer ────────────────────────────────

#[test]
fn latest_timestamp_no_events() {
    let store = EventStore::open_in_memory().unwrap();
    let ts = store.get_latest_timestamp_for_peer(&PeerId::new()).unwrap();
    assert!(ts.is_none());
}

#[test]
fn latest_timestamp_returns_max() {
    let store = EventStore::open_in_memory().unwrap();
    let eid = EntityId::new();
    let pid = PeerId::new();

    store.save_event(&make_event(eid, pid, 100)).unwrap();
    store.save_event(&make_event(eid, pid, 500)).unwrap();
    store.save_event(&make_event(eid, pid, 300)).unwrap();

    let ts = store.get_latest_timestamp_for_peer(&pid).unwrap().unwrap();
    assert_eq!(ts.wall_time(), 500);
}

// ── All payload types ────────────────────────────────────────────

#[test]
fn save_all_event_types() {
    let store = EventStore::open_in_memory().unwrap();
    let eid = EntityId::new();
    let pid = PeerId::new();

    let events = vec![
        Event::entity_created(eid, pid, "note", r#"{"title":"A"}"#),
        Event::entity_updated(eid, pid, "note", r#"{"title":"B"}"#),
        Event::entity_deleted(eid, pid, "note"),
        Event::full_snapshot(eid, pid, "note", r#"{"title":"C"}"#),
    ];

    for e in &events {
        store.save_event(e).unwrap();
    }

    let stored = store.get_events_for_entity(&eid).unwrap();
    assert_eq!(stored.len(), 4);
}

// ── Event with dependencies ──────────────────────────────────────

#[test]
fn save_event_with_dependencies() {
    let store = EventStore::open_in_memory().unwrap();
    let eid = EntityId::new();
    let pid = PeerId::new();

    let e1 = Event::entity_created(eid, pid, "x", "{}");
    let e2 = Event::entity_updated(eid, pid, "x", r#"{"v":2}"#).with_dependency(e1.id);

    store.save_event(&e1).unwrap();
    store.save_event(&e2).unwrap();

    let events = store.get_events_for_entity(&eid).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].dependencies.len(), 1);
}
