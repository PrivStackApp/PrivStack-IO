use libp2p::identity::Keypair;
use libp2p::PeerId as Libp2pPeerId;
use privstack_sync::p2p::connection::P2pConnection;
use privstack_types::PeerId;

fn make_connection() -> (P2pConnection, PeerId, Libp2pPeerId) {
    let peer_id = PeerId::new();
    let libp2p_id: Libp2pPeerId = Keypair::generate_ed25519().public().into();
    let conn = P2pConnection::new(peer_id, "Test Device".to_string(), libp2p_id);
    (conn, peer_id, libp2p_id)
}

#[test]
fn connection_creation() {
    let (conn, peer_id, libp2p_id) = make_connection();
    assert_eq!(conn.peer_id(), peer_id);
    assert_eq!(conn.device_name(), "Test Device");
    assert_eq!(conn.libp2p_peer_id(), libp2p_id);
    assert!(conn.is_connected());
}

#[test]
fn connection_close() {
    let (conn, _, _) = make_connection();
    assert!(conn.is_connected());
    conn.close();
    assert!(!conn.is_connected());
}

#[test]
fn connection_close_is_idempotent() {
    let (conn, _, _) = make_connection();
    conn.close();
    conn.close(); // should not panic
    assert!(!conn.is_connected());
}

#[test]
fn connection_set_device_name() {
    let peer_id = PeerId::new();
    let libp2p_id: Libp2pPeerId = Keypair::generate_ed25519().public().into();
    let mut conn = P2pConnection::new(peer_id, "Old".to_string(), libp2p_id);
    assert_eq!(conn.device_name(), "Old");

    conn.set_device_name("New".to_string());
    assert_eq!(conn.device_name(), "New");
}

#[test]
fn connection_debug_format() {
    let (conn, _, _) = make_connection();
    let debug = format!("{:?}", conn);
    assert!(debug.contains("P2pConnection"));
    assert!(debug.contains("Test Device"));
    assert!(debug.contains("true")); // connected
}

#[test]
fn connection_debug_after_close() {
    let (conn, _, _) = make_connection();
    conn.close();
    let debug = format!("{:?}", conn);
    assert!(debug.contains("false")); // disconnected
}
