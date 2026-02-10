//! Transport layer abstraction.
//!
//! Defines traits for different sync transports (P2P, cloud storage, etc.)
//! allowing the sync engine to work with any backend.

use crate::error::SyncResult;
use crate::protocol::SyncMessage;
use async_trait::async_trait;
use privstack_types::PeerId;
use std::any::Any;

/// Opaque token used to send a response back to an incoming request.
/// Each transport implementation wraps its own channel type inside this.
pub struct ResponseToken(Box<dyn Any + Send>);

impl ResponseToken {
    /// Wraps a transport-specific response channel.
    pub fn new<T: Any + Send + 'static>(inner: T) -> Self {
        Self(Box::new(inner))
    }

    /// Unwraps back to the transport-specific type.
    pub fn downcast<T: Any + Send + 'static>(self) -> Option<T> {
        self.0.downcast::<T>().ok().map(|b| *b)
    }
}

/// An incoming sync request received by the transport.
pub struct IncomingSyncRequest {
    /// The peer that sent the request.
    pub peer_id: PeerId,
    /// The request message.
    pub message: SyncMessage,
    /// Opaque token to send the response back through.
    pub response_token: ResponseToken,
}

/// A sync transport that can discover peers and exchange messages.
#[async_trait]
pub trait SyncTransport: Send + Sync {
    /// Starts the transport (begins listening and discovering).
    async fn start(&mut self) -> SyncResult<()>;

    /// Stops the transport.
    async fn stop(&mut self) -> SyncResult<()>;

    /// Returns whether the transport is running.
    fn is_running(&self) -> bool;

    /// Returns the local peer ID.
    fn local_peer_id(&self) -> PeerId;

    /// Returns a list of discovered peers.
    fn discovered_peers(&self) -> Vec<DiscoveredPeer>;

    /// Returns a list of discovered peers (async version for transports
    /// that need async locking).
    async fn discovered_peers_async(&self) -> Vec<DiscoveredPeer> {
        self.discovered_peers()
    }

    /// Sends a request to a peer and waits for the response.
    async fn send_request(
        &self,
        peer_id: &PeerId,
        message: SyncMessage,
    ) -> SyncResult<SyncMessage>;

    /// Receives the next incoming sync request.
    /// Returns `None` if the transport is shutting down.
    async fn recv_request(&self) -> Option<IncomingSyncRequest>;

    /// Sends a response to a previously received request.
    async fn send_response(
        &self,
        token: ResponseToken,
        message: SyncMessage,
    ) -> SyncResult<()>;
}

/// Information about a discovered peer.
#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    /// The peer's ID.
    pub peer_id: PeerId,
    /// The peer's device name (if known).
    pub device_name: Option<String>,
    /// How the peer was discovered.
    pub discovery_method: DiscoveryMethod,
    /// Addresses where the peer can be reached.
    pub addresses: Vec<String>,
}

/// How a peer was discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryMethod {
    /// Discovered via mDNS on local network.
    Mdns,
    /// Discovered via DHT.
    Dht,
    /// Manually added.
    Manual,
    /// Discovered via cloud relay.
    CloudRelay,
}

/// A mock transport for testing.
pub mod mock {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// A mock peer connection for testing message exchange.
    #[derive(Debug)]
    pub struct MockConnection {
        peer_id: PeerId,
        device_name: String,
        incoming: Arc<Mutex<VecDeque<SyncMessage>>>,
        outgoing: Arc<Mutex<VecDeque<SyncMessage>>>,
        connected: bool,
    }

    impl MockConnection {
        /// Creates a new mock connection.
        pub fn new(peer_id: PeerId, device_name: impl Into<String>) -> Self {
            Self {
                peer_id,
                device_name: device_name.into(),
                incoming: Arc::new(Mutex::new(VecDeque::new())),
                outgoing: Arc::new(Mutex::new(VecDeque::new())),
                connected: true,
            }
        }

        /// Creates a pair of connected mock connections.
        pub fn pair(
            peer1: PeerId,
            name1: impl Into<String>,
            peer2: PeerId,
            name2: impl Into<String>,
        ) -> (Self, Self) {
            let queue1 = Arc::new(Mutex::new(VecDeque::new()));
            let queue2 = Arc::new(Mutex::new(VecDeque::new()));

            let conn1 = Self {
                peer_id: peer2,
                device_name: name2.into(),
                incoming: queue1.clone(),
                outgoing: queue2.clone(),
                connected: true,
            };

            let conn2 = Self {
                peer_id: peer1,
                device_name: name1.into(),
                incoming: queue2,
                outgoing: queue1,
                connected: true,
            };

            (conn1, conn2)
        }

        /// The remote peer's ID.
        pub fn peer_id(&self) -> PeerId {
            self.peer_id
        }

        /// The remote peer's device name.
        pub fn device_name(&self) -> &str {
            &self.device_name
        }

        /// Queues a message to be received.
        pub fn queue_incoming(&self, message: SyncMessage) {
            self.incoming.lock().unwrap().push_back(message);
        }

        /// Gets the next outgoing message.
        pub fn take_outgoing(&self) -> Option<SyncMessage> {
            self.outgoing.lock().unwrap().pop_front()
        }

        /// Sends a message (adds to outgoing queue).
        pub fn send(&mut self, message: SyncMessage) -> crate::error::SyncResult<()> {
            if !self.connected {
                return Err(crate::error::SyncError::Network("not connected".into()));
            }
            self.outgoing.lock().unwrap().push_back(message);
            Ok(())
        }

        /// Receives the next message (from incoming queue).
        pub fn receive(&mut self) -> crate::error::SyncResult<Option<SyncMessage>> {
            if !self.connected {
                return Ok(None);
            }
            Ok(self.incoming.lock().unwrap().pop_front())
        }

        /// Closes the connection.
        pub fn close(&mut self) {
            self.connected = false;
        }

        /// Whether the connection is still alive.
        pub fn is_connected(&self) -> bool {
            self.connected
        }
    }
}
