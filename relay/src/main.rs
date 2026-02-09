//! PrivStack P2P Relay and Bootstrap Node
//!
//! This binary runs on a public server to help PrivStack clients:
//! 1. Discover each other via Kademlia DHT
//! 2. Relay traffic when direct P2P connection fails (NAT traversal)
//!
//! Usage:
//!   privstack-relay --port 4001
//!
//! The relay is stateless and doesn't store any user data.

use std::{fs, path::PathBuf, sync::Arc, time::Duration};
use anyhow::{Context, Result};
use clap::Parser;
use futures::StreamExt;
use libp2p::{
    identify, kad, relay,
    swarm::{NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId,
};
use privstack_relay::{build_router, IdentityResponse};
use tracing::{info, warn, debug, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(name = "privstack-relay")]
#[command(about = "PrivStack P2P relay and bootstrap node")]
struct Args {
    /// Port to listen on (UDP/QUIC)
    #[arg(short, long, default_value = "4001")]
    port: u16,

    /// Path to identity key file
    #[arg(short, long, default_value = "relay-identity.key")]
    identity: PathBuf,

    /// HTTP API port for identity endpoint
    #[arg(long, default_value = "4002")]
    http_port: u16,

    /// Enable verbose debug logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(NetworkBehaviour)]
struct RelayBehaviour {
    relay: relay::Behaviour,
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: identify::Behaviour,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let log_level = if args.verbose { Level::DEBUG } else { Level::INFO };
    FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .compact()
        .init();

    info!("PrivStack Relay starting...");
    let keypair = load_or_generate_keypair(&args.identity)?;
    let local_peer_id = PeerId::from(keypair.public());
    info!("Relay PeerId: {}", local_peer_id);

    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_quic()
        .with_behaviour(|key| {
            // Relay behaviour for NAT traversal
            let relay = relay::Behaviour::new(
                key.public().to_peer_id(),
                relay::Config::default()
            );

            // Kademlia DHT for peer discovery
            let mut kad_config = kad::Config::new(kad::PROTOCOL_NAME);
            kad_config.set_query_timeout(Duration::from_secs(60));
            let mut kademlia = kad::Behaviour::with_config(
                key.public().to_peer_id(),
                kad::store::MemoryStore::new(key.public().to_peer_id()),
                kad_config,
            );
            // Force server mode so we respond to incoming Kademlia requests
            kademlia.set_mode(Some(kad::Mode::Server));

            // Identify protocol for peer info exchange
            let identify = identify::Behaviour::new(
                identify::Config::new("/privstack/relay/1.0.0".into(), key.public())
                    .with_agent_version("privstack-relay/0.1.0".into()),
            );

            RelayBehaviour { relay, kademlia, identify }
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(300)))
        .build();

    let listen_v4: Multiaddr = format!("/ip4/0.0.0.0/udp/{}/quic-v1", args.port).parse()?;
    let listen_v6: Multiaddr = format!("/ip6/::/udp/{}/quic-v1", args.port).parse()?;
    swarm.listen_on(listen_v4)?;
    swarm.listen_on(listen_v6)?;

    // Spawn HTTP identity endpoint
    let identity_state = Arc::new(IdentityResponse {
        peer_id: local_peer_id.to_string(),
        addresses: vec![
            format!("/ip4/0.0.0.0/udp/{}/quic-v1", args.port),
            format!("/ip6/::/udp/{}/quic-v1", args.port),
        ],
        protocol_version: "/privstack/relay/1.0.0".to_string(),
        agent_version: "privstack-relay/0.1.0".to_string(),
    });

    let http_port = args.http_port;
    tokio::spawn(async move {
        let app = build_router(identity_state);

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", http_port))
            .await
            .expect("Failed to bind HTTP port");
        info!("HTTP identity endpoint listening on port {}", http_port);
        axum::serve(listener, app).await.expect("HTTP server failed");
    });

    println!("\n========================================");
    println!("  PrivStack Relay Running");
    println!("========================================");
    println!("  PeerId:    {}", local_peer_id);
    println!("  P2P Port:  {}", args.port);
    println!("  HTTP Port: {}", http_port);
    println!("\n  Bootstrap address:");
    println!("  /ip4/YOUR_PUBLIC_IP/udp/{}/quic-v1/p2p/{}", args.port, local_peer_id);
    println!("========================================\n");

    let mut peers_served: u64 = 0;
    let mut relayed_connections: u64 = 0;

    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {}/p2p/{}", address, local_peer_id);
            }
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                peers_served += 1;
                info!("Peer connected: {} (total served: {})", peer_id, peers_served);
                swarm.behaviour_mut().kademlia.add_address(
                    &peer_id,
                    endpoint.get_remote_address().clone()
                );
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                debug!("Peer disconnected: {}", peer_id);
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::Relay(event)) => {
                match event {
                    relay::Event::ReservationReqAccepted { src_peer_id, .. } => {
                        info!("Relay reservation accepted for {}", src_peer_id);
                    }
                    relay::Event::CircuitReqAccepted { src_peer_id, dst_peer_id, .. } => {
                        relayed_connections += 1;
                        info!(
                            "Relaying circuit: {} <-> {} (total relayed: {})",
                            src_peer_id, dst_peer_id, relayed_connections
                        );
                    }
                    relay::Event::CircuitClosed { src_peer_id, dst_peer_id, .. } => {
                        debug!("Circuit closed: {} <-> {}", src_peer_id, dst_peer_id);
                    }
                    _ => debug!("Relay event: {:?}", event),
                }
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::Kademlia(event)) => {
                match &event {
                    kad::Event::RoutingUpdated { peer, .. } => {
                        debug!("Kademlia routing updated for peer: {}", peer);
                    }
                    kad::Event::OutboundQueryProgressed { result, .. } => {
                        match result {
                            kad::QueryResult::Bootstrap(Ok(_)) => {
                                info!("Kademlia bootstrap completed");
                            }
                            kad::QueryResult::Bootstrap(Err(e)) => {
                                warn!("Kademlia bootstrap failed: {:?}", e);
                            }
                            _ => debug!("Kademlia query result: {:?}", result),
                        }
                    }
                    _ => debug!("Kademlia event: {:?}", event),
                }
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::Identify(event)) => {
                if let identify::Event::Received { peer_id, info, .. } = event {
                    debug!(
                        "Identified peer {}: {} ({})",
                        peer_id, info.agent_version, info.protocol_version
                    );
                    for addr in info.listen_addrs {
                        swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                    }
                }
            }
            SwarmEvent::IncomingConnectionError { error, .. } => {
                warn!("Incoming connection error: {}", error);
            }
            _ => {}
        }
    }
}

fn load_or_generate_keypair(path: &PathBuf) -> Result<libp2p::identity::Keypair> {
    if path.exists() {
        info!("Loading identity from {:?}", path);
        let bytes = fs::read(path).context("Failed to read identity file")?;
        libp2p::identity::Keypair::from_protobuf_encoding(&bytes)
            .context("Failed to decode identity key")
    } else {
        info!("Generating new identity at {:?}", path);
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        fs::write(path, keypair.to_protobuf_encoding()?)
            .context("Failed to write identity file")?;
        Ok(keypair)
    }
}
