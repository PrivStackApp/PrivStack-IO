use libp2p::identity::Keypair;
use libp2p::PeerId as Libp2pPeerId;
use privstack_sync::p2p::transport::{P2pConfig, P2pTransport};
use privstack_sync::SyncTransport;
use privstack_types::PeerId;
use std::time::Duration;

#[tokio::test]
async fn create_transport() {
    let peer_id = PeerId::new();
    let config = P2pConfig::default();

    let transport = P2pTransport::new(peer_id, config);
    assert!(transport.is_ok());

    let transport = transport.unwrap();
    assert_eq!(transport.local_peer_id(), peer_id);
    assert!(!transport.is_running());
}

#[tokio::test]
async fn peer_id_mapping_deterministic() {
    let libp2p_id: Libp2pPeerId = Keypair::generate_ed25519().public().into();

    let id1 = P2pTransport::map_peer_id(&libp2p_id);
    let id2 = P2pTransport::map_peer_id(&libp2p_id);

    assert_eq!(id1, id2);
}

// ── P2pConfig derived traits ────────────────────────────────────

#[test]
fn p2p_config_default_values() {
    let config = P2pConfig::default();
    assert_eq!(config.listen_addrs.len(), 2);
    assert_eq!(config.bootstrap_nodes.len(), 1);
    assert!(config.sync_code_hash.is_none());
    assert_eq!(config.device_name, "PrivStack Device");
    assert!(config.enable_mdns);
    assert!(config.enable_dht);
    assert_eq!(config.idle_timeout, Duration::from_secs(60));
}

#[test]
fn p2p_config_debug() {
    let config = P2pConfig::default();
    let debug = format!("{:?}", config);
    assert!(debug.contains("P2pConfig"));
    assert!(debug.contains("listen_addrs"));
    assert!(debug.contains("enable_mdns"));
}

#[test]
fn p2p_config_clone() {
    let config = P2pConfig::default();
    let cloned = config.clone();
    assert_eq!(cloned.device_name, config.device_name);
    assert_eq!(cloned.enable_mdns, config.enable_mdns);
    assert_eq!(cloned.enable_dht, config.enable_dht);
    assert_eq!(cloned.idle_timeout, config.idle_timeout);
    assert_eq!(cloned.listen_addrs.len(), config.listen_addrs.len());
    assert_eq!(cloned.bootstrap_nodes.len(), config.bootstrap_nodes.len());
    assert_eq!(cloned.sync_code_hash, config.sync_code_hash);
}

// ── P2pTransport::with_keypair ──────────────────────────────────

#[tokio::test]
async fn create_transport_with_keypair() {
    let peer_id = PeerId::new();
    let keypair = Keypair::generate_ed25519();
    let expected_libp2p_id = Libp2pPeerId::from(keypair.public());
    let config = P2pConfig::default();

    let transport = P2pTransport::with_keypair(peer_id, keypair, config).unwrap();
    assert_eq!(transport.libp2p_peer_id(), expected_libp2p_id);
    assert_eq!(transport.local_peer_id(), peer_id);
    assert!(!transport.is_running());
}

// ── P2pTransport::libp2p_peer_id accessor ───────────────────────

#[tokio::test]
async fn libp2p_peer_id_matches_keypair() {
    let keypair = Keypair::generate_ed25519();
    let expected = Libp2pPeerId::from(keypair.public());
    let transport =
        P2pTransport::with_keypair(PeerId::new(), keypair, P2pConfig::default()).unwrap();
    assert_eq!(transport.libp2p_peer_id(), expected);
}

// ── discovered_peers on fresh transport ─────────────────────────

#[tokio::test]
async fn discovered_peers_empty_on_fresh_transport() {
    let transport = P2pTransport::new(PeerId::new(), P2pConfig::default()).unwrap();
    assert!(transport.discovered_peers().is_empty());
}

#[tokio::test]
async fn discovered_peers_async_empty_on_fresh_transport() {
    let transport = P2pTransport::new(PeerId::new(), P2pConfig::default()).unwrap();
    let peers = transport.discovered_peers_async().await;
    assert!(peers.is_empty());
}

// ── stop on non-running transport ───────────────────────────────

#[tokio::test]
async fn stop_when_not_running_is_ok() {
    let mut transport = P2pTransport::new(PeerId::new(), P2pConfig::default()).unwrap();
    assert!(!transport.is_running());
    let result = transport.stop().await;
    assert!(result.is_ok());
    assert!(!transport.is_running());
}

// ── error paths when transport not started (no command_tx) ──────

#[tokio::test]
async fn send_request_fails_when_not_running() {
    let transport = P2pTransport::new(PeerId::new(), P2pConfig::default()).unwrap();
    let target = PeerId::new();
    let msg = privstack_sync::protocol::SyncMessage::Hello(
        privstack_sync::protocol::HelloMessage::new(PeerId::new(), "test"),
    );
    let result = transport.send_request(&target, msg).await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("not running"));
}

#[tokio::test]
async fn publish_to_sync_group_fails_when_not_running() {
    let transport = P2pTransport::new(PeerId::new(), P2pConfig::default()).unwrap();
    let result = transport.publish_to_sync_group(b"test-hash").await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("not running"));
}

#[tokio::test]
async fn discover_sync_group_fails_when_not_running() {
    let transport = P2pTransport::new(PeerId::new(), P2pConfig::default()).unwrap();
    let result = transport.discover_sync_group(b"test-hash").await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("not running"));
}

// ── map_peer_id produces different IDs for different peers ──────

#[test]
fn map_peer_id_different_for_different_peers() {
    let id1: Libp2pPeerId = Keypair::generate_ed25519().public().into();
    let id2: Libp2pPeerId = Keypair::generate_ed25519().public().into();

    let mapped1 = P2pTransport::map_peer_id(&id1);
    let mapped2 = P2pTransport::map_peer_id(&id2);

    assert_ne!(mapped1, mapped2);
}

// ── P2pConfig with custom values ────────────────────────────────

#[test]
fn p2p_config_custom_values() {
    let config = P2pConfig {
        listen_addrs: vec!["/ip4/127.0.0.1/udp/9999/quic-v1".parse().unwrap()],
        bootstrap_nodes: vec![],
        sync_code_hash: Some(vec![1, 2, 3]),
        device_name: "Custom Device".to_string(),
        enable_mdns: false,
        enable_dht: false,
        idle_timeout: Duration::from_secs(120),
    };
    assert_eq!(config.listen_addrs.len(), 1);
    assert!(config.bootstrap_nodes.is_empty());
    assert_eq!(config.sync_code_hash, Some(vec![1, 2, 3]));
    assert_eq!(config.device_name, "Custom Device");
    assert!(!config.enable_mdns);
    assert!(!config.enable_dht);
    assert_eq!(config.idle_timeout, Duration::from_secs(120));
}

// ── send_response with invalid token ────────────────────────────

#[tokio::test]
async fn send_response_fails_when_not_running() {
    let transport = P2pTransport::new(PeerId::new(), P2pConfig::default()).unwrap();
    // ResponseToken wrapping a dummy value - transport isn't running so it
    // will fail before even trying to downcast.
    let token = privstack_sync::transport::ResponseToken::new(42u32);
    let msg = privstack_sync::protocol::SyncMessage::Hello(
        privstack_sync::protocol::HelloMessage::new(PeerId::new(), "test"),
    );
    let result = transport.send_response(token, msg).await;
    assert!(result.is_err());
}
