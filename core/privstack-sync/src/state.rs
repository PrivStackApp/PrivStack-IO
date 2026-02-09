//! Sync state tracking.
//!
//! Tracks which events have been seen from each peer for each entity,
//! enabling efficient delta sync. Uses a monotonic event counter per peer
//! (not HybridTimestamp logical clocks, which are a different concept).

use privstack_crdt::VectorClock;
use privstack_types::{EntityId, Event, EventId, HybridTimestamp, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Tracks sync state for all entities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncState {
    /// Per-entity sync state.
    entities: HashMap<EntityId, EntitySyncState>,
    /// Our peer ID.
    local_peer_id: Option<PeerId>,
}

impl SyncState {
    /// Creates a new sync state.
    pub fn new(local_peer_id: PeerId) -> Self {
        Self {
            entities: HashMap::new(),
            local_peer_id: Some(local_peer_id),
        }
    }

    /// Gets the local peer ID.
    pub fn local_peer_id(&self) -> Option<PeerId> {
        self.local_peer_id
    }

    /// Sets the local peer ID.
    pub fn set_local_peer_id(&mut self, peer_id: PeerId) {
        self.local_peer_id = Some(peer_id);
    }

    /// Gets sync state for an entity.
    pub fn get_entity(&self, entity_id: &EntityId) -> Option<&EntitySyncState> {
        self.entities.get(entity_id)
    }

    /// Gets mutable sync state for an entity, creating if needed.
    pub fn get_or_create_entity(&mut self, entity_id: EntityId) -> &mut EntitySyncState {
        self.entities.entry(entity_id).or_default()
    }

    /// Removes sync state for an entity.
    pub fn remove_entity(&mut self, entity_id: &EntityId) {
        self.entities.remove(entity_id);
    }

    /// Returns all tracked entity IDs.
    pub fn entity_ids(&self) -> impl Iterator<Item = &EntityId> {
        self.entities.keys()
    }

    /// Records that an event was applied for a given entity.
    pub fn record_event(&mut self, entity_id: EntityId, event: &Event) {
        let entity_state = self.get_or_create_entity(entity_id);
        entity_state.record_event(event);
    }

    /// Gets the vector clock for an entity.
    pub fn get_clock(&self, entity_id: &EntityId) -> Option<&VectorClock> {
        self.entities.get(entity_id).map(|s| &s.clock)
    }

    /// Computes events that a peer is missing based on the set of event IDs
    /// they already have. This is the correct approach: compare event ID sets,
    /// not timestamp values.
    pub fn compute_missing_events<'a>(
        &self,
        entity_id: &EntityId,
        peer_known_event_ids: &HashSet<EventId>,
        all_events: impl Iterator<Item = &'a Event>,
    ) -> Vec<&'a Event> {
        // If we have no state for this entity, send everything
        let _our_state = self.entities.get(entity_id);

        all_events
            .filter(|event| !peer_known_event_ids.contains(&event.id))
            .collect()
    }

    /// Builds the set of known event IDs for an entity from stored events.
    pub fn known_event_ids(events: &[Event]) -> HashSet<EventId> {
        events.iter().map(|e| e.id).collect()
    }
}

/// Sync state for a single entity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntitySyncState {
    /// Vector clock — tracks a monotonic counter per peer.
    /// Each time we see an event from a peer, we increment their counter.
    pub clock: VectorClock,
    /// Set of event IDs we've seen (for deduplication and missing-event computation).
    pub seen_event_ids: HashSet<EventId>,
    /// Last sync time with each peer.
    pub last_sync: HashMap<PeerId, HybridTimestamp>,
    /// Number of events we have for this entity.
    pub event_count: usize,
}

impl EntitySyncState {
    /// Creates a new entity sync state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that an event was seen/applied.
    pub fn record_event(&mut self, event: &Event) {
        if self.seen_event_ids.insert(event.id) {
            // New event — increment the counter for this peer
            self.clock.increment(event.peer_id);
            self.event_count += 1;
        }
    }

    /// Records a sync with a peer.
    pub fn record_sync(&mut self, peer_id: PeerId, timestamp: HybridTimestamp) {
        self.last_sync.insert(peer_id, timestamp);
    }

    /// Merges another clock into ours (after receiving events).
    pub fn merge_clock(&mut self, other: &VectorClock) {
        self.clock.merge(other);
    }
}

/// Represents the sync status with a specific peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerSyncStatus {
    /// The peer's ID.
    pub peer_id: PeerId,
    /// The peer's device name.
    pub device_name: String,
    /// Entities shared with this peer.
    pub shared_entities: Vec<EntityId>,
    /// Whether currently connected.
    pub connected: bool,
    /// Last successful sync timestamp.
    pub last_sync: Option<HybridTimestamp>,
}

impl PeerSyncStatus {
    /// Creates a new peer sync status.
    pub fn new(peer_id: PeerId, device_name: impl Into<String>) -> Self {
        Self {
            peer_id,
            device_name: device_name.into(),
            shared_entities: Vec::new(),
            connected: false,
            last_sync: None,
        }
    }
}
