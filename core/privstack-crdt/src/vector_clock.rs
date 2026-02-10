//! Vector Clock for causality tracking.
//!
//! A vector clock tracks the logical time across multiple peers, enabling
//! determination of causality (happens-before relationships) between events.
//!
//! Use cases:
//! - Detecting concurrent operations
//! - Ordering events for CRDT merge
//! - Sync protocol (knowing what events a peer has seen)

use privstack_types::PeerId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Causality relationship between two vector clocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CausalOrder {
    /// First clock happened before second.
    Before,
    /// First clock happened after second.
    After,
    /// Clocks are concurrent (neither happened before the other).
    Concurrent,
    /// Clocks are identical.
    Equal,
}

/// A Vector Clock for tracking causality across peers.
///
/// Each peer has a logical counter that increments with each event.
/// By comparing vector clocks, we can determine if events are causally
/// related or concurrent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VectorClock {
    /// Map from peer ID to logical time at that peer.
    clocks: HashMap<PeerId, u64>,
}

impl VectorClock {
    /// Creates a new empty vector clock.
    #[must_use]
    pub fn new() -> Self {
        Self {
            clocks: HashMap::new(),
        }
    }

    /// Creates a vector clock with a single peer's initial time.
    #[must_use]
    pub fn for_peer(peer_id: PeerId) -> Self {
        let mut clocks = HashMap::new();
        clocks.insert(peer_id, 0);
        Self { clocks }
    }

    /// Returns the logical time for a peer (0 if not present).
    #[must_use]
    pub fn get(&self, peer_id: &PeerId) -> u64 {
        self.clocks.get(peer_id).copied().unwrap_or(0)
    }

    /// Returns all peers and their times.
    pub fn peers(&self) -> impl Iterator<Item = (&PeerId, &u64)> {
        self.clocks.iter()
    }

    /// Returns the number of peers in the clock.
    #[must_use]
    pub fn len(&self) -> usize {
        self.clocks.len()
    }

    /// Returns true if the clock has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.clocks.is_empty()
    }

    /// Increments the clock for a peer and returns the new time.
    ///
    /// This should be called when the peer creates a new event.
    pub fn increment(&mut self, peer_id: PeerId) -> u64 {
        let entry = self.clocks.entry(peer_id).or_insert(0);
        *entry += 1;
        *entry
    }

    /// Updates the clock for a peer to a specific time.
    ///
    /// Only updates if the new time is greater than the current time.
    pub fn update(&mut self, peer_id: PeerId, time: u64) {
        let entry = self.clocks.entry(peer_id).or_insert(0);
        if time > *entry {
            *entry = time;
        }
    }

    /// Merges another vector clock into this one.
    ///
    /// For each peer, takes the maximum of the two times.
    /// This operation is commutative, associative, and idempotent.
    pub fn merge(&mut self, other: &Self) {
        for (peer_id, &time) in &other.clocks {
            let entry = self.clocks.entry(*peer_id).or_insert(0);
            if time > *entry {
                *entry = time;
            }
        }
    }

    /// Creates a new clock that is the merge of this and another.
    #[must_use]
    pub fn merged(&self, other: &Self) -> Self {
        let mut result = self.clone();
        result.merge(other);
        result
    }

    /// Compares this clock with another to determine causal ordering.
    #[must_use]
    pub fn compare(&self, other: &Self) -> CausalOrder {
        let mut dominated_by_self = true; // self >= other for all peers
        let mut dominated_by_other = true; // other >= self for all peers

        // Collect all peer IDs from both clocks
        let all_peers: std::collections::HashSet<_> = self
            .clocks
            .keys()
            .chain(other.clocks.keys())
            .copied()
            .collect();

        for peer_id in all_peers {
            let self_time = self.get(&peer_id);
            let other_time = other.get(&peer_id);

            if self_time < other_time {
                dominated_by_self = false;
            }
            if other_time < self_time {
                dominated_by_other = false;
            }
        }

        match (dominated_by_self, dominated_by_other) {
            (true, true) => CausalOrder::Equal,
            (true, false) => CausalOrder::After,
            (false, true) => CausalOrder::Before,
            (false, false) => CausalOrder::Concurrent,
        }
    }

    /// Returns true if this clock is causally before the other.
    #[must_use]
    pub fn is_before(&self, other: &Self) -> bool {
        self.compare(other) == CausalOrder::Before
    }

    /// Returns true if this clock is causally after the other.
    #[must_use]
    pub fn is_after(&self, other: &Self) -> bool {
        self.compare(other) == CausalOrder::After
    }

    /// Returns true if this clock is concurrent with the other.
    #[must_use]
    pub fn is_concurrent(&self, other: &Self) -> bool {
        self.compare(other) == CausalOrder::Concurrent
    }

    /// Returns true if this clock dominates the other (is >= for all peers).
    #[must_use]
    pub fn dominates(&self, other: &Self) -> bool {
        matches!(self.compare(other), CausalOrder::After | CausalOrder::Equal)
    }
}

impl PartialEq for VectorClock {
    fn eq(&self, other: &Self) -> bool {
        self.compare(other) == CausalOrder::Equal
    }
}

impl Eq for VectorClock {}
