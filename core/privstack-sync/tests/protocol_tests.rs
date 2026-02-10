use privstack_crdt::VectorClock;
use privstack_sync::protocol::{
    ErrorMessage, EventAckMessage, EventBatchMessage, EventNotifyMessage, HelloAckMessage,
    HelloMessage, SubscribeMessage, SyncMessage, SyncRequestMessage, SyncStateMessage,
    MAX_BATCH_SIZE, PROTOCOL_VERSION,
};
use privstack_types::{EntityId, Event, EventPayload, HybridTimestamp, PeerId};

// ── Constants ────────────────────────────────────────────────────

#[test]
fn protocol_version_is_one() {
    assert_eq!(PROTOCOL_VERSION, 1);
}

#[test]
fn max_batch_size_is_100() {
    assert_eq!(MAX_BATCH_SIZE, 100);
}

// ── HelloMessage ─────────────────────────────────────────────────

#[test]
fn hello_message_creation() {
    let peer_id = PeerId::new();
    let msg = HelloMessage::new(peer_id, "My Device");

    assert_eq!(msg.version, PROTOCOL_VERSION);
    assert_eq!(msg.peer_id, peer_id);
    assert_eq!(msg.device_name, "My Device");
    assert!(msg.entity_ids.is_empty());
}

#[test]
fn hello_message_with_entities() {
    let peer_id = PeerId::new();
    let e1 = EntityId::new();
    let e2 = EntityId::new();

    let msg = HelloMessage::new(peer_id, "Dev").with_entities(vec![e1, e2]);

    assert_eq!(msg.entity_ids.len(), 2);
    assert_eq!(msg.entity_ids[0], e1);
    assert_eq!(msg.entity_ids[1], e2);
}

#[test]
fn hello_message_with_empty_entities() {
    let peer_id = PeerId::new();
    let msg = HelloMessage::new(peer_id, "Dev").with_entities(vec![]);
    assert!(msg.entity_ids.is_empty());
}

#[test]
fn hello_message_serde_roundtrip() {
    let peer_id = PeerId::new();
    let e1 = EntityId::new();
    let msg = HelloMessage::new(peer_id, "Device").with_entities(vec![e1]);

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: HelloMessage = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.version, msg.version);
    assert_eq!(parsed.peer_id, msg.peer_id);
    assert_eq!(parsed.device_name, msg.device_name);
    assert_eq!(parsed.entity_ids, msg.entity_ids);
}

// ── HelloAckMessage ──────────────────────────────────────────────

#[test]
fn hello_ack_accept() {
    let peer_id = PeerId::new();
    let ack = HelloAckMessage::accept(peer_id, "Other Device");

    assert!(ack.accepted);
    assert_eq!(ack.version, PROTOCOL_VERSION);
    assert_eq!(ack.peer_id, peer_id);
    assert_eq!(ack.device_name, "Other Device");
    assert!(ack.reason.is_none());
}

#[test]
fn hello_ack_reject() {
    let peer_id = PeerId::new();
    let ack = HelloAckMessage::reject(peer_id, "Version mismatch");

    assert!(!ack.accepted);
    assert_eq!(ack.reason, Some("Version mismatch".to_string()));
    assert!(ack.device_name.is_empty());
}

#[test]
fn hello_ack_serde_roundtrip() {
    let peer_id = PeerId::new();
    let ack = HelloAckMessage::accept(peer_id, "Dev");

    let json = serde_json::to_string(&ack).unwrap();
    let parsed: HelloAckMessage = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.accepted, ack.accepted);
    assert_eq!(parsed.peer_id, ack.peer_id);
    assert_eq!(parsed.device_name, ack.device_name);
}

// ── SyncRequestMessage ───────────────────────────────────────────

#[test]
fn sync_request_message() {
    let e1 = EntityId::new();
    let e2 = EntityId::new();
    let msg = SyncRequestMessage {
        entity_ids: vec![e1, e2],
        known_event_ids: std::collections::HashMap::new(),
    };
    assert_eq!(msg.entity_ids.len(), 2);
}

#[test]
fn sync_request_empty() {
    let msg = SyncRequestMessage {
        entity_ids: vec![],
        known_event_ids: std::collections::HashMap::new(),
    };
    assert!(msg.entity_ids.is_empty());
}

#[test]
fn sync_request_serde_roundtrip() {
    let e1 = EntityId::new();
    let msg = SyncRequestMessage {
        entity_ids: vec![e1],
        known_event_ids: std::collections::HashMap::new(),
    };

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: SyncRequestMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.entity_ids, msg.entity_ids);
}

// ── SyncStateMessage ─────────────────────────────────────────────

#[test]
fn sync_state_message_new_is_empty() {
    let state = SyncStateMessage::new();
    assert!(state.clocks.is_empty());
    assert!(state.event_counts.is_empty());
}

#[test]
fn sync_state_message_default() {
    let state = SyncStateMessage::default();
    assert!(state.clocks.is_empty());
}

#[test]
fn sync_state_add_entity() {
    let eid = EntityId::new();
    let peer = PeerId::new();
    let mut clock = VectorClock::new();
    clock.increment(peer);
    clock.increment(peer);

    let mut state = SyncStateMessage::new();
    state.add_entity(eid, clock.clone(), 5, vec![]);

    assert_eq!(state.clocks.len(), 1);
    assert_eq!(state.clocks.get(&eid).unwrap().get(&peer), 2);
    assert_eq!(state.event_counts.get(&eid), Some(&5));
}

#[test]
fn sync_state_multiple_entities() {
    let e1 = EntityId::new();
    let e2 = EntityId::new();
    let peer = PeerId::new();

    let mut clock1 = VectorClock::new();
    clock1.increment(peer);

    let mut clock2 = VectorClock::new();
    clock2.increment(peer);
    clock2.increment(peer);
    clock2.increment(peer);

    let mut state = SyncStateMessage::new();
    state.add_entity(e1, clock1, 1, vec![]);
    state.add_entity(e2, clock2, 3, vec![]);

    assert_eq!(state.clocks.len(), 2);
    assert_eq!(state.event_counts.get(&e1), Some(&1));
    assert_eq!(state.event_counts.get(&e2), Some(&3));
}

#[test]
fn sync_state_serde_roundtrip() {
    let eid = EntityId::new();
    let peer = PeerId::new();
    let mut clock = VectorClock::new();
    clock.increment(peer);

    let mut state = SyncStateMessage::new();
    state.add_entity(eid, clock, 10, vec![]);

    let json = serde_json::to_string(&state).unwrap();
    let parsed: SyncStateMessage = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.clocks.len(), 1);
    assert_eq!(parsed.event_counts.get(&eid), Some(&10));
}

// ── EventBatchMessage ────────────────────────────────────────────

fn make_event(entity_id: EntityId, peer_id: PeerId) -> Event {
    Event::new(
        entity_id,
        peer_id,
        HybridTimestamp::now(),
        EventPayload::FullSnapshot {
            entity_type: "test".into(),
            json_data: "{}".into(),
        },
    )
}

#[test]
fn event_batch_new() {
    let eid = EntityId::new();
    let peer = PeerId::new();
    let events = vec![make_event(eid, peer)];

    let batch = EventBatchMessage::new(eid, events, 0);
    assert_eq!(batch.entity_id, eid);
    assert_eq!(batch.events.len(), 1);
    assert!(!batch.is_final);
    assert_eq!(batch.batch_seq, 0);
}

#[test]
fn event_batch_finalize() {
    let eid = EntityId::new();
    let batch = EventBatchMessage::new(eid, vec![], 3).finalize();
    assert!(batch.is_final);
    assert_eq!(batch.batch_seq, 3);
}

#[test]
fn event_batch_serde_roundtrip() {
    let eid = EntityId::new();
    let peer = PeerId::new();
    let events = vec![make_event(eid, peer)];

    let batch = EventBatchMessage::new(eid, events, 1).finalize();
    let json = serde_json::to_string(&batch).unwrap();
    let parsed: EventBatchMessage = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.entity_id, eid);
    assert_eq!(parsed.events.len(), 1);
    assert!(parsed.is_final);
    assert_eq!(parsed.batch_seq, 1);
}

// ── EventAckMessage ──────────────────────────────────────────────

#[test]
fn event_ack_message() {
    let eid = EntityId::new();
    let ack = EventAckMessage {
        entity_id: eid,
        batch_seq: 2,
        received_count: 5,
        events: vec![],
    };
    assert_eq!(ack.entity_id, eid);
    assert_eq!(ack.batch_seq, 2);
    assert_eq!(ack.received_count, 5);
    assert!(ack.events.is_empty());
}

#[test]
fn event_ack_serde_roundtrip() {
    let eid = EntityId::new();
    let ack = EventAckMessage {
        entity_id: eid,
        batch_seq: 0,
        received_count: 3,
        events: vec![],
    };

    let json = serde_json::to_string(&ack).unwrap();
    let parsed: EventAckMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.received_count, 3);
}

#[test]
fn event_ack_default_events_empty_on_deserialize() {
    // events has #[serde(default)] so missing field should be empty vec
    let json = r#"{"entity_id":"00000000-0000-0000-0000-000000000000","batch_seq":0,"received_count":1}"#;
    let parsed: EventAckMessage = serde_json::from_str(json).unwrap();
    assert!(parsed.events.is_empty());
}

// ── SubscribeMessage ─────────────────────────────────────────────

#[test]
fn subscribe_message() {
    let e1 = EntityId::new();
    let msg = SubscribeMessage {
        entity_ids: vec![e1],
    };
    assert_eq!(msg.entity_ids.len(), 1);
}

#[test]
fn subscribe_serde_roundtrip() {
    let e1 = EntityId::new();
    let msg = SubscribeMessage {
        entity_ids: vec![e1],
    };

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: SubscribeMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.entity_ids, msg.entity_ids);
}

// ── EventNotifyMessage ───────────────────────────────────────────

#[test]
fn event_notify_message() {
    let eid = EntityId::new();
    let peer = PeerId::new();
    let event = make_event(eid, peer);

    let msg = EventNotifyMessage {
        event: event.clone(),
    };
    assert_eq!(msg.event.entity_id, eid);
}

#[test]
fn event_notify_serde_roundtrip() {
    let eid = EntityId::new();
    let peer = PeerId::new();
    let event = make_event(eid, peer);

    let msg = EventNotifyMessage { event };
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: EventNotifyMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.event.entity_id, eid);
}

// ── ErrorMessage ─────────────────────────────────────────────────

#[test]
fn error_message_new() {
    let err = ErrorMessage::new(42, "something failed");
    assert_eq!(err.code, 42);
    assert_eq!(err.message, "something failed");
}

#[test]
fn error_version_mismatch() {
    let err = ErrorMessage::version_mismatch(1, 2);
    assert_eq!(err.code, 1);
    assert!(err.message.contains("1"));
    assert!(err.message.contains("2"));
}

#[test]
fn error_unknown_entity() {
    let eid = EntityId::new();
    let err = ErrorMessage::unknown_entity(&eid);
    assert_eq!(err.code, 2);
    assert!(err.message.contains(&eid.to_string()));
}

#[test]
fn error_internal() {
    let err = ErrorMessage::internal("oh no");
    assert_eq!(err.code, 99);
    assert_eq!(err.message, "oh no");
}

#[test]
fn error_message_serde_roundtrip() {
    let err = ErrorMessage::new(5, "test error");
    let json = serde_json::to_string(&err).unwrap();
    let parsed: ErrorMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.code, 5);
    assert_eq!(parsed.message, "test error");
}

// ── SyncMessage enum ─────────────────────────────────────────────

#[test]
fn sync_message_hello_serde() {
    let peer_id = PeerId::new();
    let msg = SyncMessage::Hello(HelloMessage::new(peer_id, "Test"));
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: SyncMessage = serde_json::from_str(&json).unwrap();
    match parsed {
        SyncMessage::Hello(h) => {
            assert_eq!(h.peer_id, peer_id);
            assert_eq!(h.device_name, "Test");
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn sync_message_hello_ack_serde() {
    let peer_id = PeerId::new();
    let msg = SyncMessage::HelloAck(HelloAckMessage::accept(peer_id, "D"));
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: SyncMessage = serde_json::from_str(&json).unwrap();
    match parsed {
        SyncMessage::HelloAck(a) => assert!(a.accepted),
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn sync_message_event_batch_serde() {
    let eid = EntityId::new();
    let msg = SyncMessage::EventBatch(EventBatchMessage::new(eid, vec![], 0).finalize());
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: SyncMessage = serde_json::from_str(&json).unwrap();
    match parsed {
        SyncMessage::EventBatch(b) => {
            assert!(b.is_final);
            assert_eq!(b.entity_id, eid);
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn sync_message_ping_pong_serde() {
    let ping = SyncMessage::Ping(12345);
    let json = serde_json::to_string(&ping).unwrap();
    let parsed: SyncMessage = serde_json::from_str(&json).unwrap();
    match parsed {
        SyncMessage::Ping(v) => assert_eq!(v, 12345),
        _ => panic!("Wrong variant"),
    }

    let pong = SyncMessage::Pong(12345);
    let json = serde_json::to_string(&pong).unwrap();
    let parsed: SyncMessage = serde_json::from_str(&json).unwrap();
    match parsed {
        SyncMessage::Pong(v) => assert_eq!(v, 12345),
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn sync_message_error_serde() {
    let msg = SyncMessage::Error(ErrorMessage::internal("fail"));
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: SyncMessage = serde_json::from_str(&json).unwrap();
    match parsed {
        SyncMessage::Error(e) => assert_eq!(e.code, 99),
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn sync_message_sync_request_serde() {
    let e1 = EntityId::new();
    let msg = SyncMessage::SyncRequest(SyncRequestMessage {
        entity_ids: vec![e1],
        known_event_ids: std::collections::HashMap::new(),
    });
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: SyncMessage = serde_json::from_str(&json).unwrap();
    match parsed {
        SyncMessage::SyncRequest(r) => assert_eq!(r.entity_ids.len(), 1),
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn sync_message_sync_state_serde() {
    let msg = SyncMessage::SyncState(SyncStateMessage::new());
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: SyncMessage = serde_json::from_str(&json).unwrap();
    match parsed {
        SyncMessage::SyncState(s) => assert!(s.clocks.is_empty()),
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn sync_message_subscribe_serde() {
    let e1 = EntityId::new();
    let msg = SyncMessage::Subscribe(SubscribeMessage {
        entity_ids: vec![e1],
    });
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: SyncMessage = serde_json::from_str(&json).unwrap();
    match parsed {
        SyncMessage::Subscribe(s) => assert_eq!(s.entity_ids.len(), 1),
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn sync_message_event_notify_serde() {
    let eid = EntityId::new();
    let peer = PeerId::new();
    let event = make_event(eid, peer);
    let msg = SyncMessage::EventNotify(EventNotifyMessage { event });
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: SyncMessage = serde_json::from_str(&json).unwrap();
    match parsed {
        SyncMessage::EventNotify(n) => assert_eq!(n.event.entity_id, eid),
        _ => panic!("Wrong variant"),
    }
}
