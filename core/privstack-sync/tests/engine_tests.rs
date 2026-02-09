use privstack_storage::{EntityStore, EventStore};
use privstack_sync::protocol::{
    EventBatchMessage, HelloMessage, SyncMessage, SyncRequestMessage, PROTOCOL_VERSION,
};
use privstack_sync::{SyncConfig, SyncEngine};
use privstack_types::{EntityId, Event, EventPayload, HybridTimestamp, PeerId};
use std::collections::HashSet;
use std::sync::Arc;

fn make_engine(peer_id: PeerId) -> SyncEngine {
    SyncEngine::new(peer_id, SyncConfig::default())
}

fn make_engine_with_config(peer_id: PeerId, device_name: &str, batch_size: usize) -> SyncEngine {
    SyncEngine::new(
        peer_id,
        SyncConfig {
            device_name: device_name.to_string(),
            batch_size,
            timeout_ms: 5000,
        },
    )
}

fn make_stores() -> (Arc<EntityStore>, Arc<EventStore>) {
    let entity_store = Arc::new(EntityStore::open_in_memory().unwrap());
    let event_store = Arc::new(EventStore::open_in_memory().unwrap());
    (entity_store, event_store)
}

fn make_event(entity_id: EntityId, peer_id: PeerId) -> Event {
    Event::new(
        entity_id,
        peer_id,
        HybridTimestamp::now(),
        EventPayload::FullSnapshot {
            entity_type: "note".into(),
            json_data: r#"{"title":"test"}"#.into(),
        },
    )
}

// ── Construction & accessors ─────────────────────────────────────

#[tokio::test]
async fn engine_creation() {
    let peer_id = PeerId::new();
    let engine = make_engine(peer_id);

    assert_eq!(engine.peer_id(), peer_id);
    assert_eq!(engine.device_name(), "PrivStack Device");
    assert_eq!(engine.batch_size(), 100);
}

#[tokio::test]
async fn engine_custom_config() {
    let peer_id = PeerId::new();
    let engine = make_engine_with_config(peer_id, "My Laptop", 50);

    assert_eq!(engine.device_name(), "My Laptop");
    assert_eq!(engine.batch_size(), 50);
}

#[tokio::test]
async fn default_config() {
    let config = SyncConfig::default();
    assert_eq!(config.device_name, "PrivStack Device");
    assert_eq!(config.batch_size, 100);
    assert_eq!(config.timeout_ms, 30_000);
}

// ── Message producers ────────────────────────────────────────────

#[tokio::test]
async fn make_hello() {
    let peer_id = PeerId::new();
    let engine = make_engine(peer_id);
    let e1 = EntityId::new();

    let msg = engine.make_hello(vec![e1]);
    match msg {
        SyncMessage::Hello(h) => {
            assert_eq!(h.peer_id, peer_id);
            assert_eq!(h.device_name, "PrivStack Device");
            assert_eq!(h.version, PROTOCOL_VERSION);
            assert_eq!(h.entity_ids, vec![e1]);
        }
        _ => panic!("Expected Hello"),
    }
}

#[tokio::test]
async fn make_hello_empty_entities() {
    let engine = make_engine(PeerId::new());
    let msg = engine.make_hello(vec![]);
    match msg {
        SyncMessage::Hello(h) => assert!(h.entity_ids.is_empty()),
        _ => panic!("Expected Hello"),
    }
}

#[tokio::test]
async fn make_hello_accept() {
    let peer_id = PeerId::new();
    let engine = make_engine(peer_id);

    let msg = engine.make_hello_accept();
    match msg {
        SyncMessage::HelloAck(ack) => {
            assert!(ack.accepted);
            assert_eq!(ack.peer_id, peer_id);
            assert_eq!(ack.device_name, "PrivStack Device");
        }
        _ => panic!("Expected HelloAck"),
    }
}

#[tokio::test]
async fn make_hello_reject() {
    let engine = make_engine(PeerId::new());
    let msg = engine.make_hello_reject("bad version");
    match msg {
        SyncMessage::HelloAck(ack) => {
            assert!(!ack.accepted);
            assert_eq!(ack.reason, Some("bad version".to_string()));
        }
        _ => panic!("Expected HelloAck"),
    }
}

#[tokio::test]
async fn make_sync_request() {
    let engine = make_engine(PeerId::new());
    let e1 = EntityId::new();
    let e2 = EntityId::new();

    let (_entity_store, event_store) = make_stores();
    let msg = engine.make_sync_request(vec![e1, e2], &event_store).await;
    match msg {
        SyncMessage::SyncRequest(r) => {
            assert_eq!(r.entity_ids.len(), 2);
            assert_eq!(r.entity_ids[0], e1);
            assert_eq!(r.entity_ids[1], e2);
        }
        _ => panic!("Expected SyncRequest"),
    }
}

// ── Handle hello ─────────────────────────────────────────────────

#[tokio::test]
async fn handle_hello_accepts_matching_version() {
    let engine = make_engine(PeerId::new());
    let remote_peer = PeerId::new();

    let hello = HelloMessage::new(remote_peer, "Remote Device")
        .with_entities(vec![EntityId::new()]);

    let response = engine.handle_hello(&hello).await;
    match response {
        SyncMessage::HelloAck(ack) => {
            assert!(ack.accepted);
        }
        _ => panic!("Expected HelloAck"),
    }

    // Peer should be tracked
    let peers = engine.connected_peers().await;
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].peer_id, remote_peer);
    assert!(peers[0].connected);
    assert_eq!(peers[0].device_name, "Remote Device");
}

#[tokio::test]
async fn handle_hello_rejects_wrong_version() {
    let engine = make_engine(PeerId::new());
    let mut hello = HelloMessage::new(PeerId::new(), "Remote");
    hello.version = 999;

    let response = engine.handle_hello(&hello).await;
    match response {
        SyncMessage::HelloAck(ack) => {
            assert!(!ack.accepted);
            assert!(ack.reason.unwrap().contains("version mismatch"));
        }
        _ => panic!("Expected HelloAck"),
    }
}

// ── Handle sync request ──────────────────────────────────────────

#[tokio::test]
async fn handle_sync_request_empty_state() {
    let engine = make_engine(PeerId::new());
    let (_entity_store, event_store) = make_stores();

    let remote = PeerId::new();
    let request = SyncRequestMessage {
        entity_ids: vec![EntityId::new()],
        known_event_ids: std::collections::HashMap::new(),
    };

    let response = engine.handle_sync_request(&remote, &request, &event_store).await;
    match response {
        SyncMessage::SyncState(state) => {
            // No events stored, so state should be empty or have zero counts
            assert!(state.clocks.is_empty() || state.event_counts.values().all(|&c| c == 0));
        }
        _ => panic!("Expected SyncState"),
    }
}

#[tokio::test]
async fn handle_sync_request_with_recorded_events() {
    let peer_id = PeerId::new();
    let engine = make_engine(peer_id);
    let (_entity_store, event_store) = make_stores();

    let eid = EntityId::new();
    let event = make_event(eid, peer_id);
    event_store.save_event(&event).unwrap();
    engine.record_local_event(&event).await;

    let remote = PeerId::new();
    let request = SyncRequestMessage {
        entity_ids: vec![eid],
        known_event_ids: std::collections::HashMap::new(),
    };

    let response = engine.handle_sync_request(&remote, &request, &event_store).await;
    match response {
        SyncMessage::SyncState(state) => {
            assert!(state.clocks.contains_key(&eid));
            assert_eq!(state.event_counts.get(&eid), Some(&1));
        }
        _ => panic!("Expected SyncState"),
    }
}

// ── Compute event batches ────────────────────────────────────────

#[tokio::test]
async fn compute_event_batches_no_missing() {
    let peer_id = PeerId::new();
    let engine = make_engine(peer_id);
    let (_entity_store, event_store) = make_stores();
    let eid = EntityId::new();

    let event = make_event(eid, peer_id);
    event_store.save_event(&event).unwrap();

    // Peer already knows all events
    let known: HashSet<_> = vec![event.id].into_iter().collect();
    let batches = engine.compute_event_batches(eid, &known, &event_store).await;
    assert!(batches.is_empty());
}

#[tokio::test]
async fn compute_event_batches_has_missing() {
    let peer_id = PeerId::new();
    let engine = make_engine(peer_id);
    let (_entity_store, event_store) = make_stores();
    let eid = EntityId::new();

    // Store 3 events
    let events: Vec<Event> = (0..3).map(|_| make_event(eid, peer_id)).collect();
    for e in &events {
        event_store.save_event(e).unwrap();
    }

    // Peer knows first event only
    let known: HashSet<_> = vec![events[0].id].into_iter().collect();
    let batches = engine.compute_event_batches(eid, &known, &event_store).await;

    assert_eq!(batches.len(), 1);
    match &batches[0] {
        SyncMessage::EventBatch(b) => {
            assert_eq!(b.events.len(), 2);
            assert!(b.is_final);
            assert_eq!(b.batch_seq, 0);
        }
        _ => panic!("Expected EventBatch"),
    }
}

#[tokio::test]
async fn compute_event_batches_respects_batch_size() {
    let peer_id = PeerId::new();
    let engine = make_engine_with_config(peer_id, "Dev", 2);
    let (_entity_store, event_store) = make_stores();
    let eid = EntityId::new();

    // Store 5 events
    let events: Vec<Event> = (0..5).map(|_| make_event(eid, peer_id)).collect();
    for e in &events {
        event_store.save_event(e).unwrap();
    }

    // Peer knows nothing
    let known = HashSet::new();
    let batches = engine.compute_event_batches(eid, &known, &event_store).await;

    // 5 events / batch_size 2 = 3 batches (2, 2, 1)
    assert_eq!(batches.len(), 3);

    // Only last batch should be final
    for (i, batch) in batches.iter().enumerate() {
        match batch {
            SyncMessage::EventBatch(b) => {
                assert_eq!(b.batch_seq, i as u32);
                if i < 2 {
                    assert!(!b.is_final);
                    assert_eq!(b.events.len(), 2);
                } else {
                    assert!(b.is_final);
                    assert_eq!(b.events.len(), 1);
                }
            }
            _ => panic!("Expected EventBatch"),
        }
    }
}

#[tokio::test]
async fn compute_event_batches_no_events_in_store() {
    let engine = make_engine(PeerId::new());
    let (_entity_store, event_store) = make_stores();

    let batches = engine
        .compute_event_batches(EntityId::new(), &HashSet::new(), &event_store)
        .await;
    assert!(batches.is_empty());
}

// ── Handle event batch ───────────────────────────────────────────

#[tokio::test]
async fn handle_event_batch_applies_events() {
    let peer_id = PeerId::new();
    let engine = make_engine(peer_id);
    let (entity_store, event_store) = make_stores();
    let eid = EntityId::new();

    let event = make_event(eid, PeerId::new());
    let batch = EventBatchMessage {
        entity_id: eid,
        events: vec![event],
        is_final: true,
        batch_seq: 0,
    };

    let remote_peer = PeerId::new();
    let (ack, updated) = engine.handle_event_batch(&remote_peer, &batch, &entity_store, &event_store).await;

    match ack {
        SyncMessage::EventAck(a) => {
            assert_eq!(a.entity_id, eid);
            assert_eq!(a.batch_seq, 0);
            assert_eq!(a.received_count, 1);
        }
        _ => panic!("Expected EventAck"),
    }

    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0], eid);

    // Entity should exist in store
    let entity = entity_store.get_entity(&eid.to_string()).unwrap();
    assert!(entity.is_some());
}

#[tokio::test]
async fn handle_event_batch_empty() {
    let engine = make_engine(PeerId::new());
    let (entity_store, event_store) = make_stores();

    let batch = EventBatchMessage {
        entity_id: EntityId::new(),
        events: vec![],
        is_final: true,
        batch_seq: 0,
    };

    let remote_peer = PeerId::new();
    let (ack, updated) = engine.handle_event_batch(&remote_peer, &batch, &entity_store, &event_store).await;
    match ack {
        SyncMessage::EventAck(a) => assert_eq!(a.received_count, 0),
        _ => panic!("Expected EventAck"),
    }
    assert!(updated.is_empty());
}

#[tokio::test]
async fn handle_event_batch_multiple_events() {
    let engine = make_engine(PeerId::new());
    let (entity_store, event_store) = make_stores();
    let eid = EntityId::new();
    let remote_peer = PeerId::new();

    let events: Vec<Event> = (0..3).map(|_| make_event(eid, remote_peer)).collect();
    let batch = EventBatchMessage {
        entity_id: eid,
        events,
        is_final: true,
        batch_seq: 0,
    };

    let remote_peer = PeerId::new();
    let (ack, updated) = engine.handle_event_batch(&remote_peer, &batch, &entity_store, &event_store).await;
    match ack {
        SyncMessage::EventAck(a) => assert_eq!(a.received_count, 3),
        _ => panic!("Expected EventAck"),
    }
    // All 3 events target the same entity, so deduplication yields 1 unique ID
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0], eid);
}

// ── Record local event ───────────────────────────────────────────

#[tokio::test]
async fn record_local_event() {
    let peer_id = PeerId::new();
    let engine = make_engine(peer_id);
    let eid = EntityId::new();

    let event = make_event(eid, peer_id);
    engine.record_local_event(&event).await;

    // State should now track this entity
    let (_entity_store, event_store) = make_stores();
    event_store.save_event(&event).unwrap();

    let remote = PeerId::new();
    let request = SyncRequestMessage {
        entity_ids: vec![eid],
        known_event_ids: std::collections::HashMap::new(),
    };
    let response = engine.handle_sync_request(&remote, &request, &event_store).await;
    match response {
        SyncMessage::SyncState(state) => {
            assert!(state.clocks.contains_key(&eid));
        }
        _ => panic!("Expected SyncState"),
    }
}

// ── Known event IDs from store ───────────────────────────────────

#[tokio::test]
async fn known_event_ids_from_store_empty() {
    let engine = make_engine(PeerId::new());
    let (_entity_store, event_store) = make_stores();

    let ids = engine.known_event_ids_from_store(&EntityId::new(), &event_store).await;
    assert!(ids.is_empty());
}

#[tokio::test]
async fn known_event_ids_from_store_has_events() {
    let peer_id = PeerId::new();
    let engine = make_engine(peer_id);
    let (_entity_store, event_store) = make_stores();
    let eid = EntityId::new();

    let e1 = make_event(eid, peer_id);
    let e2 = make_event(eid, peer_id);
    event_store.save_event(&e1).unwrap();
    event_store.save_event(&e2).unwrap();

    let ids = engine.known_event_ids_from_store(&eid, &event_store).await;
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&e1.id));
    assert!(ids.contains(&e2.id));
}

// ── Peer management ──────────────────────────────────────────────

#[tokio::test]
async fn connected_peers_initially_empty() {
    let engine = make_engine(PeerId::new());
    assert!(engine.connected_peers().await.is_empty());
    assert!(engine.all_peers().await.is_empty());
}

#[tokio::test]
async fn peer_tracked_after_hello() {
    let engine = make_engine(PeerId::new());
    let remote = PeerId::new();

    let hello = HelloMessage::new(remote, "Remote Device");
    engine.handle_hello(&hello).await;

    let peers = engine.connected_peers().await;
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].peer_id, remote);
    assert_eq!(peers[0].device_name, "Remote Device");
}

#[tokio::test]
async fn peer_disconnected() {
    let engine = make_engine(PeerId::new());
    let remote = PeerId::new();

    let hello = HelloMessage::new(remote, "Remote");
    engine.handle_hello(&hello).await;
    assert_eq!(engine.connected_peers().await.len(), 1);

    engine.peer_disconnected(&remote).await;
    assert_eq!(engine.connected_peers().await.len(), 0);

    // Still in all_peers but disconnected
    let all = engine.all_peers().await;
    assert_eq!(all.len(), 1);
    assert!(!all[0].connected);
}

#[tokio::test]
async fn peer_disconnected_unknown_is_noop() {
    let engine = make_engine(PeerId::new());
    engine.peer_disconnected(&PeerId::new()).await;
    assert!(engine.all_peers().await.is_empty());
}

#[tokio::test]
async fn multiple_peers() {
    let engine = make_engine(PeerId::new());

    let p1 = PeerId::new();
    let p2 = PeerId::new();
    engine.handle_hello(&HelloMessage::new(p1, "Device 1")).await;
    engine.handle_hello(&HelloMessage::new(p2, "Device 2")).await;

    assert_eq!(engine.connected_peers().await.len(), 2);

    engine.peer_disconnected(&p1).await;
    let connected = engine.connected_peers().await;
    assert_eq!(connected.len(), 1);
    assert_eq!(connected[0].peer_id, p2);
}

// ── set_acl_handler and ACL event routing ────────────────────────

#[tokio::test]
async fn set_acl_handler_routes_acl_events() {
    use privstack_sync::policy::{EnterpriseSyncPolicy, EntityAcl, SyncRole};
    use privstack_sync::AclApplicator;

    let policy = std::sync::Arc::new(EnterpriseSyncPolicy::new());
    let local = PeerId::new();
    let remote = PeerId::new();
    let entity = EntityId::new();

    policy.known_peers.write().await.insert(remote);
    let acl = EntityAcl::new(entity).with_peer_role(remote, SyncRole::Admin);
    policy.acls.write().await.insert(entity, acl);

    let mut engine = SyncEngine::with_policy(local, SyncConfig::default(), policy.clone());
    let acl_applicator = std::sync::Arc::new(AclApplicator::new(policy.clone()));
    engine.set_acl_handler(acl_applicator);

    let (entity_store, event_store) = make_stores();

    // Create an ACL grant event
    let target_peer = PeerId::new();
    let acl_event = Event::new(
        entity,
        remote,
        HybridTimestamp::now(),
        EventPayload::AclGrantPeer {
            entity_id: entity.to_string(),
            peer_id: target_peer.to_string(),
            role: "Viewer".to_string(),
        },
    );

    let batch = EventBatchMessage {
        entity_id: entity,
        events: vec![acl_event],
        is_final: true,
        batch_seq: 0,
    };

    let (ack, updated) = engine
        .handle_event_batch(&remote, &batch, &entity_store, &event_store)
        .await;

    match ack {
        SyncMessage::EventAck(a) => {
            assert_eq!(a.received_count, 1, "ACL event should be handled by acl_handler");
        }
        other => panic!("Expected EventAck, got {:?}", other),
    }
    assert!(!updated.is_empty());

    // Verify the ACL was actually applied to policy
    let role = policy.resolve_role(&target_peer, &entity).await;
    assert_eq!(role, Some(SyncRole::Viewer));
}

// ── Peer disconnect preserving peer in all_peers ────────────────

#[tokio::test]
async fn peer_disconnect_preserves_in_all_peers() {
    let engine = make_engine(PeerId::new());
    let remote = PeerId::new();

    let hello = HelloMessage::new(remote, "Remote");
    engine.handle_hello(&hello).await;

    engine.peer_disconnected(&remote).await;

    // connected_peers should be empty
    assert!(engine.connected_peers().await.is_empty());

    // all_peers should still have the peer
    let all = engine.all_peers().await;
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].peer_id, remote);
    assert!(!all[0].connected);
}

// ── Reverse delta with policy denial ────────────────────────────

#[tokio::test]
async fn reverse_delta_filtered_by_policy() {
    use privstack_sync::policy::{EnterpriseSyncPolicy, EntityAcl, SyncRole};

    let policy = std::sync::Arc::new(EnterpriseSyncPolicy::new());
    let local = PeerId::new();
    let remote = PeerId::new();
    let entity = EntityId::new();

    policy.known_peers.write().await.insert(remote);

    // Remote is Editor (can write and read)
    // But local events should not be sent back if policy denies event_send
    // We'll test that reverse delta goes through on_event_send.
    // Give remote Viewer: can read (event_send allowed), but can't write
    let acl = EntityAcl::new(entity).with_peer_role(remote, SyncRole::Viewer);
    policy.acls.write().await.insert(entity, acl);

    let engine = SyncEngine::with_policy(local, SyncConfig::default(), policy);
    let (entity_store, event_store) = make_stores();

    // Store local events
    let local_events: Vec<Event> = (0..3).map(|_| make_event(entity, local)).collect();
    for e in &local_events {
        event_store.save_event(e).unwrap();
        engine.record_local_event(e).await;
    }

    // Remote sends empty final batch (triggering reverse delta)
    let batch = EventBatchMessage {
        entity_id: entity,
        events: vec![],
        is_final: true,
        batch_seq: 0,
    };

    let (ack, _) = engine
        .handle_event_batch(&remote, &batch, &entity_store, &event_store)
        .await;

    match ack {
        SyncMessage::EventAck(a) => {
            // Viewer can read, so event_send should be allowed
            assert_eq!(a.events.len(), 3, "viewer should receive reverse delta events");
        }
        other => panic!("Expected EventAck, got {:?}", other),
    }
}

#[tokio::test]
async fn reverse_delta_denied_when_no_role() {
    use privstack_sync::policy::{EnterpriseSyncPolicy, EntityAcl};

    let policy = std::sync::Arc::new(EnterpriseSyncPolicy::new());
    let local = PeerId::new();
    let remote = PeerId::new();
    let entity = EntityId::new();

    policy.known_peers.write().await.insert(remote);
    // ACL exists but remote has no role (empty ACL)
    let acl = EntityAcl::new(entity);
    policy.acls.write().await.insert(entity, acl);

    let engine = SyncEngine::with_policy(local, SyncConfig::default(), policy);
    let (entity_store, event_store) = make_stores();

    let local_events: Vec<Event> = (0..3).map(|_| make_event(entity, local)).collect();
    for e in &local_events {
        event_store.save_event(e).unwrap();
        engine.record_local_event(e).await;
    }

    let batch = EventBatchMessage {
        entity_id: entity,
        events: vec![],
        is_final: true,
        batch_seq: 0,
    };

    let (ack, _) = engine
        .handle_event_batch(&remote, &batch, &entity_store, &event_store)
        .await;

    match ack {
        SyncMessage::EventAck(a) => {
            assert!(a.events.is_empty(), "no-role peer should not get reverse delta");
        }
        other => panic!("Expected EventAck, got {:?}", other),
    }
}

// ── Engine policy accessor ──────────────────────────────────────

#[tokio::test]
async fn engine_policy_returns_arc() {
    let engine = make_engine(PeerId::new());
    let _policy = engine.policy();
    // Just verify it compiles and doesn't panic
}

// ── Non-final batch handling (no reverse delta) ─────────────────

#[tokio::test]
async fn handle_event_batch_non_final_no_reverse_delta() {
    let peer_id = PeerId::new();
    let engine = make_engine(peer_id);
    let (entity_store, event_store) = make_stores();
    let eid = EntityId::new();
    let remote = PeerId::new();

    // Store a local event so reverse delta would exist if batch were final
    let local_event = make_event(eid, peer_id);
    event_store.save_event(&local_event).unwrap();
    engine.record_local_event(&local_event).await;

    let remote_event = make_event(eid, remote);
    let batch = EventBatchMessage {
        entity_id: eid,
        events: vec![remote_event],
        is_final: false, // non-final
        batch_seq: 0,
    };

    let (ack, _) = engine.handle_event_batch(&remote, &batch, &entity_store, &event_store).await;
    match ack {
        SyncMessage::EventAck(a) => {
            assert_eq!(a.received_count, 1);
            // Non-final batch should NOT include reverse delta events
            assert!(a.events.is_empty(), "non-final batch should have no reverse delta");
        }
        _ => panic!("Expected EventAck"),
    }
}

// ── peer_known_ids cleanup after final batch ────────────────────

#[tokio::test]
async fn peer_known_ids_cleaned_up_after_final_batch() {
    let peer_id = PeerId::new();
    let engine = make_engine(peer_id);
    let (entity_store, event_store) = make_stores();
    let eid = EntityId::new();
    let remote = PeerId::new();

    // Send a sync request with known_event_ids to populate peer_known_ids
    let request = SyncRequestMessage {
        entity_ids: vec![eid],
        known_event_ids: {
            let mut map = std::collections::HashMap::new();
            map.insert(eid, vec![EventId::new()]);
            map
        },
    };
    engine.handle_sync_request(&remote, &request, &event_store).await;

    // Now handle a final batch - should clean up peer_known_ids
    let batch = EventBatchMessage {
        entity_id: eid,
        events: vec![],
        is_final: true,
        batch_seq: 0,
    };
    engine.handle_event_batch(&remote, &batch, &entity_store, &event_store).await;

    // peer_known_ids should be cleaned up (internal, but we can verify
    // no panic occurs and the engine works fine afterwards)
}

// ── handle_hello with device_id ─────────────────────────────────

#[tokio::test]
async fn handle_hello_with_device_id() {
    use privstack_sync::policy::{EnterpriseSyncPolicy, DeviceId};

    let policy = std::sync::Arc::new(EnterpriseSyncPolicy::new());
    let local = PeerId::new();
    let remote = PeerId::new();

    policy.known_peers.write().await.insert(local);
    policy.known_peers.write().await.insert(remote);

    // Set device limit
    policy.device_limits.write().await.insert(remote, 1);

    let engine = SyncEngine::with_policy(local, SyncConfig::default(), policy.clone());

    let d = DeviceId::new();
    let mut hello = HelloMessage::new(remote, "Device A");
    hello.device_id = Some(d.0.to_string());
    let response = engine.handle_hello(&hello).await;
    match response {
        SyncMessage::HelloAck(ack) => assert!(ack.accepted),
        other => panic!("Expected HelloAck, got {:?}", other),
    }

    // Second device should be rejected
    let d2 = DeviceId::new();
    let mut hello2 = HelloMessage::new(remote, "Device B");
    hello2.device_id = Some(d2.0.to_string());
    let response2 = engine.handle_hello(&hello2).await;
    match response2 {
        SyncMessage::HelloAck(ack) => assert!(!ack.accepted),
        other => panic!("Expected rejection, got {:?}", other),
    }
}

// ── handle_hello device limit with no device_id ─────────────────

#[tokio::test]
async fn handle_hello_device_limit_no_device_id() {
    use privstack_sync::policy::EnterpriseSyncPolicy;

    let policy = std::sync::Arc::new(EnterpriseSyncPolicy::new());
    let local = PeerId::new();
    let remote = PeerId::new();

    policy.known_peers.write().await.insert(local);
    policy.known_peers.write().await.insert(remote);
    policy.device_limits.write().await.insert(remote, 2);

    let engine = SyncEngine::with_policy(local, SyncConfig::default(), policy);

    // No device_id provided but limit exists
    let hello = HelloMessage::new(remote, "No Device ID");
    let response = engine.handle_hello(&hello).await;
    match response {
        SyncMessage::HelloAck(ack) => {
            assert!(!ack.accepted, "should reject when no device_id but limit configured");
        }
        other => panic!("Expected rejection, got {:?}", other),
    }
}

// ── ACL handler error path ──────────────────────────────────────

#[tokio::test]
async fn acl_handler_error_falls_through_to_normal_applicator() {
    use privstack_sync::AclApplicator;
    use privstack_sync::policy::EnterpriseSyncPolicy;

    let policy = std::sync::Arc::new(EnterpriseSyncPolicy::new());
    let local = PeerId::new();
    let remote = PeerId::new();
    let entity = EntityId::new();

    let mut engine = SyncEngine::with_policy(local, SyncConfig::default(), Arc::new(privstack_sync::AllowAllPolicy));
    let acl_applicator = std::sync::Arc::new(AclApplicator::new(policy));
    engine.set_acl_handler(acl_applicator);

    let (entity_store, event_store) = make_stores();

    // Send an ACL event with invalid entity_id string - acl_handler will return Err
    let bad_acl_event = Event::new(
        entity,
        remote,
        HybridTimestamp::now(),
        EventPayload::AclGrantPeer {
            entity_id: "not-a-uuid".to_string(),
            peer_id: PeerId::new().to_string(),
            role: "Editor".to_string(),
        },
    );

    let batch = EventBatchMessage {
        entity_id: entity,
        events: vec![bad_acl_event],
        is_final: true,
        batch_seq: 0,
    };

    let (ack, _) = engine.handle_event_batch(&remote, &batch, &entity_store, &event_store).await;
    // Should not panic - error is logged and falls through
    match ack {
        SyncMessage::EventAck(a) => {
            // The event falls through to normal applicator which may or may not apply it
            let _ = a.received_count;
        }
        other => panic!("Expected EventAck, got {:?}", other),
    }
}

// ── SyncConfig Debug impl ───────────────────────────────────────

#[test]
fn sync_config_debug() {
    let cfg = SyncConfig::default();
    let debug = format!("{:?}", cfg);
    assert!(debug.contains("device_name"));
    assert!(debug.contains("batch_size"));
}

#[test]
fn sync_config_clone() {
    let cfg = SyncConfig {
        device_name: "Test".to_string(),
        batch_size: 42,
        timeout_ms: 1000,
    };
    let cloned = cfg.clone();
    assert_eq!(cloned.device_name, "Test");
    assert_eq!(cloned.batch_size, 42);
}

// ── make_sync_request with known event IDs ──────────────────────

#[tokio::test]
async fn make_sync_request_with_known_events() {
    let peer_id = PeerId::new();
    let engine = make_engine(peer_id);
    let (_entity_store, event_store) = make_stores();
    let eid = EntityId::new();

    // Store some events
    let e1 = make_event(eid, peer_id);
    let e2 = make_event(eid, peer_id);
    event_store.save_event(&e1).unwrap();
    event_store.save_event(&e2).unwrap();

    let msg = engine.make_sync_request(vec![eid], &event_store).await;
    match msg {
        SyncMessage::SyncRequest(r) => {
            assert_eq!(r.entity_ids, vec![eid]);
            // Should include known event IDs
            assert!(r.known_event_ids.contains_key(&eid));
            assert_eq!(r.known_event_ids.get(&eid).unwrap().len(), 2);
        }
        _ => panic!("Expected SyncRequest"),
    }
}

// ── make_sync_state ─────────────────────────────────────────────

#[tokio::test]
async fn make_sync_state_with_events() {
    let peer_id = PeerId::new();
    let engine = make_engine(peer_id);
    let (_entity_store, event_store) = make_stores();
    let eid = EntityId::new();

    let event = make_event(eid, peer_id);
    event_store.save_event(&event).unwrap();
    engine.record_local_event(&event).await;

    let msg = engine.make_sync_state(&[eid], &event_store).await;
    match msg {
        SyncMessage::SyncState(state) => {
            assert!(state.clocks.contains_key(&eid));
            assert_eq!(state.event_counts.get(&eid), Some(&1));
        }
        _ => panic!("Expected SyncState"),
    }
}

#[tokio::test]
async fn make_sync_state_entity_not_tracked() {
    let engine = make_engine(PeerId::new());
    let (_entity_store, event_store) = make_stores();
    let eid = EntityId::new();

    // Entity has events in store but not tracked in engine state
    let event = make_event(eid, PeerId::new());
    event_store.save_event(&event).unwrap();

    let msg = engine.make_sync_state(&[eid], &event_store).await;
    match msg {
        SyncMessage::SyncState(state) => {
            // Should still include the entity (from store)
            assert!(state.known_event_ids.contains_key(&eid));
        }
        _ => panic!("Expected SyncState"),
    }
}

use privstack_types::EventId;
