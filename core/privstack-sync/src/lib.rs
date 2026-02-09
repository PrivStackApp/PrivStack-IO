//! P2P and cloud sync engine for PrivStack.
//!
//! Provides multiple sync transports:
//! - libp2p for peer-to-peer sync (LAN via mDNS, WAN via DHT)
//! - Google Drive for cloud-based sync
//! - iCloud Drive for Apple ecosystem sync
//!
//! # Architecture
//!
//! The sync system is built around CRDTs (Conflict-free Replicated Data Types),
//! which allow documents to be edited concurrently on multiple devices and
//! merged without conflicts.
//!
//! ## Components
//!
//! - **Protocol**: Defines the messages exchanged between peers
//! - **State**: Tracks sync progress using vector clocks
//! - **Transport**: Abstracts over different network transports
//! - **Engine**: Orchestrates the sync process
//!
//! ## Sync Process
//!
//! 1. **Discovery**: Find other peers (mDNS for LAN, DHT for WAN)
//! 2. **Handshake**: Exchange peer info and protocol version
//! 3. **State Exchange**: Share vector clocks to determine what's missing
//! 4. **Event Sync**: Send missing events in batches
//! 5. **Apply**: Apply received events using CRDT merge
//!
//! # Example
//!
//! ```
//! use privstack_sync::{SyncEngine, SyncConfig};
//! use privstack_types::PeerId;
//!
//! let peer_id = PeerId::new();
//! let config = SyncConfig {
//!     device_name: "My Laptop".to_string(),
//!     ..Default::default()
//! };
//!
//! let engine = SyncEngine::new(peer_id, config);
//! ```

pub mod acl_applicator;
pub mod applicator;
pub mod cloud;
mod engine;
mod error;
mod orchestrator;
pub mod p2p;
pub mod pairing;
pub mod policy;
pub mod policy_store;
pub mod protocol;
pub mod state;
pub mod transport;

pub use acl_applicator::{AclApplicator, AclEventHandler};
pub use applicator::{create_event, ApplicatorError, ApplicatorResult, EventApplicator};
pub use orchestrator::{
    create_enterprise_orchestrator, create_orchestrator, create_orchestrator_with_pairing,
    create_orchestrator_with_policy, create_personal_orchestrator, OrchestratorConfig,
    OrchestratorHandle, SyncCommand, SyncEvent, SyncOrchestrator,
};

pub use engine::{SyncConfig, SyncEngine};
pub use error::{SyncError, SyncResult};
pub use policy::{
    AllowAllPolicy, AuditAction, AuditDecision, AuditEntry, DeviceId, EntityAcl,
    EnterpriseSyncPolicy, PersonalSyncPolicy, SyncPolicy, SyncRole, TeamId,
};
pub use policy_store::PolicyStore;
pub use protocol::{
    ErrorMessage, EventAckMessage, EventBatchMessage, EventNotifyMessage, HelloAckMessage,
    HelloMessage, SubscribeMessage, SyncMessage, SyncRequestMessage, SyncStateMessage,
    MAX_BATCH_SIZE, PROTOCOL_VERSION,
};
pub use state::{EntitySyncState, PeerSyncStatus, SyncState};
pub use transport::{
    DiscoveredPeer, DiscoveryMethod, IncomingSyncRequest, ResponseToken, SyncTransport,
};

// P2P transport
pub use p2p::{
    IncomingRequest, Keypair, P2pConfig, P2pConnection, P2pTransport, SyncCodec, SyncRequest,
    SyncResponse, PRIVSTACK_BOOTSTRAP_NODE,
};

// Pairing
pub use pairing::{
    DiscoveredPeerInfo, PairingManager, PairingMessage, PairingStatus, SyncCode, SyncCodeError,
    TrustedPeer,
};
