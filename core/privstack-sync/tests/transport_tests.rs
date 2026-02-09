use privstack_sync::protocol::{
    ErrorMessage, HelloMessage, SyncMessage, SyncRequestMessage,
};
use privstack_sync::transport::mock::MockConnection;
use privstack_sync::transport::{DiscoveredPeer, DiscoveryMethod};
use privstack_types::{EntityId, PeerId};

// ── DiscoveryMethod ─────────────────────────────────────────────

#[test]
fn discovery_method_eq_and_copy() {
    let a = DiscoveryMethod::Mdns;
    let b = a; // Copy
    assert_eq!(a, b);
}

#[test]
fn discovery_method_all_variants() {
    let methods = [
        DiscoveryMethod::Mdns,
        DiscoveryMethod::Dht,
        DiscoveryMethod::Manual,
        DiscoveryMethod::CloudRelay,
    ];
    // All distinct
    for (i, a) in methods.iter().enumerate() {
        for (j, b) in methods.iter().enumerate() {
            if i == j {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b);
            }
        }
    }
}

// ── DiscoveredPeer ──────────────────────────────────────────────

#[test]
fn discovered_peer_clone() {
    let peer = DiscoveredPeer {
        peer_id: PeerId::new(),
        device_name: Some("Dev".into()),
        discovery_method: DiscoveryMethod::Mdns,
        addresses: vec!["addr1".into()],
    };
    let cloned = peer.clone();
    assert_eq!(cloned.peer_id, peer.peer_id);
    assert_eq!(cloned.device_name, peer.device_name);
    assert_eq!(cloned.discovery_method, peer.discovery_method);
    assert_eq!(cloned.addresses, peer.addresses);
}

#[test]
fn discovered_peer_no_device_name() {
    let peer = DiscoveredPeer {
        peer_id: PeerId::new(),
        device_name: None,
        discovery_method: DiscoveryMethod::Dht,
        addresses: vec![],
    };
    assert!(peer.device_name.is_none());
}

#[test]
fn discovered_peer_debug() {
    let peer = DiscoveredPeer {
        peer_id: PeerId::new(),
        device_name: Some("Dev".into()),
        discovery_method: DiscoveryMethod::Manual,
        addresses: vec![],
    };
    let debug = format!("{:?}", peer);
    assert!(debug.contains("DiscoveredPeer"));
}

// ── MockConnection basic ────────────────────────────────────────

#[test]
fn mock_connection_new() {
    let peer = PeerId::new();
    let conn = MockConnection::new(peer, "Device");
    assert_eq!(conn.peer_id(), peer);
    assert_eq!(conn.device_name(), "Device");
    assert!(conn.is_connected());
}

#[test]
fn mock_connection_close() {
    let mut conn = MockConnection::new(PeerId::new(), "Test");
    assert!(conn.is_connected());
    conn.close();
    assert!(!conn.is_connected());
}

// ── MockConnection::pair ────────────────────────────────────────

#[test]
fn mock_connection_pair_creates_linked_connections() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let (conn1, conn2) = MockConnection::pair(p1, "Dev1", p2, "Dev2");

    // conn1 sees peer2, conn2 sees peer1
    assert_eq!(conn1.peer_id(), p2);
    assert_eq!(conn1.device_name(), "Dev2");
    assert_eq!(conn2.peer_id(), p1);
    assert_eq!(conn2.device_name(), "Dev1");
}

#[test]
fn mock_connection_send_receive() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let (mut conn1, mut conn2) = MockConnection::pair(p1, "Dev1", p2, "Dev2");

    let msg = SyncMessage::Hello(HelloMessage::new(p1, "Dev1"));
    conn1.send(msg).unwrap();

    let received = conn2.receive().unwrap();
    assert!(received.is_some());
}

#[test]
fn mock_connection_send_when_closed_errors() {
    let mut conn = MockConnection::new(PeerId::new(), "Test");
    conn.close();

    let msg = SyncMessage::Error(ErrorMessage::new(1, "test"));
    let result = conn.send(msg);
    assert!(result.is_err());
}

#[test]
fn mock_connection_receive_when_closed_returns_none() {
    let mut conn = MockConnection::new(PeerId::new(), "Test");
    conn.close();

    let result = conn.receive().unwrap();
    assert!(result.is_none());
}

#[test]
fn mock_connection_receive_empty_returns_none() {
    let mut conn = MockConnection::new(PeerId::new(), "Test");
    let result = conn.receive().unwrap();
    assert!(result.is_none());
}

// ── MockConnection queue_incoming / take_outgoing ───────────────

#[test]
fn mock_connection_queue_incoming_and_receive() {
    let mut conn = MockConnection::new(PeerId::new(), "Test");
    let msg = SyncMessage::SyncRequest(SyncRequestMessage { entity_ids: vec![EntityId::new()], known_event_ids: std::collections::HashMap::new() });
    conn.queue_incoming(msg);

    let received = conn.receive().unwrap().unwrap();
    assert!(matches!(received, SyncMessage::SyncRequest(_)));
}

#[test]
fn mock_connection_send_and_take_outgoing() {
    let mut conn = MockConnection::new(PeerId::new(), "Test");
    let msg = SyncMessage::Error(ErrorMessage::new(1, "test"));
    conn.send(msg).unwrap();

    let taken = conn.take_outgoing().unwrap();
    assert!(matches!(taken, SyncMessage::Error(_)));
    // Queue is now empty
    assert!(conn.take_outgoing().is_none());
}

#[test]
fn mock_connection_take_outgoing_empty() {
    let conn = MockConnection::new(PeerId::new(), "Test");
    assert!(conn.take_outgoing().is_none());
}

// ── MockConnection bidirectional ────────────────────────────────

#[test]
fn mock_connection_bidirectional_exchange() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let (mut c1, mut c2) = MockConnection::pair(p1, "D1", p2, "D2");

    // c1 sends to c2
    c1.send(SyncMessage::Hello(HelloMessage::new(p1, "D1"))).unwrap();
    // c2 sends to c1
    c2.send(SyncMessage::Error(ErrorMessage::new(1, "nope"))).unwrap();

    // c2 receives c1's message
    let from_c1 = c2.receive().unwrap().unwrap();
    assert!(matches!(from_c1, SyncMessage::Hello(_)));

    // c1 receives c2's message
    let from_c2 = c1.receive().unwrap().unwrap();
    assert!(matches!(from_c2, SyncMessage::Error(_)));
}

#[test]
fn mock_connection_multiple_messages_fifo() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let (mut c1, mut c2) = MockConnection::pair(p1, "D1", p2, "D2");

    c1.send(SyncMessage::Hello(HelloMessage::new(p1, "D1"))).unwrap();
    c1.send(SyncMessage::Error(ErrorMessage::new(2, "second"))).unwrap();

    let first = c2.receive().unwrap().unwrap();
    assert!(matches!(first, SyncMessage::Hello(_)));

    let second = c2.receive().unwrap().unwrap();
    assert!(matches!(second, SyncMessage::Error(_)));

    // No more
    assert!(c2.receive().unwrap().is_none());
}
