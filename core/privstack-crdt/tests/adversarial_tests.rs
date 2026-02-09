//! Adversarial and stress tests for CRDT correctness under hostile conditions.
//!
//! Covers three categories:
//! 1. Split-brain / partition recovery — peers diverge completely while offline
//! 2. High-contention concurrency — many peers racing with shuffled merge order
//! 3. Resurrection bugs — deletion conflicts and late-arriving operations

use privstack_crdt::{CausalOrder, LWWRegister, ORSet, PNCounter, VectorClock, RGA};
use privstack_types::{HybridTimestamp, PeerId};
use std::collections::HashSet;

/// Deterministic peer IDs for reproducibility.
fn peer(n: u8) -> PeerId {
    PeerId::from_uuid(uuid::Uuid::from_bytes([
        n, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ]))
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. SPLIT-BRAIN / PARTITION RECOVERY
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn rga_split_brain_delete_and_append() {
    // Two peers start with "Hello", diverge, then reconcile.
    // Peer A: deletes "ello", types " World" → "H World"
    // Peer B: appends " there" → "Hello there"
    let pa = PeerId::new();
    let pb = PeerId::new();

    let mut a = RGA::from_str("Hello", pa);
    let mut b = a.clone();
    b.set_peer_id(pb);

    // --- Partition ---

    // Peer A: delete indices 1..=4 ("ello"), then insert " World" after "H"
    a.delete_range(1, 4); // "H"
    a.insert_str(1, " World"); // "H World"
    assert_eq!(a.as_string(), "H World");

    // Peer B: append " there" at end
    b.insert_str(5, " there"); // "Hello there"
    assert_eq!(b.as_string(), "Hello there");

    // --- Reunion ---
    let mut merged_a = a.clone();
    let mut merged_b = b.clone();
    merged_a.merge(&b);
    merged_b.merge(&a);

    // Both must converge to the exact same string
    assert_eq!(merged_a.as_string(), merged_b.as_string());

    // "H" must appear exactly once (no duplicate from the shared origin)
    let result = merged_a.as_string();
    assert_eq!(result.matches('H').count(), 1, "Duplicate 'H' detected in: {result}");

    // The original "ello" must be gone (Peer A deleted it)
    assert!(!result.contains("ello"), "Deleted 'ello' reappeared in: {result}");

    // " World" and " there" must both be present
    assert!(result.contains("World"), "Missing 'World' in: {result}");
    assert!(result.contains("there"), "Missing 'there' in: {result}");
}

#[test]
fn rga_split_brain_overlapping_deletes() {
    // Both peers delete overlapping ranges during partition.
    // Start: "ABCDEF"
    // Peer A: deletes "BCD" → "AEF"
    // Peer B: deletes "CDE" → "ABF"
    // After merge: only "AF" should remain (union of deletes)
    let pa = PeerId::new();
    let pb = PeerId::new();

    let mut a = RGA::from_str("ABCDEF", pa);
    let mut b = a.clone();
    b.set_peer_id(pb);

    a.delete_range(1, 3); // delete B, C, D → "AEF"
    assert_eq!(a.as_string(), "AEF");

    b.delete_range(2, 3); // delete C, D, E → "ABF"
    assert_eq!(b.as_string(), "ABF");

    a.merge(&b);
    b.merge(&a);

    assert_eq!(a.as_string(), b.as_string());
    // B, C, D, E all deleted by at least one peer
    assert_eq!(a.as_string(), "AF");
}

#[test]
fn rga_split_brain_concurrent_inserts_at_same_position() {
    // Both peers insert different text at the same position during partition.
    // Start: "AC"
    // Peer A inserts "B" at index 1
    // Peer B inserts "X" at index 1
    // After merge: both characters present, deterministic order, both converge.
    let pa = PeerId::new();
    let pb = PeerId::new();

    let mut a = RGA::from_str("AC", pa);
    let mut b = a.clone();
    b.set_peer_id(pb);

    a.insert(1, 'B');
    b.insert(1, 'X');

    a.merge(&b);
    b.merge(&a);

    let result_a = a.as_string();
    let result_b = b.as_string();
    assert_eq!(result_a, result_b, "Replicas diverged!");

    assert!(result_a.contains('B') && result_a.contains('X'));
    assert!(result_a.starts_with('A') && result_a.ends_with('C'));
    assert_eq!(result_a.len(), 4);
}

#[test]
fn rga_three_way_partition_and_cascading_merge() {
    // Three peers diverge, then merge in a chain: A←B, then A←C.
    // Verifies that transitive merge produces full convergence.
    let pa = PeerId::new();
    let pb = PeerId::new();
    let pc = PeerId::new();

    let mut a = RGA::from_str("Base", pa);
    let mut b = a.clone();
    b.set_peer_id(pb);
    let mut c = a.clone();
    c.set_peer_id(pc);

    // All three diverge
    a.insert_str(4, " Alpha");   // "Base Alpha"
    b.insert_str(0, "Pre ");     // "Pre Base"
    c.delete_range(0, 4);
    c.insert_str(0, "New");      // "New"

    // Chain merge: A merges B, then A merges C
    a.merge(&b);
    a.merge(&c);

    // Now B and C merge with the fully-merged A
    b.merge(&a);
    c.merge(&a);

    assert_eq!(a.as_string(), b.as_string());
    assert_eq!(b.as_string(), c.as_string());
}

#[test]
fn rga_split_brain_deep_edit_chain() {
    // One peer makes many sequential edits while offline.
    // Start: "Hello"
    // Peer A: types " World" char by char (5 sequential inserts)
    // Peer B: types "!!!" char by char at the end
    // After merge: both additions present, no interleaving within a run.
    let pa = PeerId::new();
    let pb = PeerId::new();

    let mut a = RGA::from_str("Hello", pa);
    let mut b = a.clone();
    b.set_peer_id(pb);

    // Peer A: sequential inserts
    for (i, c) in " World".chars().enumerate() {
        a.insert(5 + i, c);
    }
    assert_eq!(a.as_string(), "Hello World");

    // Peer B: sequential inserts at end
    for (i, c) in "!!!".chars().enumerate() {
        b.insert(5 + i, c);
    }
    assert_eq!(b.as_string(), "Hello!!!");

    a.merge(&b);
    b.merge(&a);

    let result = a.as_string();
    assert_eq!(result, b.as_string());

    // Both runs must be intact (no interleaving within a single peer's run)
    assert!(result.contains("World"), "Missing 'World' in: {result}");
    assert!(result.contains("!!!"), "Missing '!!!' in: {result}");
    assert!(result.starts_with("Hello"));
}

#[test]
fn rga_split_brain_delete_and_reinsert_at_same_spot() {
    // Peer A deletes a character, Peer B inserts at the same position.
    // Start: "ABC"
    // Peer A: delete 'B' → "AC"
    // Peer B: insert 'X' after 'A' (before 'B') → "AXBC"
    // After merge: 'B' is deleted, 'X' survives → "AXC"
    let pa = PeerId::new();
    let pb = PeerId::new();

    let mut a = RGA::from_str("ABC", pa);
    let mut b = a.clone();
    b.set_peer_id(pb);

    a.delete(1); // delete 'B' → "AC"
    b.insert(1, 'X'); // insert 'X' at index 1 → "AXBC"

    a.merge(&b);
    b.merge(&a);

    assert_eq!(a.as_string(), b.as_string());
    // 'X' should survive, 'B' should be deleted
    let result = a.as_string();
    assert!(result.contains('X'));
    assert!(!result.contains('B'), "'B' should be deleted in: {result}");
    assert_eq!(result, "AXC");
}

#[test]
fn rga_partition_with_complete_document_rewrite() {
    // Both peers independently rewrite the entire document.
    // Start: "OLD"
    // Peer A: delete all, write "NEW_A"
    // Peer B: delete all, write "NEW_B"
    // After merge: all original chars deleted, both new texts present.
    let pa = PeerId::new();
    let pb = PeerId::new();

    let mut a = RGA::from_str("OLD", pa);
    let mut b = a.clone();
    b.set_peer_id(pb);

    a.delete_range(0, 3);
    a.insert_str(0, "NEW_A");
    assert_eq!(a.as_string(), "NEW_A");

    b.delete_range(0, 3);
    b.insert_str(0, "NEW_B");
    assert_eq!(b.as_string(), "NEW_B");

    a.merge(&b);
    b.merge(&a);

    let result = a.as_string();
    assert_eq!(result, b.as_string(), "Replicas diverged!");
    // Original text gone
    assert!(!result.contains("OLD"));
    // Both rewrites present
    assert!(result.contains("NEW_A"), "Missing 'NEW_A' in: {result}");
    assert!(result.contains("NEW_B"), "Missing 'NEW_B' in: {result}");
}

#[test]
fn orset_split_brain_add_remove_divergence() {
    // Peer A adds items, Peer B removes items during partition.
    let pa = PeerId::new();
    let _pb = PeerId::new();

    let mut shared: ORSet<String> = ORSet::new();
    shared.add("alpha".into(), pa);
    shared.add("beta".into(), pa);
    shared.add("gamma".into(), pa);

    let mut a = shared.clone();
    let mut b = shared.clone();

    // Partition: A adds new items
    a.add("delta".into(), pa);
    a.add("epsilon".into(), pa);

    // Partition: B removes existing items
    b.remove(&"alpha".into());
    b.remove(&"beta".into());

    a.merge(&b);
    b.merge(&a);

    // Both converge
    let items_a: HashSet<_> = a.iter().cloned().collect();
    let items_b: HashSet<_> = b.iter().cloned().collect();
    assert_eq!(items_a, items_b);

    // "alpha" and "beta" removed by B (B observed them before removing)
    assert!(!a.contains(&"alpha".into()));
    assert!(!a.contains(&"beta".into()));
    // "gamma" untouched
    assert!(a.contains(&"gamma".into()));
    // A's additions survive
    assert!(a.contains(&"delta".into()));
    assert!(a.contains(&"epsilon".into()));
}

#[test]
fn lww_register_split_brain_both_write() {
    // Both peers write to the same register during partition.
    // Higher timestamp wins; on tie, higher peer ID wins.
    let pa = PeerId::new();
    let pb = PeerId::new();

    let ts_base = HybridTimestamp::new(1000, 0);
    let mut a = LWWRegister::with_timestamp("initial".to_string(), ts_base, pa);
    let mut b = a.clone();

    // Both write at different times
    let ts_a = HybridTimestamp::new(2000, 0);
    a.set_with_timestamp("from_A".into(), ts_a, pa);

    let ts_b = HybridTimestamp::new(3000, 0);
    b.set_with_timestamp("from_B".into(), ts_b, pb);

    let merged_ab = a.merged(&b);
    let merged_ba = b.merged(&a);

    // Commutative: same result either way
    assert_eq!(merged_ab.value(), merged_ba.value());
    // Higher timestamp wins
    assert_eq!(*merged_ab.value(), "from_B");
}

#[test]
fn lww_register_split_brain_same_timestamp_tiebreak() {
    // Both peers write at the exact same timestamp — peer ID tiebreak.
    let pa = peer(1);
    let pb = peer(2);
    let ts = HybridTimestamp::new(5000, 0);

    let a = LWWRegister::with_timestamp("A_value".to_string(), ts, pa);
    let b = LWWRegister::with_timestamp("B_value".to_string(), ts, pb);

    let merged_ab = a.merged(&b);
    let merged_ba = b.merged(&a);

    assert_eq!(merged_ab.value(), merged_ba.value());
    // Higher peer ID wins (peer(2) > peer(1) by UUID comparison)
    assert_eq!(*merged_ab.value(), "B_value");
}

#[test]
fn pncounter_split_brain_independent_operations() {
    // Two peers independently increment/decrement during partition.
    let pa = PeerId::new();
    let pb = PeerId::new();

    let mut a = PNCounter::new();
    let mut b = PNCounter::new();

    // Peer A: net +7
    a.increment(pa, 10);
    a.decrement(pa, 3);

    // Peer B: net -2
    b.increment(pb, 5);
    b.decrement(pb, 7);

    a.merge(&b);
    b.merge(&a);

    assert_eq!(a, b);
    assert_eq!(a.value(), 5); // (10 - 3) + (5 - 7) = 7 + (-2) = 5
}

#[test]
fn vector_clock_split_brain_causality_detection() {
    // After partition, clocks must be concurrent (neither before/after).
    let pa = PeerId::new();
    let pb = PeerId::new();

    let mut a = VectorClock::new();
    let mut b = VectorClock::new();

    // Both start synced
    a.increment(pa);
    b.merge(&a);

    // Partition: each advances independently
    a.increment(pa);
    a.increment(pa);
    b.increment(pb);
    b.increment(pb);
    b.increment(pb);

    assert_eq!(a.compare(&b), CausalOrder::Concurrent);
    assert_eq!(b.compare(&a), CausalOrder::Concurrent);

    // Merge restores dominance
    let mut merged = a.clone();
    merged.merge(&b);
    assert!(merged.dominates(&a));
    assert!(merged.dominates(&b));
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. HIGH-CONTENTION / CONCURRENT STRESS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn lww_register_10_peers_100_writes_shuffled_merge() {
    // 10 peers each write 100 values. Merge in multiple random orders.
    // Final value must be deterministic regardless of merge order.
    let peers: Vec<PeerId> = (0..10).map(|_| PeerId::new()).collect();
    let mut registers: Vec<LWWRegister<String>> = Vec::new();

    for (i, &p) in peers.iter().enumerate() {
        let mut reg = LWWRegister::with_timestamp(
            format!("peer{i}_init"),
            HybridTimestamp::new(1, 0),
            p,
        );
        for j in 0..100 {
            reg.set(format!("peer{i}_write{j}"), p);
        }
        registers.push(reg);
    }

    // Merge order 1: left-to-right fold
    let mut merged_lr = registers[0].clone();
    for reg in &registers[1..] {
        merged_lr.merge(reg);
    }

    // Merge order 2: right-to-left fold
    let mut merged_rl = registers[9].clone();
    for reg in registers[..9].iter().rev() {
        merged_rl.merge(reg);
    }

    // Merge order 3: pairwise tree reduction
    let mut layer = registers.clone();
    while layer.len() > 1 {
        let mut next = Vec::new();
        let mut i = 0;
        while i + 1 < layer.len() {
            next.push(layer[i].merged(&layer[i + 1]));
            i += 2;
        }
        if i < layer.len() {
            next.push(layer[i].clone());
        }
        layer = next;
    }
    let merged_tree = layer.into_iter().next().unwrap();

    // All three merge orders must produce the same value
    assert_eq!(merged_lr.value(), merged_rl.value());
    assert_eq!(merged_rl.value(), merged_tree.value());
}

#[test]
fn orset_10_peers_concurrent_add_remove_shuffled_merge() {
    // 10 peers concurrently add/remove items. Merge order must not matter.
    let peers: Vec<PeerId> = (0..10).map(|_| PeerId::new()).collect();
    let mut sets: Vec<ORSet<String>> = Vec::new();

    for (i, &p) in peers.iter().enumerate() {
        let mut set = ORSet::new();
        // Each peer adds 20 items
        for j in 0..20 {
            set.add(format!("item_{j}"), p);
        }
        // Even-numbered peers remove half
        if i % 2 == 0 {
            for j in 0..10 {
                set.remove(&format!("item_{j}"));
            }
        }
        sets.push(set);
    }

    // Merge order 1: sequential
    let mut merged_seq = sets[0].clone();
    for s in &sets[1..] {
        merged_seq.merge(s);
    }

    // Merge order 2: reverse
    let mut merged_rev = sets[9].clone();
    for s in sets[..9].iter().rev() {
        merged_rev.merge(s);
    }

    // Merge order 3: tree
    let mut layer = sets.clone();
    while layer.len() > 1 {
        let mut next = Vec::new();
        let mut i = 0;
        while i + 1 < layer.len() {
            next.push(layer[i].merged(&layer[i + 1]));
            i += 2;
        }
        if i < layer.len() {
            next.push(layer[i].clone());
        }
        layer = next;
    }
    let merged_tree = layer.into_iter().next().unwrap();

    let items_seq: HashSet<_> = merged_seq.iter().cloned().collect();
    let items_rev: HashSet<_> = merged_rev.iter().cloned().collect();
    let items_tree: HashSet<_> = merged_tree.iter().cloned().collect();

    assert_eq!(items_seq, items_rev);
    assert_eq!(items_rev, items_tree);
}

#[test]
fn pncounter_10_peers_rapid_increments_merge_determinism() {
    // 10 peers each do 100 increments and 50 decrements.
    // Merge order must not affect final value.
    let peers: Vec<PeerId> = (0..10).map(|_| PeerId::new()).collect();
    let mut counters: Vec<PNCounter> = Vec::new();

    for &p in &peers {
        let mut c = PNCounter::new();
        for _ in 0..100 {
            c.increment(p, 1);
        }
        for _ in 0..50 {
            c.decrement(p, 1);
        }
        counters.push(c);
    }

    let mut merged_lr = counters[0].clone();
    for c in &counters[1..] {
        merged_lr.merge(c);
    }

    let mut merged_rl = counters[9].clone();
    for c in counters[..9].iter().rev() {
        merged_rl.merge(c);
    }

    assert_eq!(merged_lr, merged_rl);
    // 10 peers * (100 - 50) = 500
    assert_eq!(merged_lr.value(), 500);
}

#[test]
fn vector_clock_many_peers_concurrent_increments() {
    // 20 peers each increment independently 50 times.
    // After merge, clock must track all peers.
    let peers: Vec<PeerId> = (0..20).map(|_| PeerId::new()).collect();
    let mut clocks: Vec<VectorClock> = Vec::new();

    for &p in &peers {
        let mut vc = VectorClock::new();
        for _ in 0..50 {
            vc.increment(p);
        }
        clocks.push(vc);
    }

    // All pairs must be concurrent before merge
    for i in 0..clocks.len() {
        for j in (i + 1)..clocks.len() {
            assert_eq!(
                clocks[i].compare(&clocks[j]),
                CausalOrder::Concurrent,
                "Clocks {i} and {j} should be concurrent before merge"
            );
        }
    }

    let mut merged = VectorClock::new();
    for vc in &clocks {
        merged.merge(vc);
    }

    // Merged clock must dominate all individuals
    for (i, vc) in clocks.iter().enumerate() {
        assert!(
            merged.dominates(vc),
            "Merged clock doesn't dominate clock {i}"
        );
    }

    // Merged clock must have all 20 peers at time 50
    for &p in &peers {
        assert_eq!(merged.get(&p), 50);
    }
}

#[test]
fn rga_5_peers_concurrent_inserts_at_same_position() {
    // 5 peers all insert a different character at position 0 of an empty RGA.
    // All must converge to the same 5-char string regardless of merge order.
    let chars = ['A', 'B', 'C', 'D', 'E'];
    let peers: Vec<PeerId> = (0..5).map(|_| PeerId::new()).collect();
    let mut rgas: Vec<RGA<char>> = Vec::new();

    for (i, &p) in peers.iter().enumerate() {
        let mut rga = RGA::new(p);
        rga.insert(0, chars[i]);
        rgas.push(rga);
    }

    // Merge all into each replica
    let snapshots: Vec<_> = rgas.iter().cloned().collect();
    for rga in &mut rgas {
        for snap in &snapshots {
            rga.merge(snap);
        }
    }

    // All must converge
    let first = rgas[0].as_string();
    for (i, rga) in rgas.iter().enumerate() {
        assert_eq!(rga.as_string(), first, "Replica {i} diverged");
    }

    // Must contain all 5 characters
    assert_eq!(first.len(), 5);
    for c in &chars {
        assert!(first.contains(*c), "Missing '{c}' in: {first}");
    }
}

#[test]
fn lww_register_exact_same_timestamp_different_values_all_peers() {
    // All peers write at the exact same HybridTimestamp.
    // Deterministic tiebreak by peer ID must pick one winner consistently.
    let ts = HybridTimestamp::new(9999, 0);
    let mut registers: Vec<LWWRegister<String>> = Vec::new();

    for i in 0u8..10 {
        let p = peer(i);
        registers.push(LWWRegister::with_timestamp(
            format!("value_{i}"),
            ts,
            p,
        ));
    }

    // Merge in different orders
    let mut merged_lr = registers[0].clone();
    for r in &registers[1..] {
        merged_lr.merge(r);
    }

    let mut merged_rl = registers[9].clone();
    for r in registers[..9].iter().rev() {
        merged_rl.merge(r);
    }

    assert_eq!(merged_lr.value(), merged_rl.value());

    // Winner must be the peer with highest UUID (peer(9))
    assert_eq!(*merged_lr.value(), "value_9");
}

#[test]
fn rga_interleaved_insert_delete_stress() {
    // Two peers rapidly alternate inserting and deleting across a shared document.
    // Ensures tombstone tracking doesn't corrupt the visible sequence.
    let pa = PeerId::new();
    let pb = PeerId::new();

    let mut a = RGA::from_str("0123456789", pa);
    let mut b = a.clone();
    b.set_peer_id(pb);

    // Peer A: delete even indices, insert 'A' at start repeatedly
    for _ in 0..5 {
        if a.len() > 1 {
            a.delete(0);
        }
        a.insert(0, 'A');
    }

    // Peer B: delete odd indices, insert 'B' at end repeatedly
    for _ in 0..5 {
        let len = b.len();
        if len > 1 {
            b.delete(len - 1);
        }
        b.insert(b.len(), 'B');
    }

    a.merge(&b);
    b.merge(&a);

    assert_eq!(a.as_string(), b.as_string());
    // Verify no empty/corrupt state
    assert!(!a.as_string().is_empty());
}

#[test]
fn orset_same_element_added_by_all_peers_one_removes() {
    // 10 peers all add the same element. One peer removes it.
    // Add-wins: the element should still be present because other peers'
    // tags were not observed by the remover.
    let peers: Vec<PeerId> = (0..10).map(|_| PeerId::new()).collect();

    let mut sets: Vec<ORSet<String>> = Vec::new();
    for &p in &peers {
        let mut s = ORSet::new();
        s.add("shared_item".into(), p);
        sets.push(s);
    }

    // Merge all into a single set, then one peer removes
    let mut combined = sets[0].clone();
    for s in &sets[1..] {
        combined.merge(s);
    }
    assert!(combined.contains(&"shared_item".into()));

    // Peer 0 removes after seeing all tags
    combined.remove(&"shared_item".into());
    assert!(!combined.contains(&"shared_item".into()));

    // But if peer 5 adds AGAIN after the remove (new tag),
    // and we merge, it should reappear
    let mut peer5_set = sets[5].clone();
    peer5_set.add("shared_item".into(), peers[5]);

    combined.merge(&peer5_set);
    // The new tag from peer5 was not in the remove's tombstones
    assert!(
        combined.contains(&"shared_item".into()),
        "Element should reappear after fresh add"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. RESURRECTION BUGS / DELETION CONFLICTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn orset_resurrection_classic_scenario() {
    // Classic resurrection test:
    // 1. Peer A adds "Item1"
    // 2. A syncs with B
    // 3. B removes "Item1"
    // 4. A (offline) adds "Item1" again (new tag)
    // 5. Sync
    // Result: "Item1" must be present (new add's tag wasn't in B's remove)
    let pa = PeerId::new();
    let _pb = PeerId::new();

    let mut a: ORSet<String> = ORSet::new();
    let _tag1 = a.add("Item1".to_string(), pa);

    // Sync A → B
    let mut b = a.clone();

    // B removes Item1 (observes tag1)
    b.remove(&"Item1".into());
    assert!(!b.contains(&"Item1".into()));

    // A adds Item1 again (creates tag2, different from tag1)
    let _tag2 = a.add("Item1".to_string(), pa);
    assert!(a.contains(&"Item1".into()));

    // Sync both directions
    a.merge(&b);
    b.merge(&a);

    // tag1 is tombstoned by B, but tag2 is not → Item1 present
    assert!(a.contains(&"Item1".into()), "Item1 should survive resurrection");
    assert!(b.contains(&"Item1".into()), "Item1 should survive on B too");

    // Both converge
    let items_a: HashSet<_> = a.iter().cloned().collect();
    let items_b: HashSet<_> = b.iter().cloned().collect();
    assert_eq!(items_a, items_b);
}

#[test]
fn orset_stale_add_does_not_override_delete() {
    // The OPPOSITE of resurrection: a late-arriving COPY of the original add
    // must NOT override the delete.
    // 1. Peer A adds "X" (tag1)
    // 2. A syncs with B
    // 3. B removes "X" (tombstones tag1)
    // 4. A stale replica (snapshot from step 1) merges with B
    // Result: "X" must NOT be present (tag1 was already tombstoned)
    let pa = PeerId::new();

    let mut a = ORSet::new();
    let _tag1 = a.add("X".to_string(), pa);

    // Snapshot before sync
    let a_stale = a.clone();

    // Sync A → B
    let mut b = a.clone();

    // B removes X
    b.remove(&"X".into());
    assert!(!b.contains(&"X".into()));

    // Now merge the STALE copy of A into B
    b.merge(&a_stale);

    // X must NOT reappear — the stale copy only has tag1, which B already tombstoned
    assert!(
        !b.contains(&"X".into()),
        "Stale add should not override delete"
    );
}

#[test]
fn orset_concurrent_add_and_remove_add_wins() {
    // True concurrency: Peer A removes, Peer B adds the same element
    // simultaneously (neither has seen the other's operation).
    // Add-wins semantics: element should be present after merge.
    let pa = PeerId::new();
    let pb = PeerId::new();

    let mut shared: ORSet<String> = ORSet::new();
    shared.add("conflict".into(), pa);

    let mut a = shared.clone();
    let mut b = shared.clone();

    // Concurrent: A removes, B adds with new tag
    a.remove(&"conflict".into());
    let _new_tag = b.add("conflict".into(), pb);

    a.merge(&b);
    b.merge(&a);

    // B's new tag was not in A's remove tombstones → element present
    assert!(a.contains(&"conflict".into()));
    assert!(b.contains(&"conflict".into()));

    let items_a: HashSet<_> = a.iter().cloned().collect();
    let items_b: HashSet<_> = b.iter().cloned().collect();
    assert_eq!(items_a, items_b);
}

#[test]
fn orset_remove_then_independent_readd_by_third_peer() {
    // Three-peer resurrection:
    // 1. A adds "doc", syncs to B and C
    // 2. B removes "doc"
    // 3. C (offline) adds "doc" again
    // 4. All three merge
    // Result: "doc" present (C's new tag not in B's tombstones)
    let pa = PeerId::new();
    let _pb = PeerId::new();
    let pc = PeerId::new();

    let mut a: ORSet<String> = ORSet::new();
    a.add("doc".into(), pa);

    let mut b = a.clone();
    let mut c = a.clone();

    // B removes
    b.remove(&"doc".into());

    // C adds again (new tag)
    c.add("doc".into(), pc);

    // Full sync
    let b_snap = b.clone();
    let c_snap = c.clone();
    a.merge(&b_snap);
    a.merge(&c_snap);
    b.merge(&a);
    c.merge(&a);

    assert!(a.contains(&"doc".into()));
    assert!(b.contains(&"doc".into()));
    assert!(c.contains(&"doc".into()));

    let items_a: HashSet<_> = a.iter().cloned().collect();
    let items_b: HashSet<_> = b.iter().cloned().collect();
    let items_c: HashSet<_> = c.iter().cloned().collect();
    assert_eq!(items_a, items_b);
    assert_eq!(items_b, items_c);
}

#[test]
fn orset_cascading_add_remove_add_remove() {
    // Rapid add/remove/add/remove cycle across two peers.
    // Tests that tombstone accumulation doesn't corrupt state.
    let pa = PeerId::new();
    let pb = PeerId::new();

    let mut a: ORSet<String> = ORSet::new();
    let mut b: ORSet<String> = ORSet::new();

    // Round 1: A adds, sync, B removes
    a.add("item".into(), pa);
    b.merge(&a);
    b.remove(&"item".into());
    a.merge(&b);
    assert!(!a.contains(&"item".into()));

    // Round 2: B adds, sync, A removes
    b.add("item".into(), pb);
    a.merge(&b);
    a.remove(&"item".into());
    b.merge(&a);
    assert!(!b.contains(&"item".into()));

    // Round 3: A adds fresh — must work despite accumulated tombstones
    a.add("item".into(), pa);
    b.merge(&a);
    assert!(a.contains(&"item".into()));
    assert!(b.contains(&"item".into()));
}

#[test]
fn orset_multiple_peers_remove_same_element_then_one_readds() {
    // 5 peers all observe and remove the same element.
    // Then one peer re-adds it.
    // The re-add must survive because its tag is fresh.
    let peers: Vec<PeerId> = (0..5).map(|_| PeerId::new()).collect();

    let mut origin = ORSet::new();
    origin.add("target".into(), peers[0]);

    // All peers get a copy and remove it
    let mut replicas: Vec<ORSet<String>> = (0..5).map(|_| origin.clone()).collect();
    for replica in &mut replicas {
        replica.remove(&"target".into());
    }

    // Merge all removes together
    let mut merged = replicas[0].clone();
    for r in &replicas[1..] {
        merged.merge(r);
    }
    assert!(!merged.contains(&"target".into()));

    // Peer 3 re-adds with a fresh tag
    let mut peer3 = merged.clone();
    peer3.add("target".into(), peers[3]);

    merged.merge(&peer3);
    assert!(
        merged.contains(&"target".into()),
        "Fresh re-add should survive after mass deletion"
    );
}

#[test]
fn rga_delete_then_merge_preserves_tombstones() {
    // Verifies that a deleted character doesn't reappear when merging with
    // a stale replica that still has it.
    let pa = PeerId::new();

    let mut a = RGA::from_str("XYZ", pa);
    let stale = a.clone();

    // A deletes 'Y'
    a.delete(1); // "XZ"
    assert_eq!(a.as_string(), "XZ");

    // Merge stale (has 'Y') back into A
    a.merge(&stale);

    // 'Y' must stay deleted — delete-wins in RGA
    assert_eq!(a.as_string(), "XZ");
}

#[test]
fn rga_concurrent_delete_same_character() {
    // Both peers delete the same character. Must not cause double-tombstone issues.
    let pa = PeerId::new();
    let pb = PeerId::new();

    let mut a = RGA::from_str("ABC", pa);
    let mut b = a.clone();
    b.set_peer_id(pb);

    // Both delete 'B'
    a.delete(1);
    b.delete(1);

    a.merge(&b);
    b.merge(&a);

    assert_eq!(a.as_string(), b.as_string());
    assert_eq!(a.as_string(), "AC");
    assert_eq!(a.len(), 2);
}

#[test]
fn lww_register_stale_write_does_not_resurrect_old_value() {
    // A stale peer tries to set an old value with an old timestamp.
    // It must not override the newer value.
    let pa = PeerId::new();
    let pb = PeerId::new();

    let old_ts = HybridTimestamp::new(1000, 0);
    let new_ts = HybridTimestamp::new(5000, 0);

    let mut current = LWWRegister::with_timestamp("new_value".to_string(), new_ts, pa);
    let stale = LWWRegister::with_timestamp("old_value".to_string(), old_ts, pb);

    current.merge(&stale);

    assert_eq!(*current.value(), "new_value");
}

#[test]
fn pncounter_resurrection_via_stale_merge() {
    // A stale counter snapshot merged in must not increase the value beyond
    // what the peer actually contributed.
    let pa = PeerId::new();

    let mut counter = PNCounter::new();
    counter.increment(pa, 10);

    // Take stale snapshot
    let stale = counter.clone();

    // Counter advances further
    counter.increment(pa, 5);
    assert_eq!(counter.value(), 15);

    // Merge stale (has pa=10, which is less than current pa=15)
    counter.merge(&stale);

    // Value must still be 15 — stale merge doesn't inflate
    assert_eq!(counter.value(), 15);
}

#[test]
fn vector_clock_stale_update_does_not_regress() {
    // A stale clock merged in must not decrease any peer's logical time.
    let pa = PeerId::new();
    let pb = PeerId::new();

    let mut current = VectorClock::new();
    current.increment(pa); // pa=1
    current.increment(pa); // pa=2
    current.increment(pb); // pb=1

    let stale = VectorClock::for_peer(pa); // pa=0

    current.merge(&stale);

    assert_eq!(current.get(&pa), 2, "pa should not regress");
    assert_eq!(current.get(&pb), 1, "pb should be unaffected");
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. CROSS-CRDT INTEGRATION UNDER ADVERSARIAL CONDITIONS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn full_entity_split_brain_scenario() {
    // Simulates a real entity (like a Note) with title (LWW), content (RGA),
    // and tags (ORSet) going through a split-brain partition.
    let pa = PeerId::new();
    let pb = PeerId::new();

    // Shared initial state
    let mut title_a = LWWRegister::new("Draft".to_string(), pa);
    let mut content_a = RGA::from_str("Hello world", pa);
    let mut tags_a: ORSet<String> = ORSet::new();
    tags_a.add("work".into(), pa);
    tags_a.add("draft".into(), pa);

    let mut title_b = title_a.clone();
    let mut content_b = content_a.clone();
    content_b.set_peer_id(pb);
    let mut tags_b = tags_a.clone();

    // --- Partition ---

    // Peer A: rename, extend content, add tag
    title_a.set("Final Report".into(), pa);
    content_a.insert_str(11, "! This is important.");
    tags_a.add("important".into(), pa);
    tags_a.remove(&"draft".into());

    // Peer B: different rename, different edit, different tags
    title_b.set("Meeting Notes".into(), pb);
    content_b.delete_range(5, 6); // delete " world"
    content_b.insert_str(5, " everyone");
    tags_b.add("meeting".into(), pb);

    // --- Reunion ---
    let title_a_snap = title_a.clone();
    let content_a_snap = content_a.clone();
    let tags_a_snap = tags_a.clone();

    title_a.merge(&title_b);
    title_b.merge(&title_a_snap);
    content_a.merge(&content_b);
    content_b.merge(&content_a_snap);
    tags_a.merge(&tags_b);
    tags_b.merge(&tags_a_snap);

    // Titles converge (LWW — one wins)
    assert_eq!(title_a.value(), title_b.value());

    // Content converges (RGA — both edits present)
    assert_eq!(content_a.as_string(), content_b.as_string());

    // Tags converge (ORSet — add-wins)
    let tags_a_set: HashSet<_> = tags_a.iter().cloned().collect();
    let tags_b_set: HashSet<_> = tags_b.iter().cloned().collect();
    assert_eq!(tags_a_set, tags_b_set);

    // "work" should still be present (nobody removed it on B)
    assert!(tags_a.contains(&"work".into()));
    // "important" and "meeting" added concurrently — both present
    assert!(tags_a.contains(&"important".into()));
    assert!(tags_a.contains(&"meeting".into()));
}

#[test]
fn full_entity_three_peer_cascading_sync() {
    // Three peers edit a document in a chain: A edits, B edits, C edits.
    // Then sync in reverse order: C→B, B→A, then A broadcasts.
    // All must converge.
    let pa = PeerId::new();
    let pb = PeerId::new();
    let pc = PeerId::new();

    let mut a_content = RGA::from_str("Start", pa);
    let mut a_tags: ORSet<String> = ORSet::new();
    a_tags.add("v1".into(), pa);

    let mut b_content = a_content.clone();
    b_content.set_peer_id(pb);
    let mut b_tags = a_tags.clone();

    let mut c_content = a_content.clone();
    c_content.set_peer_id(pc);
    let mut c_tags = a_tags.clone();

    // Independent edits
    a_content.insert_str(5, " from A");
    a_tags.add("edited_by_a".into(), pa);

    b_content.insert_str(5, " from B");
    b_tags.add("edited_by_b".into(), pb);
    b_tags.remove(&"v1".into());

    c_content.delete_range(0, 5);
    c_content.insert_str(0, "End");
    c_tags.add("edited_by_c".into(), pc);

    // Chain sync: C→B
    b_content.merge(&c_content);
    b_tags.merge(&c_tags);

    // B→A
    a_content.merge(&b_content);
    a_tags.merge(&b_tags);

    // A broadcasts back
    b_content.merge(&a_content);
    b_tags.merge(&a_tags);
    c_content.merge(&a_content);
    c_tags.merge(&a_tags);

    // All converge
    assert_eq!(a_content.as_string(), b_content.as_string());
    assert_eq!(b_content.as_string(), c_content.as_string());

    let a_tag_set: HashSet<_> = a_tags.iter().cloned().collect();
    let b_tag_set: HashSet<_> = b_tags.iter().cloned().collect();
    let c_tag_set: HashSet<_> = c_tags.iter().cloned().collect();
    assert_eq!(a_tag_set, b_tag_set);
    assert_eq!(b_tag_set, c_tag_set);

    // All edit markers present
    assert!(a_tags.contains(&"edited_by_a".into()));
    assert!(a_tags.contains(&"edited_by_b".into()));
    assert!(a_tags.contains(&"edited_by_c".into()));
}
