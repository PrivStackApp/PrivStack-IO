//! P2P connection handle.
//!
//! With the request-response pattern, connections are lightweight handles
//! for tracking peer metadata. Actual message exchange happens via
//! P2pTransport::send_request() and recv_request().

use libp2p::PeerId as Libp2pPeerId;
use privstack_types::PeerId;
use std::sync::atomic::{AtomicBool, Ordering};

/// A P2P connection handle to a remote peer.
///
/// This is a lightweight metadata handle. With request-response,
/// actual message exchange goes through the transport layer.
pub struct P2pConnection {
    /// The remote peer's PrivStack ID.
    peer_id: PeerId,
    /// The remote peer's device name.
    device_name: String,
    /// The remote peer's libp2p ID.
    libp2p_peer_id: Libp2pPeerId,
    /// Whether the connection is considered active.
    connected: AtomicBool,
}

impl std::fmt::Debug for P2pConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("P2pConnection")
            .field("peer_id", &self.peer_id)
            .field("device_name", &self.device_name)
            .field("libp2p_peer_id", &self.libp2p_peer_id)
            .field("connected", &self.connected.load(Ordering::SeqCst))
            .finish()
    }
}

impl P2pConnection {
    /// Creates a new P2P connection handle.
    pub fn new(peer_id: PeerId, device_name: String, libp2p_peer_id: Libp2pPeerId) -> Self {
        Self {
            peer_id,
            device_name,
            libp2p_peer_id,
            connected: AtomicBool::new(true),
        }
    }

    /// Returns the PrivStack peer ID.
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Returns the device name.
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Returns the libp2p peer ID.
    pub fn libp2p_peer_id(&self) -> Libp2pPeerId {
        self.libp2p_peer_id
    }

    /// Sets the device name (after handshake).
    pub fn set_device_name(&mut self, name: String) {
        self.device_name = name;
    }

    /// Marks the connection as closed.
    pub fn close(&self) {
        self.connected.store(false, Ordering::SeqCst);
    }

    /// Returns whether the connection is still alive.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}
