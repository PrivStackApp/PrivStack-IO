//! Property-based tests for CRDT correctness.
//!
//! These tests verify the fundamental mathematical properties that all CRDTs must satisfy:
//! - Commutativity: merge(A, B) == merge(B, A)
//! - Associativity: merge(merge(A, B), C) == merge(A, merge(B, C))
//! - Idempotence: merge(A, A) == A
//!
//! Additionally, we verify eventual consistency: all replicas converge regardless of
//! the order in which operations are received.

use privstack_crdt::{LWWRegister, ORSet, PNCounter, VectorClock, RGA};
use privstack_types::{HybridTimestamp, PeerId};
use proptest::prelude::*;
use std::collections::HashSet;

// =============================================================================
// HELPER STRATEGIES
// =============================================================================

fn peer_id_strategy() -> impl Strategy<Value = PeerId> {
    any::<u128>().prop_map(|_| PeerId::new())
}

fn timestamp_strategy() -> impl Strategy<Value = HybridTimestamp> {
    (1u64..1_000_000, 0u32..1000).prop_map(|(wall, counter)| HybridTimestamp::new(wall, counter))
}

fn char_strategy() -> impl Strategy<Value = char> {
    prop::char::range('a', 'z')
}

fn string_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9 ]{0,100}").unwrap()
}

// =============================================================================
// LWW REGISTER PROPERTY TESTS
// =============================================================================

mod lww_register_properties {
    use super::*;

    proptest! {
        /// Commutativity: merge(A, B) produces same result as merge(B, A)
        #[test]
        fn merge_is_commutative(
            v1 in string_strategy(),
            v2 in string_strategy(),
            ts1 in timestamp_strategy(),
            ts2 in timestamp_strategy(),
        ) {
            let peer1 = PeerId::new();
            let peer2 = PeerId::new();

            let reg1 = LWWRegister::with_timestamp(v1, ts1, peer1);
            let reg2 = LWWRegister::with_timestamp(v2, ts2, peer2);

            let merged_12 = reg1.merged(&reg2);
            let merged_21 = reg2.merged(&reg1);

            prop_assert_eq!(merged_12.value(), merged_21.value());
            prop_assert_eq!(merged_12.timestamp(), merged_21.timestamp());
        }

        /// Associativity: merge(merge(A, B), C) == merge(A, merge(B, C))
        #[test]
        fn merge_is_associative(
            v1 in string_strategy(),
            v2 in string_strategy(),
            v3 in string_strategy(),
            ts1 in timestamp_strategy(),
            ts2 in timestamp_strategy(),
            ts3 in timestamp_strategy(),
        ) {
            let peer1 = PeerId::new();
            let peer2 = PeerId::new();
            let peer3 = PeerId::new();

            let reg1 = LWWRegister::with_timestamp(v1, ts1, peer1);
            let reg2 = LWWRegister::with_timestamp(v2, ts2, peer2);
            let reg3 = LWWRegister::with_timestamp(v3, ts3, peer3);

            // (A merge B) merge C
            let left = reg1.merged(&reg2).merged(&reg3);

            // A merge (B merge C)
            let right = reg1.merged(&reg2.merged(&reg3));

            prop_assert_eq!(left.value(), right.value());
        }

        /// Idempotence: merge(A, A) == A
        #[test]
        fn merge_is_idempotent(
            v in string_strategy(),
            ts in timestamp_strategy(),
        ) {
            let peer = PeerId::new();
            let reg = LWWRegister::with_timestamp(v, ts, peer);

            let merged = reg.merged(&reg);

            prop_assert_eq!(reg.value(), merged.value());
            prop_assert_eq!(reg.timestamp(), merged.timestamp());
        }

        /// Higher timestamp always wins
        #[test]
        fn higher_timestamp_wins(
            v1 in string_strategy(),
            v2 in string_strategy(),
            base_ts in 100u64..500000,
        ) {
            let peer1 = PeerId::new();
            let peer2 = PeerId::new();

            let ts1 = HybridTimestamp::new(base_ts, 0);
            let ts2 = HybridTimestamp::new(base_ts + 100, 0); // ts2 > ts1

            let reg1 = LWWRegister::with_timestamp(v1, ts1, peer1);
            let reg2 = LWWRegister::with_timestamp(v2.clone(), ts2, peer2);

            let merged = reg1.merged(&reg2);

            prop_assert_eq!(merged.value(), &v2);
            prop_assert_eq!(merged.timestamp(), ts2);
        }

        /// Set operation increases timestamp
        #[test]
        fn set_increases_timestamp(
            initial in string_strategy(),
            updated in string_strategy(),
        ) {
            let peer = PeerId::new();
            let mut reg = LWWRegister::new(initial, peer);
            let ts_before = reg.timestamp();

            reg.set(updated, peer);

            prop_assert!(reg.timestamp() > ts_before);
        }
    }
}

// =============================================================================
// VECTOR CLOCK PROPERTY TESTS
// =============================================================================

mod vector_clock_properties {
    use super::*;

    proptest! {
        /// Commutativity: merge(A, B) == merge(B, A)
        #[test]
        fn merge_is_commutative(
            increments1 in prop::collection::vec((peer_id_strategy(), 1u32..10), 1..5),
            increments2 in prop::collection::vec((peer_id_strategy(), 1u32..10), 1..5),
        ) {
            let mut clock1 = VectorClock::new();
            let mut clock2 = VectorClock::new();

            for (peer, count) in &increments1 {
                for _ in 0..*count {
                    clock1.increment(*peer);
                }
            }

            for (peer, count) in &increments2 {
                for _ in 0..*count {
                    clock2.increment(*peer);
                }
            }

            let merged_12 = clock1.merged(&clock2);
            let merged_21 = clock2.merged(&clock1);

            prop_assert_eq!(merged_12, merged_21);
        }

        /// Associativity: merge(merge(A, B), C) == merge(A, merge(B, C))
        #[test]
        fn merge_is_associative(
            increments1 in prop::collection::vec(peer_id_strategy(), 1..5),
            increments2 in prop::collection::vec(peer_id_strategy(), 1..5),
            increments3 in prop::collection::vec(peer_id_strategy(), 1..5),
        ) {
            let mut clock1 = VectorClock::new();
            let mut clock2 = VectorClock::new();
            let mut clock3 = VectorClock::new();

            for peer in &increments1 { clock1.increment(*peer); }
            for peer in &increments2 { clock2.increment(*peer); }
            for peer in &increments3 { clock3.increment(*peer); }

            let left = clock1.merged(&clock2).merged(&clock3);
            let right = clock1.merged(&clock2.merged(&clock3));

            prop_assert_eq!(left, right);
        }

        /// Idempotence: merge(A, A) == A
        #[test]
        fn merge_is_idempotent(
            increments in prop::collection::vec(peer_id_strategy(), 1..10),
        ) {
            let mut clock = VectorClock::new();
            for peer in &increments {
                clock.increment(*peer);
            }

            let merged = clock.merged(&clock);

            prop_assert_eq!(clock, merged);
        }

        /// Increment is monotonic
        #[test]
        fn increment_is_monotonic(times in 1usize..100) {
            let peer = PeerId::new();
            let mut clock = VectorClock::new();

            let mut prev = 0u64;
            for _ in 0..times {
                let new_time = clock.increment(peer);
                prop_assert!(new_time > prev);
                prev = new_time;
            }

            prop_assert_eq!(clock.get(&peer), times as u64);
        }

        /// Merge takes maximum of each component
        #[test]
        fn merge_takes_max(
            peer in peer_id_strategy(),
            time1 in 1u64..1000,
            time2 in 1u64..1000,
        ) {
            let mut clock1 = VectorClock::new();
            let mut clock2 = VectorClock::new();

            for _ in 0..time1 { clock1.increment(peer); }
            for _ in 0..time2 { clock2.increment(peer); }

            let merged = clock1.merged(&clock2);

            prop_assert_eq!(merged.get(&peer), std::cmp::max(time1, time2));
        }

        /// Causality: A < B iff A happened-before B
        #[test]
        fn causality_ordering(
            peer in peer_id_strategy(),
            extra_increments in 1usize..10,
        ) {
            let mut clock1 = VectorClock::new();
            clock1.increment(peer);

            // clock2 = clock1 + more increments (clock1 happened-before clock2)
            let mut clock2 = clock1.clone();
            for _ in 0..extra_increments {
                clock2.increment(peer);
            }

            prop_assert!(clock1.is_before(&clock2));
            prop_assert!(clock2.is_after(&clock1));
            prop_assert!(!clock1.is_concurrent(&clock2));
        }

        /// Concurrent clocks are detected correctly
        #[test]
        fn concurrent_detection(
            peer1 in peer_id_strategy(),
            peer2 in peer_id_strategy(),
        ) {
            prop_assume!(peer1 != peer2);

            let mut clock1 = VectorClock::new();
            let mut clock2 = VectorClock::new();

            clock1.increment(peer1);
            clock2.increment(peer2);

            prop_assert!(clock1.is_concurrent(&clock2));
            prop_assert!(clock2.is_concurrent(&clock1));
        }
    }
}

// =============================================================================
// OR-SET PROPERTY TESTS
// =============================================================================

mod orset_properties {
    use super::*;

    proptest! {
        /// Commutativity: merge(A, B) contains same elements as merge(B, A)
        #[test]
        fn merge_is_commutative(
            ops1 in prop::collection::vec((any::<bool>(), 0i32..100), 0..20),
            ops2 in prop::collection::vec((any::<bool>(), 0i32..100), 0..20),
        ) {
            let peer1 = PeerId::new();
            let peer2 = PeerId::new();

            let mut set1: ORSet<i32> = ORSet::new();
            let mut set2: ORSet<i32> = ORSet::new();

            for (is_add, val) in &ops1 {
                if *is_add { set1.add(*val, peer1); }
                else { set1.remove(val); }
            }

            for (is_add, val) in &ops2 {
                if *is_add { set2.add(*val, peer2); }
                else { set2.remove(val); }
            }

            let merged_12 = set1.merged(&set2);
            let merged_21 = set2.merged(&set1);

            let elems_12: HashSet<_> = merged_12.iter().collect();
            let elems_21: HashSet<_> = merged_21.iter().collect();

            prop_assert_eq!(elems_12, elems_21);
        }

        /// Associativity: merge(merge(A, B), C) == merge(A, merge(B, C))
        #[test]
        fn merge_is_associative(
            items1 in prop::collection::vec(0i32..50, 0..10),
            items2 in prop::collection::vec(0i32..50, 0..10),
            items3 in prop::collection::vec(0i32..50, 0..10),
        ) {
            let peer1 = PeerId::new();
            let peer2 = PeerId::new();
            let peer3 = PeerId::new();

            let mut set1: ORSet<i32> = ORSet::new();
            let mut set2: ORSet<i32> = ORSet::new();
            let mut set3: ORSet<i32> = ORSet::new();

            for item in &items1 { set1.add(*item, peer1); }
            for item in &items2 { set2.add(*item, peer2); }
            for item in &items3 { set3.add(*item, peer3); }

            let left = set1.merged(&set2).merged(&set3);
            let right = set1.merged(&set2.merged(&set3));

            let elems_left: HashSet<_> = left.iter().collect();
            let elems_right: HashSet<_> = right.iter().collect();

            prop_assert_eq!(elems_left, elems_right);
        }

        /// Idempotence: merge(A, A) == A
        #[test]
        fn merge_is_idempotent(
            items in prop::collection::vec(0i32..100, 0..20),
        ) {
            let peer = PeerId::new();
            let mut set: ORSet<i32> = ORSet::new();

            for item in &items {
                set.add(*item, peer);
            }

            let merged = set.merged(&set);

            prop_assert_eq!(set.len(), merged.len());
            for item in set.iter() {
                prop_assert!(merged.contains(item));
            }
        }

        /// Add-wins semantics: concurrent add and remove results in element present
        #[test]
        fn add_wins(
            item in 0i32..1000,
        ) {
            let peer1 = PeerId::new();

            // Initial state: item is present
            let mut set1: ORSet<i32> = ORSet::new();
            set1.add(item, peer1);

            // Fork
            let mut set2 = set1.clone();

            // Peer 1 removes
            set2.remove(&item);

            // Peer 2 adds concurrently (new unique tag)
            set1.add(item, peer1);

            // Merge - add should win
            set1.merge(&set2);
            prop_assert!(set1.contains(&item));

            // Other direction should also have item
            set2.merge(&set1);
            prop_assert!(set2.contains(&item));
        }

        /// Re-add after remove works
        #[test]
        fn re_add_after_remove(
            item in 0i32..1000,
        ) {
            let peer = PeerId::new();
            let mut set: ORSet<i32> = ORSet::new();

            set.add(item, peer);
            prop_assert!(set.contains(&item));

            set.remove(&item);
            prop_assert!(!set.contains(&item));

            set.add(item, peer);
            prop_assert!(set.contains(&item));
        }

        /// Eventual consistency: all replicas converge after full sync
        #[test]
        fn eventual_consistency(
            ops in prop::collection::vec((0u8..3, any::<bool>(), 0i32..50), 1..30),
        ) {
            let peers = [PeerId::new(), PeerId::new(), PeerId::new()];
            let mut sets: [ORSet<i32>; 3] = [ORSet::new(), ORSet::new(), ORSet::new()];

            // Apply operations to different replicas
            for (node_idx, is_add, val) in &ops {
                let idx = (*node_idx as usize) % 3;
                if *is_add {
                    sets[idx].add(*val, peers[idx]);
                } else {
                    sets[idx].remove(val);
                }
            }

            // Full sync: every replica merges with every other
            for i in 0..3 {
                for j in 0..3 {
                    if i != j {
                        let other = sets[j].clone();
                        sets[i].merge(&other);
                    }
                }
            }

            // All should have same elements now
            let elems0: HashSet<_> = sets[0].iter().copied().collect();
            let elems1: HashSet<_> = sets[1].iter().copied().collect();
            let elems2: HashSet<_> = sets[2].iter().copied().collect();

            prop_assert_eq!(&elems0, &elems1);
            prop_assert_eq!(&elems1, &elems2);
        }
    }
}

// =============================================================================
// RGA PROPERTY TESTS
// =============================================================================

mod rga_properties {
    use super::*;

    proptest! {
        /// Commutativity: merge produces same sequence regardless of order
        #[test]
        fn merge_is_commutative(
            text1 in "[a-z]{0,20}",
            text2 in "[a-z]{0,20}",
        ) {
            let peer1 = PeerId::new();
            let peer2 = PeerId::new();

            let rga1 = RGA::from_str(&text1, peer1);
            let mut rga2 = RGA::from_str(&text2, peer2);
            rga2.set_peer_id(peer2);

            let merged_12 = rga1.merged(&rga2);
            let merged_21 = rga2.merged(&rga1);

            prop_assert_eq!(merged_12.to_vec(), merged_21.to_vec());
        }

        /// Idempotence: merge(A, A) == A
        #[test]
        fn merge_is_idempotent(
            text in "[a-z]{0,30}",
        ) {
            let peer = PeerId::new();
            let rga = RGA::from_str(&text, peer);

            let merged = rga.merged(&rga);

            prop_assert_eq!(rga.to_vec(), merged.to_vec());
        }

        /// Insert at any valid position succeeds
        #[test]
        fn insert_at_valid_positions(
            initial in "[a-z]{0,10}",
            char_to_insert in char_strategy(),
            pos_factor in 0.0f64..=1.0,
        ) {
            let peer = PeerId::new();
            let mut rga = RGA::from_str(&initial, peer);
            let len = rga.len();
            let pos = (pos_factor * (len as f64 + 1.0)).floor() as usize;
            let pos = pos.min(len);

            rga.insert(pos, char_to_insert);

            prop_assert_eq!(rga.len(), len + 1);
            prop_assert_eq!(rga.get(pos), Some(&char_to_insert));
        }

        /// Delete at any valid position succeeds
        #[test]
        fn delete_at_valid_positions(
            initial in "[a-z]{1,20}",
            pos_factor in 0.0f64..1.0,
        ) {
            let peer = PeerId::new();
            let mut rga = RGA::from_str(&initial, peer);
            let len = rga.len();
            let pos = (pos_factor * len as f64).floor() as usize;
            let pos = pos.min(len - 1);

            let deleted_id = rga.delete(pos);

            prop_assert!(deleted_id.is_some());
            prop_assert_eq!(rga.len(), len - 1);
        }

        /// Concurrent inserts at same position both appear
        #[test]
        fn concurrent_inserts_both_appear(
            base in "[a-z]{0,10}",
            char1 in prop::char::range('A', 'M'),
            char2 in prop::char::range('N', 'Z'),
            pos_factor in 0.0f64..=1.0,
        ) {
            let peer1 = PeerId::new();
            let peer2 = PeerId::new();

            let mut rga1 = RGA::from_str(&base, peer1);
            let mut rga2 = rga1.clone();
            rga2.set_peer_id(peer2);

            let pos = (pos_factor * (rga1.len() as f64 + 1.0)).floor() as usize;
            let pos = pos.min(rga1.len());

            // Concurrent inserts
            rga1.insert(pos, char1);
            rga2.insert(pos, char2);

            // Merge
            rga1.merge(&rga2);
            rga2.merge(&rga1);

            // Both should have same result
            prop_assert_eq!(rga1.to_vec(), rga2.to_vec());

            // Both characters should be present
            let result = rga1.to_vec();
            prop_assert!(result.contains(&char1));
            prop_assert!(result.contains(&char2));
        }

        /// Concurrent delete of same element converges
        #[test]
        fn concurrent_delete_converges(
            base in "[a-z]{1,20}",
            pos_factor in 0.0f64..1.0,
        ) {
            let peer1 = PeerId::new();
            let peer2 = PeerId::new();

            let mut rga1 = RGA::from_str(&base, peer1);
            let mut rga2 = rga1.clone();
            rga2.set_peer_id(peer2);

            let len = rga1.len();
            let pos = (pos_factor * len as f64).floor() as usize;
            let pos = pos.min(len - 1);

            // Both delete the same position
            rga1.delete(pos);
            rga2.delete(pos);

            // Merge
            rga1.merge(&rga2);
            rga2.merge(&rga1);

            // Both should converge
            prop_assert_eq!(rga1.to_vec(), rga2.to_vec());
            prop_assert_eq!(rga1.len(), len - 1);
        }

        /// Eventual consistency across multiple replicas
        #[test]
        fn eventual_consistency(
            base in "[a-z]{0,5}",
            ops in prop::collection::vec((0u8..3, any::<bool>(), char_strategy()), 1..15),
        ) {
            let peers = [PeerId::new(), PeerId::new(), PeerId::new()];
            let base_rga = RGA::from_str(&base, peers[0]);

            let mut rgas: Vec<RGA<char>> = (0..3).map(|i| {
                let mut r = base_rga.clone();
                r.set_peer_id(peers[i]);
                r
            }).collect();

            // Apply operations
            for (node_idx, is_insert, ch) in &ops {
                let idx = (*node_idx as usize) % 3;
                let len = rgas[idx].len();
                if *is_insert || len == 0 {
                    let pos = if len == 0 { 0 } else { len / 2 };
                    rgas[idx].insert(pos.min(len), *ch);
                } else {
                    let pos = len / 2;
                    rgas[idx].delete(pos.min(len - 1));
                }
            }

            // Full sync
            for i in 0..3 {
                for j in 0..3 {
                    if i != j {
                        let other = rgas[j].clone();
                        rgas[i].merge(&other);
                    }
                }
            }

            // All should converge
            prop_assert_eq!(rgas[0].to_vec(), rgas[1].to_vec());
            prop_assert_eq!(rgas[1].to_vec(), rgas[2].to_vec());
        }
    }
}

// =============================================================================
// PN-COUNTER PROPERTY TESTS
// =============================================================================

mod pn_counter_properties {
    use super::*;

    proptest! {
        /// Commutativity: merge(A, B) == merge(B, A)
        #[test]
        fn merge_is_commutative(
            ops1 in prop::collection::vec((peer_id_strategy(), any::<bool>(), 1u64..100), 1..10),
            ops2 in prop::collection::vec((peer_id_strategy(), any::<bool>(), 1u64..100), 1..10),
        ) {
            let mut c1 = PNCounter::new();
            let mut c2 = PNCounter::new();

            for (peer, is_inc, amount) in &ops1 {
                if *is_inc { c1.increment(*peer, *amount); }
                else { c1.decrement(*peer, *amount); }
            }
            for (peer, is_inc, amount) in &ops2 {
                if *is_inc { c2.increment(*peer, *amount); }
                else { c2.decrement(*peer, *amount); }
            }

            let merged_12 = c1.merged(&c2);
            let merged_21 = c2.merged(&c1);

            prop_assert_eq!(merged_12.value(), merged_21.value());
            prop_assert_eq!(&merged_12, &merged_21);
        }

        /// Associativity: merge(merge(A, B), C) == merge(A, merge(B, C))
        #[test]
        fn merge_is_associative(
            ops1 in prop::collection::vec((peer_id_strategy(), 1u64..50), 1..5),
            ops2 in prop::collection::vec((peer_id_strategy(), 1u64..50), 1..5),
            ops3 in prop::collection::vec((peer_id_strategy(), 1u64..50), 1..5),
        ) {
            let mut c1 = PNCounter::new();
            let mut c2 = PNCounter::new();
            let mut c3 = PNCounter::new();

            for (peer, amount) in &ops1 { c1.increment(*peer, *amount); }
            for (peer, amount) in &ops2 { c2.decrement(*peer, *amount); }
            for (peer, amount) in &ops3 { c3.increment(*peer, *amount); }

            let left = c1.merged(&c2).merged(&c3);
            let right = c1.merged(&c2.merged(&c3));

            prop_assert_eq!(left, right);
        }

        /// Idempotence: merge(A, A) == A
        #[test]
        fn merge_is_idempotent(
            ops in prop::collection::vec((peer_id_strategy(), any::<bool>(), 1u64..100), 1..10),
        ) {
            let mut c = PNCounter::new();
            for (peer, is_inc, amount) in &ops {
                if *is_inc { c.increment(*peer, *amount); }
                else { c.decrement(*peer, *amount); }
            }

            let merged = c.merged(&c);
            prop_assert_eq!(c, merged);
        }

        /// Eventual consistency: all replicas converge after full sync
        #[test]
        fn eventual_consistency(
            ops in prop::collection::vec((0u8..3, any::<bool>(), 1u64..50), 1..20),
        ) {
            let peers = [PeerId::new(), PeerId::new(), PeerId::new()];
            let mut counters = [PNCounter::new(), PNCounter::new(), PNCounter::new()];

            for (node_idx, is_inc, amount) in &ops {
                let idx = (*node_idx as usize) % 3;
                if *is_inc {
                    counters[idx].increment(peers[idx], *amount);
                } else {
                    counters[idx].decrement(peers[idx], *amount);
                }
            }

            // Full sync
            for i in 0..3 {
                for j in 0..3 {
                    if i != j {
                        let other = counters[j].clone();
                        counters[i].merge(&other);
                    }
                }
            }

            prop_assert_eq!(counters[0].value(), counters[1].value());
            prop_assert_eq!(counters[1].value(), counters[2].value());
            prop_assert_eq!(&counters[0], &counters[1]);
            prop_assert_eq!(&counters[1], &counters[2]);
        }
    }
}

// =============================================================================
// CROSS-CRDT INTEGRATION TESTS
// =============================================================================

mod integration_tests {
    use super::*;

    proptest! {
        /// Document model: title (LWW) + tags (ORSet) + content (RGA) converge
        #[test]
        fn document_model_convergence(
            title1 in "[a-z ]{1,20}",
            title2 in "[a-z ]{1,20}",
            content1 in "[a-z ]{0,30}",
            content2 in "[a-z ]{0,30}",
            tags1 in prop::collection::vec("[a-z]{1,10}", 0..5),
            tags2 in prop::collection::vec("[a-z]{1,10}", 0..5),
        ) {
            let peer1 = PeerId::new();
            let peer2 = PeerId::new();

            // Peer 1's document
            let mut title_lww1 = LWWRegister::new(title1, peer1);
            let mut tags_set1: ORSet<String> = ORSet::new();
            for tag in &tags1 { tags_set1.add(tag.clone(), peer1); }
            let content_rga1 = RGA::from_str(&content1, peer1);

            // Peer 2's document (fork + modifications)
            let mut title_lww2 = title_lww1.clone();
            title_lww2.set(title2, peer2);
            let mut tags_set2 = tags_set1.clone();
            for tag in &tags2 { tags_set2.add(tag.clone(), peer2); }
            let mut content_rga2 = content_rga1.clone();
            content_rga2.set_peer_id(peer2);
            if !content2.is_empty() {
                content_rga2.insert_str(0, &content2);
            }

            // Merge all components
            title_lww1.merge(&title_lww2);
            tags_set1.merge(&tags_set2);
            let content_merged1 = content_rga1.merged(&content_rga2);

            title_lww2.merge(&title_lww1);
            tags_set2.merge(&tags_set1);
            let content_merged2 = content_rga2.merged(&content_rga1);

            // Both peers should converge to same state
            prop_assert_eq!(title_lww1.value(), title_lww2.value());

            let tags_1: HashSet<_> = tags_set1.iter().collect();
            let tags_2: HashSet<_> = tags_set2.iter().collect();
            prop_assert_eq!(tags_1, tags_2);

            prop_assert_eq!(content_merged1.to_vec(), content_merged2.to_vec());
        }
    }
}
