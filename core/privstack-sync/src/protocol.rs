//! Sync protocol messages and types.
//!
//! The sync protocol uses a simple request-response model:
//! 1. Peers exchange their sync state (vector clocks)
//! 2. Each peer determines what events the other is missing
//! 3. Missing events are sent in batches
//!
//! This is a CRDT-based sync, so events can be applied in any order
//! and will converge to the same state.

use privstack_crdt::VectorClock;
use privstack_types::{EntityId, Event, EventId, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Protocol version for compatibility checking.
pub const PROTOCOL_VERSION: u32 = 1;

/// Maximum number of events to send in a single batch.
pub const MAX_BATCH_SIZE: usize = 100;

/// A sync protocol message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncMessage {
    /// Handshake message sent when connecting.
    Hello(HelloMessage),

    /// Response to a Hello message.
    HelloAck(HelloAckMessage),

    /// Request sync state for specific documents.
    SyncRequest(SyncRequestMessage),

    /// Response with sync state.
    SyncState(SyncStateMessage),

    /// Batch of events to apply.
    EventBatch(EventBatchMessage),

    /// Acknowledgment of received events.
    EventAck(EventAckMessage),

    /// Request to subscribe to real-time updates.
    Subscribe(SubscribeMessage),

    /// Real-time event notification.
    EventNotify(EventNotifyMessage),

    /// Ping for keepalive.
    Ping(u64),

    /// Pong response.
    Pong(u64),

    /// Error message.
    Error(ErrorMessage),
}

/// Initial handshake message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloMessage {
    /// Protocol version.
    pub version: u32,
    /// Sender's peer ID.
    pub peer_id: PeerId,
    /// Human-readable device name.
    pub device_name: String,
    /// List of document IDs this peer has.
    pub entity_ids: Vec<EntityId>,
    /// Optional device identifier for device-limit enforcement.
    #[serde(default)]
    pub device_id: Option<String>,
}

impl HelloMessage {
    /// Creates a new Hello message.
    pub fn new(peer_id: PeerId, device_name: impl Into<String>) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            peer_id,
            device_name: device_name.into(),
            entity_ids: Vec::new(),
            device_id: None,
        }
    }

    /// Adds document IDs to the message.
    pub fn with_entities(mut self, ids: Vec<EntityId>) -> Self {
        self.entity_ids = ids;
        self
    }

    /// Sets the device ID for device-limit enforcement.
    pub fn with_device_id(mut self, device_id: impl Into<String>) -> Self {
        self.device_id = Some(device_id.into());
        self
    }
}

/// Response to Hello message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAckMessage {
    /// Protocol version.
    pub version: u32,
    /// Responder's peer ID.
    pub peer_id: PeerId,
    /// Human-readable device name.
    pub device_name: String,
    /// Whether the connection is accepted.
    pub accepted: bool,
    /// Reason if not accepted.
    pub reason: Option<String>,
}

impl HelloAckMessage {
    /// Creates an accepting HelloAck.
    pub fn accept(peer_id: PeerId, device_name: impl Into<String>) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            peer_id,
            device_name: device_name.into(),
            accepted: true,
            reason: None,
        }
    }

    /// Creates a rejecting HelloAck.
    pub fn reject(peer_id: PeerId, reason: impl Into<String>) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            peer_id,
            device_name: String::new(),
            accepted: false,
            reason: Some(reason.into()),
        }
    }
}

/// Request sync state for documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequestMessage {
    /// Documents to sync (empty = all shared documents).
    pub entity_ids: Vec<EntityId>,
    /// Known event IDs per entity so the responder can compute a reverse delta.
    #[serde(default)]
    pub known_event_ids: HashMap<EntityId, Vec<EventId>>,
}

/// Sync state response with vector clocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStateMessage {
    /// Vector clock for each document.
    pub clocks: HashMap<EntityId, VectorClock>,
    /// Total event count per document (for progress indication).
    pub event_counts: HashMap<EntityId, usize>,
    /// Known event IDs per entity for exact delta computation.
    #[serde(default)]
    pub known_event_ids: HashMap<EntityId, Vec<EventId>>,
}

impl SyncStateMessage {
    /// Creates a new empty sync state.
    pub fn new() -> Self {
        Self {
            clocks: HashMap::new(),
            event_counts: HashMap::new(),
            known_event_ids: HashMap::new(),
        }
    }

    /// Adds a document's sync state.
    pub fn add_entity(
        &mut self,
        id: EntityId,
        clock: VectorClock,
        event_count: usize,
        event_ids: Vec<EventId>,
    ) {
        self.clocks.insert(id, clock);
        self.event_counts.insert(id, event_count);
        if !event_ids.is_empty() {
            self.known_event_ids.insert(id, event_ids);
        }
    }
}

impl Default for SyncStateMessage {
    fn default() -> Self {
        Self::new()
    }
}

/// Batch of events to sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBatchMessage {
    /// Document these events belong to.
    pub entity_id: EntityId,
    /// The events.
    pub events: Vec<Event>,
    /// Whether this is the last batch for this document.
    pub is_final: bool,
    /// Batch sequence number (for ordering).
    pub batch_seq: u32,
}

impl EventBatchMessage {
    /// Creates a new event batch.
    pub fn new(entity_id: EntityId, events: Vec<Event>, batch_seq: u32) -> Self {
        Self {
            entity_id,
            events,
            is_final: false,
            batch_seq,
        }
    }

    /// Marks this as the final batch.
    pub fn finalize(mut self) -> Self {
        self.is_final = true;
        self
    }
}

/// Acknowledgment for received events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventAckMessage {
    /// Document the ack is for.
    pub entity_id: EntityId,
    /// Batch sequence number being acknowledged.
    pub batch_seq: u32,
    /// Number of events received/applied.
    pub received_count: usize,
    /// Events we want to send back (bidirectional sync).
    #[serde(default)]
    pub events: Vec<Event>,
}

/// Subscribe to real-time updates for documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeMessage {
    /// Documents to subscribe to.
    pub entity_ids: Vec<EntityId>,
}

/// Real-time event notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventNotifyMessage {
    /// The event that occurred.
    pub event: Event,
}

/// Error message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    /// Error code.
    pub code: u32,
    /// Human-readable error message.
    pub message: String,
}

impl ErrorMessage {
    /// Creates a new error message.
    pub fn new(code: u32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Version mismatch error.
    pub fn version_mismatch(expected: u32, got: u32) -> Self {
        Self::new(
            1,
            format!("protocol version mismatch: expected {expected}, got {got}"),
        )
    }

    /// Unknown document error.
    pub fn unknown_entity(id: &EntityId) -> Self {
        Self::new(2, format!("unknown document: {id}"))
    }

    /// Internal error.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(99, msg)
    }
}
