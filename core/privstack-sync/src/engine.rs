//! Sync engine — stateful sync logic without I/O.
//!
//! The engine is a pure state machine. It produces and consumes `SyncMessage`s.
//! The orchestrator handles all I/O (sending/receiving via transport).

use crate::acl_applicator::AclEventHandler;
use crate::applicator::EventApplicator;
use crate::policy::{AllowAllPolicy, SyncPolicy};
use crate::protocol::{
    EventAckMessage, EventBatchMessage, HelloAckMessage, HelloMessage, SyncMessage,
    SyncRequestMessage, SyncStateMessage, MAX_BATCH_SIZE, PROTOCOL_VERSION,
};
use crate::state::{PeerSyncStatus, SyncState};
use privstack_storage::{EntityStore, EventStore};
use privstack_types::{EntityId, Event, EventId, PeerId};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Configuration for the sync engine.
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Device name for identification.
    pub device_name: String,
    /// Maximum events per batch.
    pub batch_size: usize,
    /// Timeout for sync operations (ms).
    pub timeout_ms: u64,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            device_name: "PrivStack Device".to_string(),
            batch_size: MAX_BATCH_SIZE,
            timeout_ms: 30_000,
        }
    }
}

/// The sync engine — produces/consumes messages, manages state.
pub struct SyncEngine {
    /// Our peer ID.
    peer_id: PeerId,
    /// Configuration.
    config: SyncConfig,
    /// Sync state (vector clocks + seen event IDs per entity).
    state: Arc<RwLock<SyncState>>,
    /// Connected peers.
    peers: Arc<RwLock<HashMap<PeerId, PeerSyncStatus>>>,
    /// Per-peer known event IDs received via SyncRequest (for bidirectional ack).
    peer_known_ids: Arc<RwLock<HashMap<PeerId, HashMap<EntityId, HashSet<EventId>>>>>,
    /// Sync policy for access control.
    policy: Arc<dyn SyncPolicy>,
    /// Optional ACL event handler for ACL-as-CRDT propagation.
    acl_handler: Option<Arc<dyn AclEventHandler>>,
}

impl SyncEngine {
    /// Creates a new sync engine with the default `AllowAllPolicy`.
    pub fn new(peer_id: PeerId, config: SyncConfig) -> Self {
        Self::with_policy(peer_id, config, Arc::new(AllowAllPolicy))
    }

    /// Creates a new sync engine with a custom policy.
    pub fn with_policy(
        peer_id: PeerId,
        config: SyncConfig,
        policy: Arc<dyn SyncPolicy>,
    ) -> Self {
        Self {
            peer_id,
            config,
            state: Arc::new(RwLock::new(SyncState::new(peer_id))),
            peers: Arc::new(RwLock::new(HashMap::new())),
            peer_known_ids: Arc::new(RwLock::new(HashMap::new())),
            policy,
            acl_handler: None,
        }
    }

    /// Sets the ACL event handler for ACL-as-CRDT propagation.
    pub fn set_acl_handler(&mut self, handler: Arc<dyn AclEventHandler>) {
        self.acl_handler = Some(handler);
    }

    /// Returns a reference to the policy.
    pub fn policy(&self) -> &Arc<dyn SyncPolicy> {
        &self.policy
    }

    /// Returns our peer ID.
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Returns the device name.
    pub fn device_name(&self) -> &str {
        &self.config.device_name
    }

    /// Returns the batch size.
    pub fn batch_size(&self) -> usize {
        self.config.batch_size
    }

    // ── Message producers ────────────────────────────────────────

    /// Produces a Hello message to send to a peer.
    pub fn make_hello(&self, entity_ids: Vec<EntityId>) -> SyncMessage {
        let hello = HelloMessage::new(self.peer_id, &self.config.device_name)
            .with_entities(entity_ids);
        SyncMessage::Hello(hello)
    }

    /// Produces a HelloAck (accept) response.
    pub fn make_hello_accept(&self) -> SyncMessage {
        SyncMessage::HelloAck(HelloAckMessage::accept(self.peer_id, &self.config.device_name))
    }

    /// Produces a HelloAck (reject) response.
    pub fn make_hello_reject(&self, reason: impl Into<String>) -> SyncMessage {
        SyncMessage::HelloAck(HelloAckMessage::reject(self.peer_id, reason))
    }

    /// Produces a SyncRequest message for the given entities, including
    /// our known event IDs so the responder can compute a reverse delta.
    pub async fn make_sync_request(
        &self,
        entity_ids: Vec<EntityId>,
        event_store: &Arc<EventStore>,
    ) -> SyncMessage {
        let mut known_event_ids = HashMap::new();
        for eid in &entity_ids {
            let ids = self.known_event_ids_from_store(eid, event_store).await;
            if !ids.is_empty() {
                known_event_ids.insert(*eid, ids.into_iter().collect());
            }
        }
        SyncMessage::SyncRequest(SyncRequestMessage {
            entity_ids,
            known_event_ids,
        })
    }

    /// Produces our SyncState message for the given entities.
    pub async fn make_sync_state(
        &self,
        entity_ids: &[EntityId],
        event_store: &Arc<EventStore>,
    ) -> SyncMessage {
        let state = self.state.read().await;
        let mut sync_state = SyncStateMessage::new();

        for eid in entity_ids {
            // Always fetch event IDs from store for bidirectional delta computation.
            let store = event_store.clone();
            let eid_owned = *eid;
            let stored_events = tokio::task::spawn_blocking(move || {
                store.get_events_for_entity(&eid_owned)
            })
            .await;

            let stored_events = stored_events.unwrap_or(Ok(Vec::new())).unwrap_or_default();
            let event_ids: Vec<EventId> = stored_events.iter().map(|e| e.id).collect();

            if let Some(entity_state) = state.get_entity(eid) {
                sync_state.add_entity(
                    *eid,
                    entity_state.clock.clone(),
                    entity_state.event_count,
                    event_ids,
                );
            } else {
                sync_state.add_entity(
                    *eid,
                    Default::default(),
                    stored_events.len(),
                    event_ids,
                );
            }
        }

        SyncMessage::SyncState(sync_state)
    }

    // ── Message handlers ─────────────────────────────────────────

    /// Handles a Hello message from a remote peer.
    /// Returns the response to send back.
    pub async fn handle_hello(&self, hello: &HelloMessage) -> SyncMessage {
        if hello.version != PROTOCOL_VERSION {
            return self.make_hello_reject(format!(
                "version mismatch: expected {PROTOCOL_VERSION}, got {}",
                hello.version
            ));
        }

        // Policy gate: handshake
        if let Err(e) = self.policy.on_handshake(&self.peer_id, &hello.peer_id).await {
            return self.make_hello_reject(e.to_string());
        }

        // Policy gate: device limit
        if let Err(e) = self
            .policy
            .on_device_check(&hello.peer_id, hello.device_id.as_deref())
            .await
        {
            return self.make_hello_reject(e.to_string());
        }

        let mut status = PeerSyncStatus::new(hello.peer_id, &hello.device_name);
        status.shared_entities = hello.entity_ids.clone();
        status.connected = true;
        self.peers.write().await.insert(hello.peer_id, status);

        self.make_hello_accept()
    }

    /// Handles a SyncRequest from a remote peer.
    /// Stores the initiator's known event IDs for bidirectional ack, then
    /// returns our SyncState response.
    pub async fn handle_sync_request(
        &self,
        peer_id: &PeerId,
        request: &SyncRequestMessage,
        event_store: &Arc<EventStore>,
    ) -> SyncMessage {
        // Policy gate: filter entity IDs the peer may access
        let allowed_entities = match self
            .policy
            .on_sync_request(peer_id, &request.entity_ids)
            .await
        {
            Ok(ids) => ids,
            Err(e) => {
                warn!("Policy denied sync request from {}: {}", peer_id, e);
                return SyncMessage::Error(crate::protocol::ErrorMessage::new(
                    403,
                    e.to_string(),
                ));
            }
        };

        // Store the initiator's known event IDs so handle_event_batch can
        // compute which of our events they're missing.
        if !request.known_event_ids.is_empty() {
            let mut peer_ids = self.peer_known_ids.write().await;
            let entry = peer_ids.entry(*peer_id).or_default();
            for (eid, ids) in &request.known_event_ids {
                entry.insert(*eid, ids.iter().copied().collect());
            }
        }

        self.make_sync_state(&allowed_entities, event_store).await
    }

    /// Computes events to send to a peer for a given entity, based on
    /// the peer's known event IDs. Returns batched EventBatch messages.
    /// This overload does not apply policy filtering (backward-compatible).
    pub async fn compute_event_batches(
        &self,
        entity_id: EntityId,
        peer_known_ids: &HashSet<EventId>,
        event_store: &Arc<EventStore>,
    ) -> Vec<SyncMessage> {
        self.compute_event_batches_inner(None, entity_id, peer_known_ids, event_store)
            .await
    }

    /// Computes events to send to a specific peer, applying policy filtering.
    pub async fn compute_event_batches_for_peer(
        &self,
        peer_id: &PeerId,
        entity_id: EntityId,
        peer_known_ids: &HashSet<EventId>,
        event_store: &Arc<EventStore>,
    ) -> Vec<SyncMessage> {
        self.compute_event_batches_inner(Some(peer_id), entity_id, peer_known_ids, event_store)
            .await
    }

    async fn compute_event_batches_inner(
        &self,
        peer_id: Option<&PeerId>,
        entity_id: EntityId,
        peer_known_ids: &HashSet<EventId>,
        event_store: &Arc<EventStore>,
    ) -> Vec<SyncMessage> {
        let store = event_store.clone();
        let eid = entity_id;
        let all_events = match tokio::task::spawn_blocking(move || {
            store.get_events_for_entity(&eid)
        })
        .await
        {
            Ok(Ok(events)) => events,
            Ok(Err(e)) => {
                warn!("Failed to get events for entity {}: {}", entity_id, e);
                return Vec::new();
            }
            Err(e) => {
                warn!("spawn_blocking panicked for entity {}: {}", entity_id, e);
                return Vec::new();
            }
        };

        let state = self.state.read().await;
        let mut missing: Vec<Event> = state
            .compute_missing_events(&entity_id, peer_known_ids, all_events.iter())
            .into_iter()
            .cloned()
            .collect();
        drop(state);

        // Policy gate: filter outgoing events
        if let Some(pid) = peer_id {
            match self.policy.on_event_send(pid, &entity_id, &missing).await {
                Ok(filtered) => missing = filtered,
                Err(e) => {
                    warn!("Policy denied event send to {:?}: {}", pid, e);
                    return Vec::new();
                }
            }
        }

        if missing.is_empty() {
            return Vec::new();
        }

        info!("Found {} events to send for entity {}", missing.len(), entity_id);

        let total = missing.len();
        let mut batches = Vec::new();
        let mut sent = 0;

        for (seq, chunk) in missing.chunks(self.config.batch_size).enumerate() {
            sent += chunk.len();
            let is_final = sent >= total;
            let batch = EventBatchMessage {
                entity_id,
                events: chunk.to_vec(),
                is_final,
                batch_seq: seq as u32,
            };
            batches.push(SyncMessage::EventBatch(batch));
        }

        batches
    }

    /// Handles a received event batch — applies events and returns an ack.
    /// If the initiator sent known_event_ids, the ack includes events the
    /// initiator is missing (bidirectional sync).
    pub async fn handle_event_batch(
        &self,
        peer_id: &PeerId,
        batch: &EventBatchMessage,
        entity_store: &Arc<EntityStore>,
        event_store: &Arc<EventStore>,
    ) -> (SyncMessage, Vec<EntityId>) {
        // Policy gate: filter incoming events
        let allowed_events = match self
            .policy
            .on_event_receive(peer_id, &batch.entity_id, &batch.events)
            .await
        {
            Ok(evts) => evts,
            Err(e) => {
                warn!("Policy denied event batch from {}: {}", peer_id, e);
                let ack = EventAckMessage {
                    entity_id: batch.entity_id,
                    batch_seq: batch.batch_seq,
                    received_count: 0,
                    events: Vec::new(),
                };
                return (SyncMessage::EventAck(ack), Vec::new());
            }
        };

        let mut applied = 0;
        let mut updated_entities = HashSet::new();

        for event in &allowed_events {
            // Try ACL handler first — if it handles the event, skip normal applicator
            if let Some(acl_handler) = &self.acl_handler {
                match acl_handler.handle_acl_event(event).await {
                    Ok(true) => {
                        // ACL event handled — record in state and save
                        self.state
                            .write()
                            .await
                            .record_event(batch.entity_id, event);

                        let evs = event_store.clone();
                        let ev = event.clone();
                        let save_result = tokio::task::spawn_blocking(move || {
                            evs.save_event(&ev)
                        })
                        .await;
                        if let Err(e) = save_result.unwrap_or_else(|e| {
                            warn!("spawn_blocking panicked saving ACL event: {}", e);
                            Ok(())
                        }) {
                            warn!("Failed to save ACL event to store: {}", e);
                        }

                        applied += 1;
                        updated_entities.insert(event.entity_id);
                        debug!("Applied ACL event {:?} for entity {}", event.id, event.entity_id);
                        continue;
                    }
                    Ok(false) => {
                        // Not an ACL event, fall through to normal applicator
                    }
                    Err(e) => {
                        warn!("ACL handler error for event {:?}: {}", event.id, e);
                        // Fall through to normal applicator
                    }
                }
            }

            // Run the blocking apply_event on a dedicated thread
            let es = entity_store.clone();
            let ev = event.clone();
            let app_peer = self.peer_id;
            let apply_result = tokio::task::spawn_blocking(move || {
                let applicator = EventApplicator::new(app_peer);
                applicator.apply_event(&ev, &es, None, None)
            })
            .await;

            match apply_result {
                Ok(Ok(was_applied)) => {
                    if was_applied {
                        self.state
                            .write()
                            .await
                            .record_event(batch.entity_id, event);

                        let evs = event_store.clone();
                        let ev = event.clone();
                        let save_result = tokio::task::spawn_blocking(move || {
                            evs.save_event(&ev)
                        })
                        .await;
                        if let Err(e) = save_result.unwrap_or_else(|e| {
                            warn!("spawn_blocking panicked saving event: {}", e);
                            Ok(())
                        }) {
                            warn!("Failed to save event to store: {}", e);
                        }

                        applied += 1;
                        updated_entities.insert(event.entity_id);
                        debug!("Applied event {:?} to entity {}", event.id, event.entity_id);
                    }
                }
                Ok(Err(e)) => {
                    warn!("Failed to apply event {:?}: {}", event.id, e);
                }
                Err(e) => {
                    warn!("spawn_blocking panicked for event {:?}: {}", event.id, e);
                }
            }
        }

        info!(
            "Applied {}/{} events for entity {}",
            applied,
            batch.events.len(),
            batch.entity_id
        );

        // Compute reverse delta: events we have that the initiator is missing.
        let mut reverse_events = Vec::new();
        if batch.is_final {
            // Build the full set of IDs the initiator knows: their declared known_event_ids
            // plus everything they just sent us in this (and prior) batches.
            let peer_ids = self.peer_known_ids.read().await;
            let initiator_known: HashSet<EventId> = peer_ids
                .get(peer_id)
                .and_then(|m| m.get(&batch.entity_id))
                .cloned()
                .unwrap_or_default();
            drop(peer_ids);

            // Also include the batch event IDs (they obviously have what they sent).
            let mut all_initiator_known = initiator_known;
            for ev in &batch.events {
                all_initiator_known.insert(ev.id);
            }

            // Fetch our events for this entity and find what they're missing.
            let store = event_store.clone();
            let eid = batch.entity_id;
            let our_events = tokio::task::spawn_blocking(move || {
                store.get_events_for_entity(&eid)
            })
            .await;

            if let Ok(Ok(our_events)) = our_events {
                for ev in our_events {
                    if !all_initiator_known.contains(&ev.id) {
                        reverse_events.push(ev);
                    }
                }
            }

            // Policy gate: filter reverse-delta events before sending back
            if !reverse_events.is_empty() {
                match self
                    .policy
                    .on_event_send(peer_id, &batch.entity_id, &reverse_events)
                    .await
                {
                    Ok(filtered) => reverse_events = filtered,
                    Err(e) => {
                        warn!(
                            "Policy denied reverse-delta send to {}: {}, sending empty",
                            peer_id, e
                        );
                        reverse_events = Vec::new();
                    }
                }
            }

            if !reverse_events.is_empty() {
                info!(
                    "Sending {} reverse-delta events for entity {} back to peer {}",
                    reverse_events.len(),
                    batch.entity_id,
                    peer_id
                );
            }

            // Clean up stored peer known IDs for this entity.
            let mut peer_ids = self.peer_known_ids.write().await;
            if let Some(m) = peer_ids.get_mut(peer_id) {
                m.remove(&batch.entity_id);
                if m.is_empty() {
                    peer_ids.remove(peer_id);
                }
            }
        }

        let ack = EventAckMessage {
            entity_id: batch.entity_id,
            batch_seq: batch.batch_seq,
            received_count: applied,
            events: reverse_events,
        };

        (SyncMessage::EventAck(ack), updated_entities.into_iter().collect())
    }

    /// Records a local event into the sync state.
    pub async fn record_local_event(&self, event: &Event) {
        self.state.write().await.record_event(event.entity_id, event);
    }

    /// Builds the set of known event IDs for an entity from the event store.
    pub async fn known_event_ids_from_store(
        &self,
        entity_id: &EntityId,
        event_store: &Arc<EventStore>,
    ) -> HashSet<EventId> {
        let store = event_store.clone();
        let eid = *entity_id;
        tokio::task::spawn_blocking(move || {
            store
                .get_events_for_entity(&eid)
                .unwrap_or_default()
                .into_iter()
                .map(|e| e.id)
                .collect()
        })
        .await
        .unwrap_or_default()
    }

    // ── Peer management ──────────────────────────────────────────

    /// Returns the list of connected peers.
    pub async fn connected_peers(&self) -> Vec<PeerSyncStatus> {
        self.peers
            .read()
            .await
            .values()
            .filter(|p| p.connected)
            .cloned()
            .collect()
    }

    /// Returns all known peers.
    pub async fn all_peers(&self) -> Vec<PeerSyncStatus> {
        self.peers.read().await.values().cloned().collect()
    }

    /// Marks a peer as disconnected.
    pub async fn peer_disconnected(&self, peer_id: &PeerId) {
        if let Some(status) = self.peers.write().await.get_mut(peer_id) {
            status.connected = false;
        }
    }
}
