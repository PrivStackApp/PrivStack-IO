//! P2P transport implementation using libp2p swarm.

use crate::error::{SyncError, SyncResult};
use crate::p2p::behaviour::{SyncBehaviour, SyncBehaviourEvent};
use crate::p2p::codec::{SyncRequest, SyncResponse};
use crate::protocol::SyncMessage;
use crate::transport::{
    DiscoveredPeer, DiscoveryMethod, IncomingSyncRequest, ResponseToken, SyncTransport,
};
use async_trait::async_trait;
use futures::StreamExt;
use libp2p::{
    identity::Keypair,
    kad,
    mdns,
    request_response::{self, OutboundRequestId, ResponseChannel},
    swarm::SwarmEvent,
    Multiaddr, PeerId as Libp2pPeerId, Swarm,
};
use privstack_types::PeerId;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tracing::{debug, info, warn};

/// PrivStack's official bootstrap relay node.
pub const PRIVSTACK_BOOTSTRAP_NODE: &str =
    "/ip4/76.13.98.108/udp/4001/quic-v1/p2p/12D3KooWN8q37myvsWydgRnVxHbxTay1H9MUDo9PwSSYaj5NcrzS";

/// Configuration for the P2P transport.
#[derive(Debug, Clone)]
pub struct P2pConfig {
    /// Listen addresses.
    pub listen_addrs: Vec<Multiaddr>,
    /// Bootstrap nodes for DHT discovery (relay/bootstrap servers).
    pub bootstrap_nodes: Vec<Multiaddr>,
    /// Sync code hash for namespaced discovery (only see peers with same code).
    /// If None, DHT discovery is disabled for privacy.
    pub sync_code_hash: Option<Vec<u8>>,
    /// Our device name for pairing.
    pub device_name: String,
    /// Enable mDNS discovery (local network only, always private).
    pub enable_mdns: bool,
    /// Enable Kademlia DHT (requires sync_code_hash for privacy).
    pub enable_dht: bool,
    /// Connection idle timeout.
    pub idle_timeout: Duration,
}

impl Default for P2pConfig {
    fn default() -> Self {
        Self {
            listen_addrs: vec![
                "/ip4/0.0.0.0/udp/0/quic-v1".parse().unwrap(),
                "/ip6/::/udp/0/quic-v1".parse().unwrap(),
            ],
            bootstrap_nodes: vec![
                PRIVSTACK_BOOTSTRAP_NODE.parse().unwrap(),
            ],
            sync_code_hash: None, // Must be set for DHT discovery
            device_name: "PrivStack Device".to_string(),
            enable_mdns: true,
            enable_dht: true,
            idle_timeout: Duration::from_secs(60),
        }
    }
}

/// Internal peer info tracking.
struct PeerInfo {
    libp2p_id: Libp2pPeerId,
    privstack_id: PeerId,
    device_name: Option<String>,
    discovery_method: DiscoveryMethod,
    addresses: Vec<Multiaddr>,
}

/// Incoming sync request with response channel.
pub struct IncomingRequest {
    /// The peer that sent the request.
    pub peer_id: PeerId,
    /// The libp2p peer ID.
    pub libp2p_peer_id: Libp2pPeerId,
    /// The request message.
    pub message: SyncMessage,
    /// Channel to send the response.
    pub response_channel: ResponseChannel<SyncResponse>,
}

/// Command sent to the swarm event loop.
enum SwarmCommand {
    /// Send a request to a peer.
    SendRequest {
        peer_id: Libp2pPeerId,
        message: SyncMessage,
        response_tx: oneshot::Sender<SyncResult<SyncMessage>>,
    },
    /// Send a response to an incoming request.
    SendResponse {
        channel: ResponseChannel<SyncResponse>,
        message: SyncMessage,
    },
    /// Publish our presence to the sync group DHT.
    PublishToSyncGroup {
        sync_code_hash: Vec<u8>,
        device_name: String,
    },
    /// Discover peers in our sync group.
    DiscoverSyncGroup {
        sync_code_hash: Vec<u8>,
    },
}

/// P2P transport using libp2p.
pub struct P2pTransport {
    /// Our libp2p peer ID.
    libp2p_peer_id: Libp2pPeerId,
    /// Our PrivStack peer ID.
    local_peer_id: PeerId,
    /// The libp2p keypair.
    keypair: Keypair,
    /// Configuration.
    config: P2pConfig,
    /// Discovered peers.
    discovered_peers: Arc<RwLock<HashMap<Libp2pPeerId, PeerInfo>>>,
    /// Channel to send commands to the swarm.
    command_tx: Option<mpsc::Sender<SwarmCommand>>,
    /// Channel to receive incoming requests.
    incoming_rx: Arc<Mutex<mpsc::Receiver<IncomingRequest>>>,
    /// Sender for incoming requests (kept for cloning to event loop).
    incoming_tx: mpsc::Sender<IncomingRequest>,
    /// Whether the transport is running.
    running: Arc<AtomicBool>,
}

impl P2pTransport {
    /// Creates a new P2P transport with a random keypair.
    pub fn new(local_peer_id: PeerId, config: P2pConfig) -> SyncResult<Self> {
        let keypair = Keypair::generate_ed25519();
        Self::with_keypair(local_peer_id, keypair, config)
    }

    /// Creates a new P2P transport with a specific keypair.
    pub fn with_keypair(
        local_peer_id: PeerId,
        keypair: Keypair,
        config: P2pConfig,
    ) -> SyncResult<Self> {
        let libp2p_peer_id = Libp2pPeerId::from(keypair.public());
        let (incoming_tx, incoming_rx) = mpsc::channel(32);

        Ok(Self {
            libp2p_peer_id,
            local_peer_id,
            keypair,
            config,
            discovered_peers: Arc::new(RwLock::new(HashMap::new())),
            command_tx: None,
            incoming_rx: Arc::new(Mutex::new(incoming_rx)),
            incoming_tx,
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Returns our libp2p peer ID.
    pub fn libp2p_peer_id(&self) -> Libp2pPeerId {
        self.libp2p_peer_id
    }

    /// Creates the libp2p swarm.
    fn create_swarm(&self) -> SyncResult<Swarm<SyncBehaviour>> {
        let behaviour = SyncBehaviour::new(
            self.libp2p_peer_id,
            &self.keypair,
            &self.config.bootstrap_nodes,
            self.config.enable_mdns,
            &self.config.device_name,
        );

        let swarm = libp2p::SwarmBuilder::with_existing_identity(self.keypair.clone())
            .with_tokio()
            .with_quic()
            .with_behaviour(|_| behaviour)
            .map_err(|e| SyncError::Network(format!("failed to create behaviour: {e}")))?
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(self.config.idle_timeout))
            .build();

        Ok(swarm)
    }

    /// Maps a libp2p PeerId to our internal PeerId.
    pub fn map_peer_id(libp2p_id: &Libp2pPeerId) -> PeerId {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        libp2p_id.to_bytes().hash(&mut hasher);
        let hash = hasher.finish();

        let bytes = hash.to_be_bytes();
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes[..8].copy_from_slice(&bytes);
        uuid_bytes[8..].copy_from_slice(&bytes);
        uuid_bytes[6] = (uuid_bytes[6] & 0x0f) | 0x40;
        uuid_bytes[8] = (uuid_bytes[8] & 0x3f) | 0x80;

        PeerId::from_uuid(uuid::Uuid::from_bytes(uuid_bytes))
    }

    /// Sends a sync message to a peer and waits for a response (internal).
    async fn send_request_inner(
        &self,
        peer_id: &PeerId,
        message: SyncMessage,
    ) -> SyncResult<SyncMessage> {
        let command_tx = self
            .command_tx
            .as_ref()
            .ok_or_else(|| SyncError::Network("transport not running".to_string()))?;

        // Find the libp2p peer ID
        let discovered = self.discovered_peers.read().await;
        let peer_info = discovered
            .values()
            .find(|p| p.privstack_id == *peer_id)
            .ok_or_else(|| SyncError::Network(format!("unknown peer: {peer_id}")))?;

        let libp2p_id = peer_info.libp2p_id;
        drop(discovered);

        let (response_tx, response_rx) = oneshot::channel();

        command_tx
            .send(SwarmCommand::SendRequest {
                peer_id: libp2p_id,
                message,
                response_tx,
            })
            .await
            .map_err(|_| SyncError::Network("command channel closed".to_string()))?;

        response_rx
            .await
            .map_err(|_| SyncError::Network("response channel closed".to_string()))?
    }

    /// Sends a response to an incoming request (internal, takes libp2p channel).
    async fn send_response_inner(
        &self,
        channel: ResponseChannel<SyncResponse>,
        message: SyncMessage,
    ) -> SyncResult<()> {
        let command_tx = self
            .command_tx
            .as_ref()
            .ok_or_else(|| SyncError::Network("transport not running".to_string()))?;

        command_tx
            .send(SwarmCommand::SendResponse { channel, message })
            .await
            .map_err(|_| SyncError::Network("command channel closed".to_string()))?;

        Ok(())
    }

    /// Publishes our presence to the sync group DHT.
    /// Call this after setting a sync code to make ourselves discoverable.
    pub async fn publish_to_sync_group(&self, sync_code_hash: &[u8]) -> SyncResult<()> {
        let command_tx = self
            .command_tx
            .as_ref()
            .ok_or_else(|| SyncError::Network("transport not running".to_string()))?;

        command_tx
            .send(SwarmCommand::PublishToSyncGroup {
                sync_code_hash: sync_code_hash.to_vec(),
                device_name: self.config.device_name.clone(),
            })
            .await
            .map_err(|_| SyncError::Network("command channel closed".to_string()))?;

        Ok(())
    }

    /// Discovers peers in the sync group via DHT.
    pub async fn discover_sync_group(&self, sync_code_hash: &[u8]) -> SyncResult<()> {
        let command_tx = self
            .command_tx
            .as_ref()
            .ok_or_else(|| SyncError::Network("transport not running".to_string()))?;

        command_tx
            .send(SwarmCommand::DiscoverSyncGroup {
                sync_code_hash: sync_code_hash.to_vec(),
            })
            .await
            .map_err(|_| SyncError::Network("command channel closed".to_string()))?;

        Ok(())
    }

    /// Runs the swarm event loop.
    async fn run_event_loop(
        mut swarm: Swarm<SyncBehaviour>,
        mut command_rx: mpsc::Receiver<SwarmCommand>,
        discovered_peers: Arc<RwLock<HashMap<Libp2pPeerId, PeerInfo>>>,
        incoming_tx: mpsc::Sender<IncomingRequest>,
        running: Arc<AtomicBool>,
        sync_code_hash: Option<Vec<u8>>,
        device_name: String,
        local_libp2p_peer_id: Libp2pPeerId,
    ) {
        // Pending outbound requests waiting for responses
        let mut pending_requests: HashMap<
            OutboundRequestId,
            oneshot::Sender<SyncResult<SyncMessage>>,
        > = HashMap::new();

        // Track our listen addresses for DHT publishing
        let mut listen_addresses: Vec<Multiaddr> = Vec::new();

        // Timer for periodic sync group publish/discover
        let mut sync_group_interval = tokio::time::interval(Duration::from_secs(30));
        let mut initial_publish_done = false;

        loop {
            if !running.load(Ordering::SeqCst) {
                break;
            }

            tokio::select! {
                // Handle swarm events
                event = swarm.select_next_some() => {
                    match event {
                        SwarmEvent::Behaviour(SyncBehaviourEvent::Mdns(mdns::Event::Discovered(peers))) => {
                            for (peer_id, addr) in peers {
                                // Skip ourselves
                                if peer_id == local_libp2p_peer_id {
                                    continue;
                                }
                                info!("mDNS discovered peer: {peer_id} at {addr}");
                                // Register address with the swarm so it can dial this peer
                                swarm.add_peer_address(peer_id, addr.clone());
                                let mut discovered = discovered_peers.write().await;
                                let privstack_id = Self::map_peer_id(&peer_id);
                                discovered.entry(peer_id).or_insert_with(|| PeerInfo {
                                    libp2p_id: peer_id,
                                    privstack_id,
                                    device_name: None,
                                    discovery_method: DiscoveryMethod::Mdns,
                                    addresses: Vec::new(),
                                }).addresses.push(addr);
                            }
                        }
                        SwarmEvent::Behaviour(SyncBehaviourEvent::Mdns(mdns::Event::Expired(peers))) => {
                            for (peer_id, _addr) in peers {
                                debug!("mDNS peer expired: {peer_id}");
                            }
                        }
                        SwarmEvent::Behaviour(SyncBehaviourEvent::Kademlia(kad_event)) => {
                            match kad_event {
                                kad::Event::OutboundQueryProgressed { result, .. } => {
                                    match result {
                                        kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(peer_record))) => {
                                            // Found a peer in our sync group!
                                            if let Ok(value_str) = std::str::from_utf8(&peer_record.record.value) {
                                                if let Ok(value) = serde_json::from_str::<serde_json::Value>(value_str) {
                                                    let peer_device_name = value.get("device_name")
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or("Unknown Device")
                                                        .to_string();
                                                    let addresses: Vec<Multiaddr> = value.get("addresses")
                                                        .and_then(|v| v.as_array())
                                                        .map(|arr| {
                                                            arr.iter()
                                                                .filter_map(|a| a.as_str()?.parse().ok())
                                                                .collect()
                                                        })
                                                        .unwrap_or_default();

                                                    if let Some(publisher) = peer_record.record.publisher {
                                                        // Skip ourselves
                                                        if publisher == local_libp2p_peer_id {
                                                            continue;
                                                        }
                                                        info!("DHT discovered sync group peer: {publisher} ({peer_device_name})");
                                                        // Register addresses with the swarm for dialing
                                                        for addr in &addresses {
                                                            swarm.add_peer_address(publisher, addr.clone());
                                                        }
                                                        let mut discovered = discovered_peers.write().await;
                                                        let privstack_id = Self::map_peer_id(&publisher);
                                                        let entry = discovered.entry(publisher).or_insert_with(|| PeerInfo {
                                                            libp2p_id: publisher,
                                                            privstack_id,
                                                            device_name: Some(peer_device_name.clone()),
                                                            discovery_method: DiscoveryMethod::Dht,
                                                            addresses: Vec::new(),
                                                        });
                                                        entry.device_name = Some(peer_device_name);
                                                        entry.addresses.extend(addresses);
                                                    }
                                                }
                                            }
                                        }
                                        kad::QueryResult::GetRecord(Err(e)) => {
                                            debug!("DHT get record failed: {:?}", e);
                                        }
                                        kad::QueryResult::PutRecord(Ok(_)) => {
                                            debug!("Successfully published to sync group DHT");
                                        }
                                        kad::QueryResult::PutRecord(Err(e)) => {
                                            warn!("Failed to publish to sync group DHT: {:?}", e);
                                        }
                                        kad::QueryResult::Bootstrap(Ok(_)) => {
                                            info!("Kademlia bootstrap completed successfully");
                                        }
                                        kad::QueryResult::Bootstrap(Err(e)) => {
                                            warn!("Kademlia bootstrap failed: {:?}", e);
                                        }
                                        _ => {}
                                    }
                                }
                                kad::Event::RoutingUpdated { peer, .. } => {
                                    debug!("Kademlia routing updated for peer: {peer}");
                                }
                                _ => {}
                            }
                        }
                        SwarmEvent::Behaviour(SyncBehaviourEvent::Identify(identify_event)) => {
                            if let libp2p::identify::Event::Received { peer_id, info, .. } = identify_event {
                                // Extract device name from agent_version (format: "PrivStack/{name}")
                                let peer_device_name = info.agent_version
                                    .strip_prefix("PrivStack/")
                                    .map(|n| n.to_string());

                                if let Some(ref name) = peer_device_name {
                                    info!("Identified peer {peer_id}: {name}");
                                } else {
                                    debug!("Identified peer {peer_id}: {:?}", info.agent_version);
                                }

                                // Register addresses with the swarm for dialing
                                for addr in &info.listen_addrs {
                                    swarm.add_peer_address(peer_id, addr.clone());
                                }
                                let mut discovered = discovered_peers.write().await;
                                if let Some(peer_info) = discovered.get_mut(&peer_id) {
                                    peer_info.addresses.extend(info.listen_addrs);
                                    if peer_device_name.is_some() {
                                        peer_info.device_name = peer_device_name;
                                    }
                                }
                            }
                        }
                        SwarmEvent::Behaviour(SyncBehaviourEvent::SyncProtocol(req_res_event)) => {
                            match req_res_event {
                                request_response::Event::Message { peer, message, .. } => {
                                    match message {
                                        request_response::Message::Request { request, channel, .. } => {
                                            let privstack_id = Self::map_peer_id(&peer);
                                            let incoming = IncomingRequest {
                                                peer_id: privstack_id,
                                                libp2p_peer_id: peer,
                                                message: request.0,
                                                response_channel: channel,
                                            };
                                            if incoming_tx.send(incoming).await.is_err() {
                                                warn!("Failed to send incoming request to channel");
                                            }
                                        }
                                        request_response::Message::Response { request_id, response } => {
                                            if let Some(response_tx) = pending_requests.remove(&request_id) {
                                                let _ = response_tx.send(Ok(response.0));
                                            }
                                        }
                                    }
                                }
                                request_response::Event::OutboundFailure { request_id, error, .. } => {
                                    if let Some(response_tx) = pending_requests.remove(&request_id) {
                                        let _ = response_tx.send(Err(SyncError::Network(
                                            format!("outbound request failed: {error:?}")
                                        )));
                                    }
                                }
                                request_response::Event::InboundFailure { error, .. } => {
                                    warn!("Inbound request failed: {error:?}");
                                }
                                request_response::Event::ResponseSent { .. } => {
                                    debug!("Response sent successfully");
                                }
                            }
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            info!("Connection established with {peer_id}");
                        }
                        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                            info!("Connection closed with {peer_id}: {cause:?}");
                        }
                        SwarmEvent::IncomingConnection { local_addr, send_back_addr, .. } => {
                            debug!("Incoming connection from {send_back_addr} on {local_addr}");
                        }
                        SwarmEvent::NewListenAddr { address, .. } => {
                            info!("Listening on {address}");
                            listen_addresses.push(address);
                        }
                        _ => {}
                    }
                }

                // Handle commands
                Some(command) = command_rx.recv() => {
                    match command {
                        SwarmCommand::SendRequest { peer_id, message, response_tx } => {
                            let request_id = swarm
                                .behaviour_mut()
                                .sync_protocol
                                .send_request(&peer_id, SyncRequest(message));
                            pending_requests.insert(request_id, response_tx);
                        }
                        SwarmCommand::SendResponse { channel, message } => {
                            if swarm
                                .behaviour_mut()
                                .sync_protocol
                                .send_response(channel, SyncResponse(message))
                                .is_err()
                            {
                                warn!("Failed to send response (channel closed)");
                            }
                        }
                        SwarmCommand::PublishToSyncGroup { sync_code_hash, device_name } => {
                            swarm.behaviour_mut().publish_to_sync_group(
                                &sync_code_hash,
                                &device_name,
                                &listen_addresses,
                            );
                            info!("Published presence to sync group DHT");
                        }
                        SwarmCommand::DiscoverSyncGroup { sync_code_hash } => {
                            swarm.behaviour_mut().discover_sync_group(&sync_code_hash);
                            debug!("Initiated sync group discovery");
                        }
                    }
                }

                // Periodic sync group publish/discover
                _ = sync_group_interval.tick() => {
                    if let Some(ref hash) = sync_code_hash {
                        // Publish our presence
                        swarm.behaviour_mut().publish_to_sync_group(
                            hash,
                            &device_name,
                            &listen_addresses,
                        );

                        // Discover peers in our sync group
                        swarm.behaviour_mut().discover_sync_group(hash);

                        if !initial_publish_done {
                            info!("Initial sync group publish/discover completed");
                            initial_publish_done = true;
                        }
                    }
                }
            }
        }
    }
}

#[async_trait]
impl SyncTransport for P2pTransport {
    async fn start(&mut self) -> SyncResult<()> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        let mut swarm = self.create_swarm()?;

        // Listen on configured addresses
        for addr in &self.config.listen_addrs {
            swarm
                .listen_on(addr.clone())
                .map_err(|e| SyncError::Network(format!("failed to listen on {addr}: {e}")))?;
        }

        self.running.store(true, Ordering::SeqCst);

        // Create command channel
        let (command_tx, command_rx) = mpsc::channel(32);
        self.command_tx = Some(command_tx);

        // Spawn the event loop
        let peers_clone = Arc::clone(&self.discovered_peers);
        let incoming_tx = self.incoming_tx.clone();
        let running_clone = Arc::clone(&self.running);
        let sync_code_hash = self.config.sync_code_hash.clone();
        let device_name = self.config.device_name.clone();
        let local_libp2p_peer_id = self.libp2p_peer_id;

        tokio::spawn(async move {
            Self::run_event_loop(
                swarm,
                command_rx,
                peers_clone,
                incoming_tx,
                running_clone,
                sync_code_hash,
                device_name,
                local_libp2p_peer_id,
            ).await;
        });

        info!(
            "P2P transport started, libp2p peer ID: {}",
            self.libp2p_peer_id
        );
        Ok(())
    }

    async fn stop(&mut self) -> SyncResult<()> {
        self.running.store(false, Ordering::SeqCst);
        self.command_tx = None;
        info!("P2P transport stopped");
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    fn discovered_peers(&self) -> Vec<DiscoveredPeer> {
        // Uses try_read to avoid blocking the async runtime.
        // Prefer discovered_peers_async() in async contexts.
        match self.discovered_peers.try_read() {
            Ok(discovered) => discovered
                .values()
                .map(|info| DiscoveredPeer {
                    peer_id: info.privstack_id,
                    device_name: info.device_name.clone(),
                    discovery_method: info.discovery_method,
                    addresses: info.addresses.iter().map(|a| a.to_string()).collect(),
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    async fn discovered_peers_async(&self) -> Vec<DiscoveredPeer> {
        let discovered = self.discovered_peers.read().await;
        discovered
            .values()
            .map(|info| DiscoveredPeer {
                peer_id: info.privstack_id,
                device_name: info.device_name.clone(),
                discovery_method: info.discovery_method,
                addresses: info.addresses.iter().map(|a| a.to_string()).collect(),
            })
            .collect()
    }

    async fn send_request(
        &self,
        peer_id: &PeerId,
        message: SyncMessage,
    ) -> SyncResult<SyncMessage> {
        self.send_request_inner(peer_id, message).await
    }

    async fn recv_request(&self) -> Option<IncomingSyncRequest> {
        let mut rx = self.incoming_rx.lock().await;
        let req = rx.recv().await?;
        Some(IncomingSyncRequest {
            peer_id: req.peer_id,
            message: req.message,
            response_token: ResponseToken::new(req.response_channel),
        })
    }

    async fn send_response(
        &self,
        token: ResponseToken,
        message: SyncMessage,
    ) -> SyncResult<()> {
        let channel: ResponseChannel<SyncResponse> = token
            .downcast()
            .ok_or_else(|| SyncError::Network("invalid response token".to_string()))?;
        self.send_response_inner(channel, message).await
    }
}
