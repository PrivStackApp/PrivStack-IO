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

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(n: u8) -> PeerId {
        PeerId::from_uuid(uuid::Uuid::from_bytes([
            n, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))
    }

    #[test]
    fn new_counter_is_zero() {
        let c = PNCounter::new();
        assert_eq!(c.value(), 0);
    }

    #[test]
    fn increment_increases_value() {
        let mut c = PNCounter::new();
        c.increment(peer(1), 5);
        assert_eq!(c.value(), 5);
        c.increment(peer(1), 3);
        assert_eq!(c.value(), 8);
    }

    #[test]
    fn decrement_decreases_value() {
        let mut c = PNCounter::new();
        c.increment(peer(1), 10);
        c.decrement(peer(1), 3);
        assert_eq!(c.value(), 7);
    }

    #[test]
    fn value_can_go_negative() {
        let mut c = PNCounter::new();
        c.decrement(peer(1), 5);
        assert_eq!(c.value(), -5);
    }

    #[test]
    fn merge_is_commutative() {
        let mut a = PNCounter::new();
        a.increment(peer(1), 3);
        a.decrement(peer(2), 1);

        let mut b = PNCounter::new();
        b.increment(peer(2), 5);
        b.decrement(peer(1), 2);

        let ab = a.merged(&b);
        let ba = b.merged(&a);
        assert_eq!(ab, ba);
        assert_eq!(ab.value(), ba.value());
    }

    #[test]
    fn merge_is_associative() {
        let mut a = PNCounter::new();
        a.increment(peer(1), 1);
        let mut b = PNCounter::new();
        b.increment(peer(2), 2);
        let mut c = PNCounter::new();
        c.decrement(peer(3), 1);

        let ab_c = a.merged(&b).merged(&c);
        let a_bc = a.merged(&b.merged(&c));
        assert_eq!(ab_c, a_bc);
    }

    #[test]
    fn merge_is_idempotent() {
        let mut a = PNCounter::new();
        a.increment(peer(1), 5);
        a.decrement(peer(2), 2);

        let aa = a.merged(&a);
        assert_eq!(a, aa);
        assert_eq!(a.value(), aa.value());
    }

    #[test]
    fn concurrent_increments_from_different_peers() {
        let mut a = PNCounter::new();
        a.increment(peer(1), 3);

        let mut b = PNCounter::new();
        b.increment(peer(2), 7);

        a.merge(&b);
        assert_eq!(a.value(), 10);
    }

    #[test]
    fn merge_takes_max_per_peer() {
        let mut a = PNCounter::new();
        a.increment(peer(1), 5);

        let mut b = PNCounter::new();
        b.increment(peer(1), 3); // peer 1 only did 3 in this replica

        let merged = a.merged(&b);
        assert_eq!(merged.value(), 5); // max(5, 3) = 5
    }

    #[test]
    fn merge_takes_max_for_decrements_too() {
        let mut a = PNCounter::new();
        a.decrement(peer(1), 10);

        let mut b = PNCounter::new();
        b.decrement(peer(1), 3);

        let merged = a.merged(&b);
        assert_eq!(merged.value(), -10); // max(10, 3) = 10 decrements
    }

    #[test]
    fn serialization_roundtrip() {
        let mut c = PNCounter::new();
        c.increment(peer(1), 10);
        c.increment(peer(2), 5);
        c.decrement(peer(1), 3);

        let json = serde_json::to_string(&c).unwrap();
        let parsed: PNCounter = serde_json::from_str(&json).unwrap();

        assert_eq!(c, parsed);
        assert_eq!(parsed.value(), 12); // 10 + 5 - 3
    }

    #[test]
    fn serialization_empty_counter() {
        let c = PNCounter::new();
        let json = serde_json::to_string(&c).unwrap();
        let parsed: PNCounter = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.value(), 0);
    }

    #[test]
    fn three_peer_convergence() {
        // Simulate 3 peers making independent changes, then syncing
        let mut a = PNCounter::new();
        let mut b = PNCounter::new();
        let mut c = PNCounter::new();

        a.increment(peer(1), 10);
        b.increment(peer(2), 20);
        b.decrement(peer(2), 5);
        c.decrement(peer(3), 3);

        // Full sync: every replica merges with every other
        let a_snap = a.clone();
        let b_snap = b.clone();
        let c_snap = c.clone();

        a.merge(&b_snap);
        a.merge(&c_snap);
        b.merge(&a_snap);
        b.merge(&c_snap);
        c.merge(&a_snap);
        c.merge(&b_snap);

        // All converge
        assert_eq!(a, b);
        assert_eq!(b, c);
        assert_eq!(a.value(), 22); // 10 + 20 - 5 - 3
    }

    #[test]
    fn increment_zero_is_noop_for_value() {
        let mut c = PNCounter::new();
        c.increment(peer(1), 0);
        assert_eq!(c.value(), 0);
    }

    #[test]
    fn decrement_zero_is_noop_for_value() {
        let mut c = PNCounter::new();
        c.decrement(peer(1), 0);
        assert_eq!(c.value(), 0);
    }

    #[test]
    fn many_small_increments_same_peer() {
        let mut c = PNCounter::new();
        for _ in 0..100 {
            c.increment(peer(1), 1);
        }
        assert_eq!(c.value(), 100);
    }

    #[test]
    fn equality_checks_per_peer_not_just_value() {
        // Two counters can have the same value but different internal state
        let mut a = PNCounter::new();
        a.increment(peer(1), 5);

        let mut b = PNCounter::new();
        b.increment(peer(2), 5);

        // Same value but different internal state
        assert_eq!(a.value(), b.value());
        assert_ne!(a, b);
    }

    #[test]
    fn default_is_same_as_new() {
        let a = PNCounter::new();
        let b = PNCounter::default();
        assert_eq!(a, b);
        assert_eq!(a.value(), 0);
    }

    #[test]
    fn merge_with_empty_is_identity() {
        let mut c = PNCounter::new();
        c.increment(peer(1), 7);
        c.decrement(peer(2), 2);

        let empty = PNCounter::new();
        let merged = c.merged(&empty);
        assert_eq!(merged, c);

        let merged2 = empty.merged(&c);
        assert_eq!(merged2, c);
    }
}
