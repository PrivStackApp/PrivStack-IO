//! Last-Writer-Wins Register (LWW-Register).
//!
//! A CRDT that stores a single value. Concurrent writes are resolved by
//! comparing timestamps — the write with the highest timestamp wins.
//!
//! Use cases:
//! - Single-value properties (document title, block type, etc.)
//! - Any field where "last write wins" semantics are acceptable

use privstack_types::{HybridTimestamp, PeerId};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// A Last-Writer-Wins Register.
///
/// Stores a value of type `T` along with metadata for conflict resolution.
/// When two replicas have different values, the one with the higher timestamp wins.
/// If timestamps are equal, the higher peer ID breaks the tie (arbitrary but deterministic).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LWWRegister<T> {
    /// The current value.
    value: T,
    /// Timestamp of the last write.
    timestamp: HybridTimestamp,
    /// Peer that performed the last write.
    peer_id: PeerId,
}

impl<T> LWWRegister<T> {
    /// Creates a new register with the given initial value.
    #[must_use]
    pub fn new(value: T, peer_id: PeerId) -> Self {
        Self {
            value,
            timestamp: HybridTimestamp::now(),
            peer_id,
        }
    }

    /// Creates a register with explicit timestamp (for testing or replay).
    #[must_use]
    pub fn with_timestamp(value: T, timestamp: HybridTimestamp, peer_id: PeerId) -> Self {
        Self {
            value,
            timestamp,
            peer_id,
        }
    }

    /// Returns a reference to the current value.
    #[must_use]
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Returns the timestamp of the last write.
    #[must_use]
    pub fn timestamp(&self) -> HybridTimestamp {
        self.timestamp
    }

    /// Returns the peer that performed the last write.
    #[must_use]
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Sets a new value, updating the timestamp.
    ///
    /// The timestamp is incremented from the current timestamp to ensure
    /// monotonicity even if the system clock hasn't advanced.
    pub fn set(&mut self, value: T, peer_id: PeerId) {
        self.value = value;
        self.timestamp = self.timestamp.tick();
        self.peer_id = peer_id;
    }

    /// Sets a new value with an explicit timestamp.
    ///
    /// Only updates if the new timestamp is greater than the current one,
    /// or if timestamps are equal and the peer ID is greater.
    /// Returns true if the value was updated.
    pub fn set_with_timestamp(
        &mut self,
        value: T,
        timestamp: HybridTimestamp,
        peer_id: PeerId,
    ) -> bool {
        if self.should_update(timestamp, peer_id) {
            self.value = value;
            self.timestamp = timestamp;
            self.peer_id = peer_id;
            true
        } else {
            false
        }
    }

    /// Determines if an incoming write should win over the current value.
    fn should_update(&self, timestamp: HybridTimestamp, peer_id: PeerId) -> bool {
        match timestamp.cmp(&self.timestamp) {
            Ordering::Greater => true,
            Ordering::Less => false,
            // Tie-breaker: compare peer IDs (arbitrary but deterministic)
            Ordering::Equal => peer_id.as_uuid() > self.peer_id.as_uuid(),
        }
    }
}

impl<T: Clone> LWWRegister<T> {
    /// Merges another register into this one.
    ///
    /// The value with the higher timestamp (or higher peer ID on tie) wins.
    /// This operation is:
    /// - Commutative: merge(a, b) == merge(b, a)
    /// - Associative: merge(merge(a, b), c) == merge(a, merge(b, c))
    /// - Idempotent: merge(a, a) == a
    pub fn merge(&mut self, other: &Self) {
        if self.should_update(other.timestamp, other.peer_id) {
            self.value = other.value.clone();
            self.timestamp = other.timestamp;
            self.peer_id = other.peer_id;
        }
    }

    /// Creates a new register that is the merge of this and another.
    #[must_use]
    pub fn merged(&self, other: &Self) -> Self {
        let mut result = self.clone();
        result.merge(other);
        result
    }
}

impl<T: PartialEq> PartialEq for LWWRegister<T> {
    fn eq(&self, other: &Self) -> bool {
        // Two registers are equal if they have the same value and timestamp
        self.value == other.value && self.timestamp == other.timestamp
    }
}

impl<T: Eq> Eq for LWWRegister<T> {}

impl<T: Default> Default for LWWRegister<T> {
    fn default() -> Self {
        Self {
            value: T::default(),
            timestamp: HybridTimestamp::now(),
            peer_id: PeerId::new(),
        }
    }
}
