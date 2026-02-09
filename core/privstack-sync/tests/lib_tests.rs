use privstack_sync::{SyncConfig, SyncEngine, PROTOCOL_VERSION};
use privstack_types::PeerId;

#[test]
fn sync_engine_creation() {
    let peer_id = PeerId::new();
    let config = SyncConfig::default();
    let engine = SyncEngine::new(peer_id, config);

    assert_eq!(engine.peer_id(), peer_id);
}

#[test]
fn protocol_version() {
    assert_eq!(PROTOCOL_VERSION, 1);
}
