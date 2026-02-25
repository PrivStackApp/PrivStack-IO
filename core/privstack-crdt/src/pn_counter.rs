//! Positive-Negative Counter CRDT.
//!
//! A PN-Counter supports both increment and decrement operations across
//! distributed peers. It uses two internal maps (positive and negative)
//! keyed by peer ID. The value is `sum(positive) - sum(negative)`.
//!
//! Satisfies commutativity, associativity, and idempotency for merge.

use privstack_types::PeerId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A Positive-Negative Counter CRDT.
///
/// Each peer tracks its own increments and decrements independently.
/// The counter value is the difference between all increments and all decrements.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PNCounter {
    positive: HashMap<PeerId, u64>,
    negative: HashMap<PeerId, u64>,
}

impl PNCounter {
    /// Creates a new counter with value 0.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Increments the counter by `amount` for the given peer.
    pub fn increment(&mut self, peer_id: PeerId, amount: u64) {
        *self.positive.entry(peer_id).or_insert(0) += amount;
    }

    /// Decrements the counter by `amount` for the given peer.
    pub fn decrement(&mut self, peer_id: PeerId, amount: u64) {
        *self.negative.entry(peer_id).or_insert(0) += amount;
    }

    /// Returns the current counter value (may be negative).
    #[must_use]
    pub fn value(&self) -> i64 {
        let pos: u64 = self.positive.values().sum();
        let neg: u64 = self.negative.values().sum();
        pos as i64 - neg as i64
    }

    /// Merges another PNCounter into this one (takes per-peer max).
    pub fn merge(&mut self, other: &Self) {
        for (&peer_id, &count) in &other.positive {
            let entry = self.positive.entry(peer_id).or_insert(0);
            *entry = (*entry).max(count);
        }
        for (&peer_id, &count) in &other.negative {
            let entry = self.negative.entry(peer_id).or_insert(0);
            *entry = (*entry).max(count);
        }
    }

    /// Returns a new counter that is the merge of this and another.
    #[must_use]
    pub fn merged(&self, other: &Self) -> Self {
        let mut result = self.clone();
        result.merge(other);
        result
    }
}

impl PartialEq for PNCounter {
    fn eq(&self, other: &Self) -> bool {
        // Two counters are equal if all per-peer values match
        let all_pos_peers: std::collections::HashSet<_> = self
            .positive
            .keys()
            .chain(other.positive.keys())
            .collect();
        let all_neg_peers: std::collections::HashSet<_> = self
            .negative
            .keys()
            .chain(other.negative.keys())
            .collect();

        for peer in all_pos_peers {
            if self.positive.get(peer).copied().unwrap_or(0)
                != other.positive.get(peer).copied().unwrap_or(0)
            {
                return false;
            }
        }
        for peer in all_neg_peers {
            if self.negative.get(peer).copied().unwrap_or(0)
                != other.negative.get(peer).copied().unwrap_or(0)
            {
                return false;
            }
        }
        true
    }
}

impl Eq for PNCounter {}
