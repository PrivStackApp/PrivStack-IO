//! libp2p-based P2P transport for sync.
//!
//! Provides peer discovery via mDNS (local network) and Kademlia DHT (WAN),
//! with encrypted connections using Noise protocol.

mod behaviour;
pub mod codec;
pub mod connection;
pub mod transport;

pub use codec::{SyncCodec, SyncRequest, SyncResponse};
pub use connection::P2pConnection;
pub use libp2p::identity::Keypair;
pub use transport::{IncomingRequest, P2pConfig, P2pTransport, PRIVSTACK_BOOTSTRAP_NODE};
