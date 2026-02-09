//! Network behaviour combining mDNS discovery and sync protocol.

use crate::p2p::codec::SyncCodec;
use libp2p::{
    identify, kad, mdns,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, behaviour::toggle::Toggle},
    Multiaddr,
};
use std::iter;
use std::time::Duration;
use tracing::{info, warn, debug};

/// The sync protocol identifier.
pub const SYNC_PROTOCOL: &str = "/privstack/sync/1.0.0";

/// DHT record key prefix for sync code discovery.
const SYNC_CODE_DHT_PREFIX: &[u8] = b"/privstack/syncgroup/";

/// Combined network behaviour for PrivStack sync.
#[derive(NetworkBehaviour)]
pub struct SyncBehaviour {
    /// mDNS for local network peer discovery (optional — disabled for DHT-only mode).
    pub mdns: Toggle<mdns::tokio::Behaviour>,
    /// Kademlia DHT for WAN peer discovery.
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    /// Identify protocol for peer info exchange.
    pub identify: identify::Behaviour,
    /// Request-response for sync messages.
    pub sync_protocol: request_response::Behaviour<SyncCodec>,
}

impl SyncBehaviour {
    /// Creates a new sync behaviour with explicit mDNS toggle.
    pub fn new(
        local_peer_id: libp2p::PeerId,
        keypair: &libp2p::identity::Keypair,
        bootstrap_nodes: &[Multiaddr],
        enable_mdns: bool,
        device_name: &str,
    ) -> Self {
        // mDNS for local discovery (conditionally enabled)
        let mdns = if enable_mdns {
            let behaviour = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)
                .expect("mDNS behaviour creation failed");
            Toggle::from(Some(behaviour))
        } else {
            debug!("mDNS disabled");
            Toggle::from(None)
        };

        // Kademlia for DHT-based discovery
        let store = kad::store::MemoryStore::new(local_peer_id);
        let mut kademlia_config = kad::Config::new(kad::PROTOCOL_NAME);
        kademlia_config.set_query_timeout(Duration::from_secs(60));
        let mut kademlia = kad::Behaviour::with_config(local_peer_id, store, kademlia_config);

        // Add bootstrap nodes to Kademlia
        for addr in bootstrap_nodes {
            if let Some(peer_id) = extract_peer_id(addr) {
                let addr_without_peer = remove_peer_id_suffix(addr);
                kademlia.add_address(&peer_id, addr_without_peer);
                info!("Added bootstrap node: {}", peer_id);
            } else {
                warn!("Bootstrap address missing peer ID: {}", addr);
            }
        }

        // Force server mode so we respond to incoming Kademlia requests
        kademlia.set_mode(Some(kad::Mode::Server));

        // Trigger bootstrap if we have nodes
        if !bootstrap_nodes.is_empty() {
            if let Err(e) = kademlia.bootstrap() {
                warn!("Failed to trigger Kademlia bootstrap: {:?}", e);
            }
        }

        // Identify for exchanging peer info (device name carried in agent_version)
        let identify = identify::Behaviour::new(
            identify::Config::new(
                SYNC_PROTOCOL.to_string(),
                keypair.public(),
            )
            .with_agent_version(format!("PrivStack/{}", device_name)),
        );

        // Request-response for sync protocol
        let sync_protocol = request_response::Behaviour::new(
            iter::once((SYNC_PROTOCOL, ProtocolSupport::Full)),
            request_response::Config::default().with_request_timeout(Duration::from_secs(120)),
        );

        Self {
            mdns,
            kademlia,
            identify,
            sync_protocol,
        }
    }

    /// Publishes our presence to the DHT under the sync code namespace.
    /// Only peers with the same sync code will be able to discover us.
    pub fn publish_to_sync_group(
        &mut self,
        sync_code_hash: &[u8],
        device_name: &str,
        addresses: &[Multiaddr],
    ) -> kad::QueryId {
        let key = Self::sync_group_key(sync_code_hash);

        // Value contains device name and addresses as JSON
        let value = serde_json::json!({
            "device_name": device_name,
            "addresses": addresses.iter().map(|a| a.to_string()).collect::<Vec<_>>(),
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        });

        let record = kad::Record {
            key: key.clone(),
            value: value.to_string().into_bytes(),
            publisher: None,
            expires: None,
        };

        debug!("Publishing to sync group DHT key: {:?}", key);
        self.kademlia.put_record(record, kad::Quorum::One).unwrap()
    }

    /// Queries the DHT for peers in the same sync group.
    pub fn discover_sync_group(&mut self, sync_code_hash: &[u8]) -> kad::QueryId {
        let key = Self::sync_group_key(sync_code_hash);
        debug!("Querying sync group DHT key: {:?}", key);
        self.kademlia.get_record(key)
    }

    /// Creates the DHT key for a sync group.
    fn sync_group_key(sync_code_hash: &[u8]) -> kad::RecordKey {
        let mut key = SYNC_CODE_DHT_PREFIX.to_vec();
        key.extend_from_slice(sync_code_hash);
        kad::RecordKey::new(&key)
    }
}

/// Extract the PeerId from a multiaddr like /ip4/.../p2p/12D3KooW...
fn extract_peer_id(addr: &Multiaddr) -> Option<libp2p::PeerId> {
    addr.iter().find_map(|proto| {
        if let libp2p::multiaddr::Protocol::P2p(peer_id) = proto {
            Some(peer_id)
        } else {
            None
        }
    })
}

/// Remove the /p2p/... suffix from a multiaddr.
fn remove_peer_id_suffix(addr: &Multiaddr) -> Multiaddr {
    addr.iter()
        .filter(|proto| !matches!(proto, libp2p::multiaddr::Protocol::P2p(_)))
        .collect()
}
