//! N-peer convergence tests for team/enterprise scenarios.
//!
//! These tests simulate realistic multi-peer topologies:
//! 1. Gossip-based selective sync (random peer pairs, not full mesh)
//! 2. Chain/transitive convergence (A→B→C achieves global convergence)
//! 3. Interleaved write+merge (ops happening during sync rounds)
//! 4. Partial state divergence (peers at different progress levels)
//! 5. Tombstone stress under high churn (1000+ add/remove cycles)
//! 6. Large team simulation (20-50 concurrent peers on shared data)

use privstack_crdt::{CausalOrder, LWWRegister, ORSet, PNCounter, RGA, VectorClock};
use privstack_types::{HybridTimestamp, PeerId};
use std::collections::HashSet;

/// Deterministic peer IDs for reproducibility.
fn peer(n: u8) -> PeerId {
    PeerId::from_uuid(uuid::Uuid::from_bytes([
        n, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ]))
}

/// Merge replica `src` into `dst` for all CRDT fields of a simulated entity.
fn merge_entity(
    dst_content: &mut RGA<char>,
    dst_tags: &mut ORSet<String>,
    dst_counter: &mut PNCounter,
    src_content: &RGA<char>,
    src_tags: &ORSet<String>,
    src_counter: &PNCounter,
) {
    dst_content.merge(src_content);
    dst_tags.merge(src_tags);
    dst_counter.merge(src_counter);
}

/// Assert that all replicas converge to the same state.
fn assert_all_converged(
    contents: &[RGA<char>],
    tags: &[ORSet<String>],
    counters: &[PNCounter],
) {
    let ref_str = contents[0].as_string();
    let ref_tags: HashSet<_> = tags[0].iter().cloned().collect();
    let ref_val = counters[0].value();

    for i in 1..contents.len() {
        assert_eq!(
            contents[i].as_string(),
            ref_str,
            "RGA diverged at replica {i}: got '{}', expected '{ref_str}'",
            contents[i].as_string()
        );
        let t: HashSet<_> = tags[i].iter().cloned().collect();
        assert_eq!(t, ref_tags, "ORSet diverged at replica {i}");
        assert_eq!(
            counters[i].value(),
            ref_val,
            "PNCounter diverged at replica {i}: got {}, expected {ref_val}",
            counters[i].value()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. GOSSIP / SELECTIVE SYNC — NOT FULL MESH
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn gossip_convergence_10_peers_random_pairs() {
    // 10 peers each write independently. Instead of full-mesh merge,
    // simulate gossip: each round, each peer merges with one random neighbor.
    // After enough rounds, all must converge.
    let n = 10;
    let peers: Vec<PeerId> = (0..n).map(|i| peer(i as u8)).collect();
    let mut rgas: Vec<RGA<char>> = peers.iter().map(|&p| RGA::new(p)).collect();

    // Each peer inserts its own character
    for (i, rga) in rgas.iter_mut().enumerate() {
        rga.insert(0, (b'A' + i as u8) as char);
    }

    // Deterministic "random" gossip: peer i merges with peer (i + round) % n
    for round in 1..=n {
        let snapshots: Vec<_> = rgas.iter().cloned().collect();
        for i in 0..n {
            let partner = (i + round) % n;
            rgas[i].merge(&snapshots[partner]);
        }
    }

    // All 10 replicas must have the same string containing all 10 characters
    let expected_chars: HashSet<char> = (0..n).map(|i| (b'A' + i as u8) as char).collect();
    let reference = rgas[0].as_string();

    for (i, rga) in rgas.iter().enumerate() {
        assert_eq!(rga.as_string(), reference, "Replica {i} diverged after gossip");
    }

    let actual_chars: HashSet<char> = reference.chars().collect();
    assert_eq!(actual_chars, expected_chars, "Not all characters present");
}

#[test]
fn gossip_orset_20_peers_selective_sync() {
    // 20 peers add items, some remove. Sync via ring gossip (each peer
    // only talks to its immediate neighbor). Verify eventual convergence.
    let n = 20;
    let peers: Vec<PeerId> = (0..n).map(|i| peer(i as u8)).collect();
    let mut sets: Vec<ORSet<String>> = (0..n).map(|_| ORSet::new()).collect();

    // Each peer adds a unique item + a shared item
    for (i, set) in sets.iter_mut().enumerate() {
        set.add(format!("unique_{i}"), peers[i]);
        set.add("shared".into(), peers[i]);
    }

    // Every 5th peer removes "shared" (before seeing others' tags)
    for i in (0..n).step_by(5) {
        sets[i].remove(&"shared".into());
    }

    // Ring gossip: peer i → peer (i+1) % n, repeat n times
    for _round in 0..n {
        let snapshots: Vec<_> = sets.iter().cloned().collect();
        for i in 0..n {
            sets[i].merge(&snapshots[(i + 1) % n]);
        }
    }

    let ref_items: HashSet<_> = sets[0].iter().cloned().collect();
    for (i, set) in sets.iter().enumerate() {
        let items: HashSet<_> = set.iter().cloned().collect();
        assert_eq!(items, ref_items, "ORSet replica {i} diverged");
    }

    // All unique items must be present
    for i in 0..n {
        assert!(
            sets[0].contains(&format!("unique_{i}")),
            "Missing unique_{i}"
        );
    }

    // "shared" must be present (add-wins: peers that didn't remove still have fresh tags)
    assert!(
        sets[0].contains(&"shared".into()),
        "shared item should survive partial removes due to add-wins"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. CHAIN / TRANSITIVE CONVERGENCE
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn chain_sync_10_peers_linear_propagation() {
    // 10 peers in a chain: 0→1→2→...→9, then 9→8→...→0.
    // Only adjacent peers sync. Two passes must achieve full convergence.
    let n = 10;
    let peers: Vec<PeerId> = (0..n).map(|i| peer(i as u8)).collect();
    let mut counters: Vec<PNCounter> = (0..n).map(|_| PNCounter::new()).collect();

    // Each peer increments by its index + 1
    for (i, c) in counters.iter_mut().enumerate() {
        c.increment(peers[i], (i + 1) as u64);
    }

    // Forward pass: 0→1, 1→2, ..., 8→9
    for i in 0..n - 1 {
        let snap = counters[i].clone();
        counters[i + 1].merge(&snap);
    }

    // Backward pass: 9→8, 8→7, ..., 1→0
    for i in (0..n - 1).rev() {
        let snap = counters[i + 1].clone();
        counters[i].merge(&snap);
    }

    // Expected: sum(1..=10) = 55
    let expected = (n * (n + 1) / 2) as i64;
    for (i, c) in counters.iter().enumerate() {
        assert_eq!(c.value(), expected, "Counter {i} has wrong value");
    }
}

#[test]
fn chain_sync_rga_5_peers_no_direct_link_between_extremes() {
    // Peers 0 and 4 never directly sync. They only converge through
    // intermediaries 1, 2, 3. Tests that transitive RGA merge works.
    let peers: Vec<PeerId> = (0..5).map(|i| peer(i)).collect();
    let mut rgas: Vec<RGA<char>> = peers.iter().map(|&p| RGA::new(p)).collect();

    // Each peer inserts at position 0
    for (i, rga) in rgas.iter_mut().enumerate() {
        rga.insert(0, (b'A' + i as u8) as char);
    }

    // Forward chain: 0→1, 1→2, 2→3, 3→4
    for i in 0..4 {
        let snap = rgas[i].clone();
        rgas[i + 1].merge(&snap);
    }

    // Backward chain: 4→3, 3→2, 2→1, 1→0
    for i in (0..4).rev() {
        let snap = rgas[i + 1].clone();
        rgas[i].merge(&snap);
    }

    let reference = rgas[0].as_string();
    for (i, rga) in rgas.iter().enumerate() {
        assert_eq!(rga.as_string(), reference, "Replica {i} diverged in chain");
    }

    assert_eq!(reference.len(), 5, "All 5 chars must be present");
}

#[test]
fn hub_and_spoke_20_peers_sync_through_central_hub() {
    // Enterprise pattern: 20 peers sync only through a central relay (peer 0).
    // No peer-to-peer direct links. Hub collects from all, then broadcasts.
    let n = 20;
    let peers: Vec<PeerId> = (0..n).map(|i| peer(i as u8)).collect();
    let mut sets: Vec<ORSet<String>> = (0..n).map(|_| ORSet::new()).collect();

    // Each peer adds its own tag
    for (i, set) in sets.iter_mut().enumerate() {
        set.add(format!("from_peer_{i}"), peers[i]);
    }

    // Phase 1: all spokes merge into hub (peer 0)
    for i in 1..n {
        let snap = sets[i].clone();
        sets[0].merge(&snap);
    }

    // Phase 2: hub broadcasts to all spokes
    let hub_snap = sets[0].clone();
    for set in sets.iter_mut().skip(1) {
        set.merge(&hub_snap);
    }

    // All must converge
    let ref_items: HashSet<_> = sets[0].iter().cloned().collect();
    for (i, set) in sets.iter().enumerate() {
        let items: HashSet<_> = set.iter().cloned().collect();
        assert_eq!(items, ref_items, "Spoke {i} diverged from hub");
    }

    assert_eq!(ref_items.len(), n, "Hub should have all {n} items");
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. INTERLEAVED WRITE + MERGE (OPS DURING SYNC)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn write_during_merge_rounds_rga_10_peers() {
    // 10 peers write new characters between merge rounds.
    // Simulates real-time editing while sync is happening.
    let n = 10;
    let peers: Vec<PeerId> = (0..n).map(|i| peer(i as u8)).collect();
    let mut rgas: Vec<RGA<char>> = peers.iter().map(|&p| RGA::new(p)).collect();

    for round in 0..5 {
        // Each peer writes a character
        for (i, rga) in rgas.iter_mut().enumerate() {
            let c = (b'A' + (round * n + i) as u8) as char;
            let pos = rga.len();
            rga.insert(pos, c);
        }

        // Gossip round: merge with neighbor
        let snapshots: Vec<_> = rgas.iter().cloned().collect();
        for i in 0..n {
            rgas[i].merge(&snapshots[(i + 1) % n]);
        }
    }

    // Final full-mesh merge to ensure total convergence
    let final_snaps: Vec<_> = rgas.iter().cloned().collect();
    for rga in &mut rgas {
        for snap in &final_snaps {
            rga.merge(snap);
        }
    }

    let reference = rgas[0].as_string();
    for (i, rga) in rgas.iter().enumerate() {
        assert_eq!(rga.as_string(), reference, "Replica {i} diverged after interleaved write+merge");
    }

    // 5 rounds × 10 peers = 50 unique characters
    assert_eq!(reference.len(), 50, "Expected 50 characters from 5 rounds of 10 peers");
}

#[test]
fn write_during_merge_rounds_orset_with_removes() {
    // Peers add/remove between merge rounds. Tests that tombstone tracking
    // stays correct when interleaved with partial syncs.
    let n = 10;
    let peers: Vec<PeerId> = (0..n).map(|i| peer(i as u8)).collect();
    let mut sets: Vec<ORSet<String>> = (0..n).map(|_| ORSet::new()).collect();

    for round in 0..5u32 {
        // Even peers add, odd peers remove previous round's item
        for (i, set) in sets.iter_mut().enumerate() {
            let item = format!("r{round}_p{i}");
            set.add(item, peers[i]);

            if round > 0 && i % 2 == 1 {
                let old_item = format!("r{}_p{i}", round - 1);
                set.remove(&old_item);
            }
        }

        // Partial sync: each peer merges with 2 neighbors
        let snaps: Vec<_> = sets.iter().cloned().collect();
        for i in 0..n {
            sets[i].merge(&snaps[(i + 1) % n]);
            sets[i].merge(&snaps[(i + 2) % n]);
        }
    }

    // Full convergence round
    let snaps: Vec<_> = sets.iter().cloned().collect();
    for set in &mut sets {
        for snap in &snaps {
            set.merge(snap);
        }
    }

    let ref_items: HashSet<_> = sets[0].iter().cloned().collect();
    for (i, set) in sets.iter().enumerate() {
        let items: HashSet<_> = set.iter().cloned().collect();
        assert_eq!(items, ref_items, "ORSet replica {i} diverged after interleaved ops");
    }
}

#[test]
fn pncounter_write_during_chain_sync() {
    // 10 peers increment between chain-sync passes.
    // After 3 full chain passes, values must converge.
    let n = 10;
    let peers: Vec<PeerId> = (0..n).map(|i| peer(i as u8)).collect();
    let mut counters: Vec<PNCounter> = (0..n).map(|_| PNCounter::new()).collect();

    let mut expected_total: i64 = 0;

    for round in 1u64..=3 {
        // Each peer increments
        for (i, c) in counters.iter_mut().enumerate() {
            let amount = (i as u64 + 1) * round;
            c.increment(peers[i], amount);
            expected_total += amount as i64;
        }

        // Forward chain pass
        for i in 0..n - 1 {
            let snap = counters[i].clone();
            counters[i + 1].merge(&snap);
        }
        // Backward chain pass
        for i in (0..n - 1).rev() {
            let snap = counters[i + 1].clone();
            counters[i].merge(&snap);
        }
    }

    for (i, c) in counters.iter().enumerate() {
        assert_eq!(
            c.value(),
            expected_total,
            "Counter {i} has wrong value after interleaved chain sync"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. PARTIAL STATE DIVERGENCE (PEERS AT DIFFERENT PROGRESS)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn staggered_joins_10_peers_arrive_at_different_times() {
    // Peers join one at a time. Each new peer syncs with the previous one
    // (who has accumulated state from all earlier peers).
    let n = 10;
    let peers: Vec<PeerId> = (0..n).map(|i| peer(i as u8)).collect();
    let mut sets: Vec<ORSet<String>> = Vec::new();

    for i in 0..n {
        // New peer creates its own set with a unique item
        let mut new_set = ORSet::new();
        new_set.add(format!("item_{i}"), peers[i]);

        // Sync with previous peer (who has accumulated state)
        if i > 0 {
            new_set.merge(&sets[i - 1]);
            // Previous peer also gets the new item
            let snap = new_set.clone();
            sets[i - 1].merge(&snap);
        }

        sets.push(new_set);
    }

    // At this point, only peer n-1 and n-2 are fully converged.
    // Do a backward pass to propagate to all.
    for i in (0..n - 1).rev() {
        let snap = sets[i + 1].clone();
        sets[i].merge(&snap);
    }

    let ref_items: HashSet<_> = sets[0].iter().cloned().collect();
    assert_eq!(ref_items.len(), n, "Should have all {n} items");

    for (i, set) in sets.iter().enumerate() {
        let items: HashSet<_> = set.iter().cloned().collect();
        assert_eq!(items, ref_items, "Set {i} missing items after staggered join");
    }
}

#[test]
fn late_joiner_catches_up_from_single_peer() {
    // 10 peers collaborate for many rounds. A new peer (11th) joins late
    // and syncs with just one existing peer. Must get full state.
    let n = 10;
    let peers: Vec<PeerId> = (0..n).map(|i| peer(i as u8)).collect();
    let late_peer = peer(99);

    let mut rgas: Vec<RGA<char>> = peers.iter().map(|&p| RGA::new(p)).collect();

    // Each peer writes, then full-mesh merge
    for (i, rga) in rgas.iter_mut().enumerate() {
        rga.insert(0, (b'A' + i as u8) as char);
    }
    let snaps: Vec<_> = rgas.iter().cloned().collect();
    for rga in &mut rgas {
        for snap in &snaps {
            rga.merge(snap);
        }
    }

    // Late joiner syncs with just peer 0
    let mut late_rga = RGA::new(late_peer);
    late_rga.merge(&rgas[0]);

    assert_eq!(
        late_rga.as_string(),
        rgas[0].as_string(),
        "Late joiner should have full state from single sync"
    );
    assert_eq!(late_rga.as_string().len(), n, "Should have all {n} characters");
}

#[test]
fn asymmetric_progress_some_peers_ahead() {
    // Peer 0 has ops 1-10, Peer 1 has ops 1-5, Peer 2 has ops 1-3.
    // After pairwise sync, all must converge to ops 1-10.
    let p0 = peer(0);
    let p1 = peer(1);
    let p2 = peer(2);

    let mut c0 = PNCounter::new();
    let mut c1 = PNCounter::new();
    let mut c2 = PNCounter::new();

    // p0 does 10 increments
    for _ in 0..10 {
        c0.increment(p0, 1);
    }
    // p1 does 5
    for _ in 0..5 {
        c1.increment(p1, 1);
    }
    // p2 does 3
    for _ in 0..3 {
        c2.increment(p2, 1);
    }

    // Also give each peer partial knowledge of others via stale snapshots
    // p1 saw p0 at count 3
    let mut stale_p0 = PNCounter::new();
    stale_p0.increment(p0, 3);
    c1.merge(&stale_p0);

    // p2 saw p0 at count 1, p1 at count 2
    let mut stale_p0_for_2 = PNCounter::new();
    stale_p0_for_2.increment(p0, 1);
    let mut stale_p1_for_2 = PNCounter::new();
    stale_p1_for_2.increment(p1, 2);
    c2.merge(&stale_p0_for_2);
    c2.merge(&stale_p1_for_2);

    // Now full sync
    let s0 = c0.clone();
    let s1 = c1.clone();
    let s2 = c2.clone();
    c0.merge(&s1);
    c0.merge(&s2);
    c1.merge(&s0);
    c1.merge(&s2);
    c2.merge(&s0);
    c2.merge(&s1);

    assert_eq!(c0.value(), 18); // 10 + 5 + 3
    assert_eq!(c0, c1);
    assert_eq!(c1, c2);
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. TOMBSTONE STRESS / HIGH CHURN
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn orset_1000_add_remove_cycles_two_peers() {
    // Stress test: 1000 add/remove cycles on an ORSet across 2 peers.
    // Verifies tombstone accumulation doesn't corrupt merge correctness.
    let pa = peer(1);
    let pb = peer(2);

    let mut a: ORSet<String> = ORSet::new();
    let mut b: ORSet<String> = ORSet::new();

    for cycle in 0..1000 {
        let item = format!("item_{}", cycle % 50); // reuse 50 item names
        if cycle % 3 == 0 {
            a.add(item.clone(), pa);
            b.merge(&a);
            b.remove(&item);
            a.merge(&b);
        } else {
            b.add(item.clone(), pb);
            a.merge(&b);
            a.remove(&item);
            b.merge(&a);
        }
    }

    // Final sync
    let sa = a.clone();
    let sb = b.clone();
    a.merge(&sb);
    b.merge(&sa);

    let items_a: HashSet<_> = a.iter().cloned().collect();
    let items_b: HashSet<_> = b.iter().cloned().collect();
    assert_eq!(items_a, items_b, "Diverged after 1000 cycles");
}

#[test]
fn orset_high_churn_10_peers_concurrent_add_remove_cycles() {
    // 10 peers each do 100 add/remove cycles on overlapping item names.
    let n = 10;
    let peers: Vec<PeerId> = (0..n).map(|i| peer(i as u8)).collect();
    let mut sets: Vec<ORSet<String>> = (0..n).map(|_| ORSet::new()).collect();

    for cycle in 0..100u32 {
        for (i, set) in sets.iter_mut().enumerate() {
            let item = format!("shared_{}", cycle % 20);
            set.add(item.clone(), peers[i]);
            if (cycle + i as u32) % 4 == 0 {
                set.remove(&item);
            }
        }

        // Periodic partial sync (every 10 cycles)
        if cycle % 10 == 9 {
            let snaps: Vec<_> = sets.iter().cloned().collect();
            for i in 0..n {
                sets[i].merge(&snaps[(i + 1) % n]);
            }
        }
    }

    // Full convergence
    let snaps: Vec<_> = sets.iter().cloned().collect();
    for set in &mut sets {
        for snap in &snaps {
            set.merge(snap);
        }
    }

    let ref_items: HashSet<_> = sets[0].iter().cloned().collect();
    for (i, set) in sets.iter().enumerate() {
        let items: HashSet<_> = set.iter().cloned().collect();
        assert_eq!(items, ref_items, "Replica {i} diverged after high churn");
    }
}

#[test]
fn rga_500_insert_delete_cycles_tombstone_correctness() {
    // 2 peers do 500 rounds of insert+delete on an RGA.
    // The visible text must always be consistent after merge.
    let pa = peer(1);
    let pb = peer(2);

    let mut a = RGA::from_str("seed", pa);
    let mut b = a.clone();
    b.set_peer_id(pb);

    for round in 0..500u32 {
        // Peer A inserts at start, then deletes at end
        let c = (b'a' + (round % 26) as u8) as char;
        a.insert(0, c);
        if a.len() > 10 {
            a.delete(a.len() - 1);
        }

        // Peer B inserts at end, then deletes at start
        let c = (b'A' + (round % 26) as u8) as char;
        let len = b.len();
        b.insert(len, c);
        if b.len() > 10 {
            b.delete(0);
        }

        // Periodic merge
        if round % 50 == 49 {
            let sa = a.clone();
            let sb = b.clone();
            a.merge(&sb);
            b.merge(&sa);
        }
    }

    // Final merge
    let sa = a.clone();
    let sb = b.clone();
    a.merge(&sb);
    b.merge(&sa);

    assert_eq!(
        a.as_string(),
        b.as_string(),
        "RGA diverged after 500 insert/delete cycles"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. LARGE TEAM / ENTERPRISE SIMULATION
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn enterprise_50_peers_shared_document() {
    // 50 team members collaborate on a shared document (RGA + ORSet tags).
    // Sync via hub-and-spoke with periodic direct peer merges.
    let n = 50;
    let peers: Vec<PeerId> = (0..n).map(|i| peer(i as u8)).collect();
    let hub = 0; // peer 0 is the relay

    let mut rgas: Vec<RGA<char>> = peers.iter().map(|&p| RGA::new(p)).collect();
    let mut tags: Vec<ORSet<String>> = (0..n).map(|_| ORSet::new()).collect();

    // Each peer writes a character and adds a tag
    for (i, (rga, tag)) in rgas.iter_mut().zip(tags.iter_mut()).enumerate() {
        rga.insert(0, (b'!' + i as u8) as char);
        tag.add(format!("author_{i}"), peers[i]);
    }

    // Hub collects from all spokes
    let spoke_rgas: Vec<_> = rgas[1..].iter().cloned().collect();
    let spoke_tags: Vec<_> = tags[1..].iter().cloned().collect();
    for (sr, st) in spoke_rgas.iter().zip(spoke_tags.iter()) {
        rgas[hub].merge(sr);
        tags[hub].merge(st);
    }

    // Hub broadcasts back
    let hub_rga = rgas[hub].clone();
    let hub_tags = tags[hub].clone();
    for i in 1..n {
        rgas[i].merge(&hub_rga);
        tags[i].merge(&hub_tags);
    }

    // Verify convergence
    let ref_str = rgas[0].as_string();
    let ref_tags: HashSet<_> = tags[0].iter().cloned().collect();

    for (i, (rga, tag)) in rgas.iter().zip(tags.iter()).enumerate() {
        assert_eq!(rga.as_string(), ref_str, "RGA diverged at peer {i}");
        let t: HashSet<_> = tag.iter().cloned().collect();
        assert_eq!(t, ref_tags, "Tags diverged at peer {i}");
    }

    assert_eq!(ref_str.len(), n, "Document should have {n} characters");
    assert_eq!(ref_tags.len(), n, "Should have {n} author tags");
}

#[test]
fn enterprise_30_peers_full_entity_with_interleaved_sync() {
    // 30 peers collaborate on a full entity (RGA content + ORSet tags + PNCounter views).
    // 3 sync rounds with writes between each round.
    let n = 30;
    let peers: Vec<PeerId> = (0..n).map(|i| peer(i as u8)).collect();

    let mut contents: Vec<RGA<char>> = peers.iter().map(|&p| RGA::new(p)).collect();
    let mut tags: Vec<ORSet<String>> = (0..n).map(|_| ORSet::new()).collect();
    let mut counters: Vec<PNCounter> = (0..n).map(|_| PNCounter::new()).collect();

    for round in 0..3u32 {
        // Each peer writes
        for (i, ((content, tag), counter)) in contents
            .iter_mut()
            .zip(tags.iter_mut())
            .zip(counters.iter_mut())
            .enumerate()
        {
            let c = (b'a' + ((round * n as u32 + i as u32) % 26) as u8) as char;
            let pos = content.len();
            content.insert(pos, c);
            tag.add(format!("round{round}_peer{i}"), peers[i]);
            counter.increment(peers[i], 1);
        }

        // Sync: collect into hub (peer 0), broadcast
        let spoke_contents: Vec<_> = contents[1..].iter().cloned().collect();
        let spoke_tags: Vec<_> = tags[1..].iter().cloned().collect();
        let spoke_counters: Vec<_> = counters[1..].iter().cloned().collect();
        for (sc, (st, sctr)) in spoke_contents.iter().zip(spoke_tags.iter().zip(spoke_counters.iter())) {
            merge_entity(&mut contents[0], &mut tags[0], &mut counters[0], sc, st, sctr);
        }

        let hub_content = contents[0].clone();
        let hub_tags = tags[0].clone();
        let hub_counter = counters[0].clone();
        for i in 1..n {
            merge_entity(
                &mut contents[i],
                &mut tags[i],
                &mut counters[i],
                &hub_content,
                &hub_tags,
                &hub_counter,
            );
        }
    }

    assert_all_converged(&contents, &tags, &counters);

    // 3 rounds × 30 peers = 90 characters
    assert_eq!(contents[0].as_string().len(), 90);
    // 3 rounds × 30 peers = 90 tags
    assert_eq!(tags[0].iter().count(), 90);
    // 3 rounds × 30 peers × 1 = 90
    assert_eq!(counters[0].value(), 90);
}

#[test]
fn enterprise_20_peers_partition_into_two_teams_then_merge() {
    // 20 peers split into 2 teams of 10 (simulating office partition).
    // Each team syncs internally for multiple rounds.
    // Then teams re-merge. All must converge.
    let n = 20;
    let peers: Vec<PeerId> = (0..n).map(|i| peer(i as u8)).collect();

    let mut sets: Vec<ORSet<String>> = (0..n).map(|_| ORSet::new()).collect();
    let mut counters: Vec<PNCounter> = (0..n).map(|_| PNCounter::new()).collect();

    // Common initial state
    for (i, (set, counter)) in sets.iter_mut().zip(counters.iter_mut()).enumerate() {
        set.add("shared_doc".into(), peers[i]);
        counter.increment(peers[i], 1);
    }

    // --- PARTITION: Team A (0..10), Team B (10..20) ---

    // Team A works for 3 rounds
    for round in 0..3u32 {
        for i in 0..10 {
            sets[i].add(format!("team_a_r{round}_p{i}"), peers[i]);
            counters[i].increment(peers[i], 1);
        }
        // Internal sync within Team A
        let team_snaps: Vec<_> = sets[0..10].iter().cloned().collect();
        let counter_snaps: Vec<_> = counters[0..10].iter().cloned().collect();
        for i in 0..10 {
            for (j, (snap, csnap)) in team_snaps.iter().zip(counter_snaps.iter()).enumerate() {
                if i != j {
                    sets[i].merge(snap);
                    counters[i].merge(csnap);
                }
            }
        }
    }

    // Team B works for 3 rounds (different items)
    for round in 0..3u32 {
        for i in 10..20 {
            sets[i].add(format!("team_b_r{round}_p{i}"), peers[i]);
            counters[i].increment(peers[i], 1);
        }
        let team_snaps: Vec<_> = sets[10..20].iter().cloned().collect();
        let counter_snaps: Vec<_> = counters[10..20].iter().cloned().collect();
        for i in 10..20 {
            for (j, (snap, csnap)) in team_snaps.iter().zip(counter_snaps.iter()).enumerate() {
                if (i - 10) != j {
                    sets[i].merge(snap);
                    counters[i].merge(csnap);
                }
            }
        }
    }

    // --- REUNION: Team leads (peer 0 and peer 10) sync ---
    let s0 = sets[0].clone();
    let s10 = sets[10].clone();
    let c0 = counters[0].clone();
    let c10 = counters[10].clone();

    sets[0].merge(&s10);
    sets[10].merge(&s0);
    counters[0].merge(&c10);
    counters[10].merge(&c0);

    // Each lead broadcasts to their team
    let lead_a = sets[0].clone();
    let lead_b = sets[10].clone();
    let clead_a = counters[0].clone();
    let clead_b = counters[10].clone();

    for i in 1..10 {
        sets[i].merge(&lead_a);
        counters[i].merge(&clead_a);
    }
    for i in 11..20 {
        sets[i].merge(&lead_b);
        counters[i].merge(&clead_b);
    }

    // All 20 must converge
    let ref_items: HashSet<_> = sets[0].iter().cloned().collect();
    let ref_val = counters[0].value();

    for (i, (set, counter)) in sets.iter().zip(counters.iter()).enumerate() {
        let items: HashSet<_> = set.iter().cloned().collect();
        assert_eq!(items, ref_items, "Peer {i} ORSet diverged after reunion");
        assert_eq!(counter.value(), ref_val, "Peer {i} counter diverged");
    }

    // 1 shared_doc + 10×3 team_a items + 10×3 team_b items = 1 + 30 + 30 = 61
    assert_eq!(ref_items.len(), 61);
    // Counter: 20 initial + 10×3 team A + 10×3 team B = 20 + 30 + 30 = 80
    assert_eq!(ref_val, 80);
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. VECTOR CLOCK SCALING
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn vector_clock_50_peers_gossip_convergence() {
    // 50 peers each increment, sync via gossip. After enough rounds,
    // all clocks must dominate all individual starting states.
    let n = 50;
    let peers: Vec<PeerId> = (0..n).map(|i| peer(i as u8)).collect();
    let mut clocks: Vec<VectorClock> = (0..n).map(|_| VectorClock::new()).collect();

    // Each peer increments 10 times
    for (i, vc) in clocks.iter_mut().enumerate() {
        for _ in 0..10 {
            vc.increment(peers[i]);
        }
    }

    // Gossip rounds: peer i merges with (i + round) % n
    for round in 1..=n {
        let snaps: Vec<_> = clocks.iter().cloned().collect();
        for i in 0..n {
            clocks[i].merge(&snaps[(i + round) % n]);
        }
    }

    // All clocks must be identical
    for i in 1..n {
        assert_eq!(
            clocks[i].compare(&clocks[0]),
            CausalOrder::Equal,
            "Clock {i} not equal to clock 0 after gossip convergence"
        );
    }

    // Every peer must be at 10
    for &p in &peers {
        assert_eq!(clocks[0].get(&p), 10);
    }
}

#[test]
fn vector_clock_causality_preserved_through_chain() {
    // Verify that causal ordering is maintained through a 10-peer chain.
    // Peer 0 writes, syncs to 1, who writes and syncs to 2, etc.
    // Each later peer's clock must dominate all earlier peers'.
    let n = 10;
    let peers: Vec<PeerId> = (0..n).map(|i| peer(i as u8)).collect();
    let mut clocks: Vec<VectorClock> = (0..n).map(|_| VectorClock::new()).collect();

    for i in 0..n {
        // Peer i writes
        clocks[i].increment(peers[i]);

        // Sync to next peer (if not last)
        if i + 1 < n {
            let snap = clocks[i].clone();
            clocks[i + 1].merge(&snap);
        }
    }

    // Each peer i's clock should dominate all peers < i
    for i in 1..n {
        for j in 0..i {
            assert!(
                clocks[i].dominates(&clocks[j]),
                "Clock {i} should dominate clock {j} (causal chain)"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. DUPLICATE MESSAGE / RE-SYNC IDEMPOTENCE
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn duplicate_merge_is_idempotent_all_crdts() {
    // Merging the same state twice (simulating network retry) must be a no-op.
    let pa = peer(1);
    let pb = peer(2);

    // RGA
    let mut a_rga = RGA::from_str("Hello", pa);
    let mut b_rga = a_rga.clone();
    b_rga.set_peer_id(pb);
    b_rga.insert_str(5, " World");

    a_rga.merge(&b_rga);
    let after_first = a_rga.as_string();
    a_rga.merge(&b_rga); // duplicate
    a_rga.merge(&b_rga); // triplicate
    assert_eq!(a_rga.as_string(), after_first, "RGA not idempotent");

    // ORSet
    let mut a_set: ORSet<String> = ORSet::new();
    a_set.add("x".into(), pa);
    let mut b_set = a_set.clone();
    b_set.add("y".into(), pb);

    a_set.merge(&b_set);
    let after_first: HashSet<_> = a_set.iter().cloned().collect();
    a_set.merge(&b_set);
    a_set.merge(&b_set);
    let after_triple: HashSet<_> = a_set.iter().cloned().collect();
    assert_eq!(after_first, after_triple, "ORSet not idempotent");

    // PNCounter
    let mut a_c = PNCounter::new();
    a_c.increment(pa, 5);
    let mut b_c = PNCounter::new();
    b_c.increment(pb, 3);

    a_c.merge(&b_c);
    let v1 = a_c.value();
    a_c.merge(&b_c);
    a_c.merge(&b_c);
    assert_eq!(a_c.value(), v1, "PNCounter not idempotent");

    // VectorClock
    let mut a_vc = VectorClock::new();
    a_vc.increment(pa);
    let mut b_vc = VectorClock::new();
    b_vc.increment(pb);

    a_vc.merge(&b_vc);
    let after = a_vc.clone();
    a_vc.merge(&b_vc);
    a_vc.merge(&b_vc);
    assert_eq!(a_vc.compare(&after), CausalOrder::Equal, "VectorClock not idempotent");
}

#[test]
fn out_of_order_merge_produces_same_result() {
    // 5 peers' states arrive in different orders. Result must be identical.
    let peers: Vec<PeerId> = (0..5).map(|i| peer(i)).collect();
    let mut sets: Vec<ORSet<String>> = Vec::new();

    for (i, &p) in peers.iter().enumerate() {
        let mut s = ORSet::new();
        s.add(format!("item_{i}"), p);
        sets.push(s);
    }

    // Order 1: 0, 1, 2, 3, 4
    let mut merged_1 = ORSet::new();
    for s in &sets {
        merged_1.merge(s);
    }

    // Order 2: 4, 3, 2, 1, 0
    let mut merged_2 = ORSet::new();
    for s in sets.iter().rev() {
        merged_2.merge(s);
    }

    // Order 3: 2, 0, 4, 1, 3
    let mut merged_3 = ORSet::new();
    for &i in &[2, 0, 4, 1, 3] {
        merged_3.merge(&sets[i]);
    }

    let items_1: HashSet<_> = merged_1.iter().cloned().collect();
    let items_2: HashSet<_> = merged_2.iter().cloned().collect();
    let items_3: HashSet<_> = merged_3.iter().cloned().collect();

    assert_eq!(items_1, items_2, "Merge order 1 vs 2 diverged");
    assert_eq!(items_2, items_3, "Merge order 2 vs 3 diverged");
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. LWW REGISTER N-PEER TIEBREAK DETERMINISM
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn lww_register_50_peers_same_timestamp_deterministic_winner() {
    // 50 peers all write at the exact same timestamp.
    // Merge order must not affect which value wins.
    let ts = HybridTimestamp::new(5000, 0);
    let mut registers: Vec<LWWRegister<String>> = Vec::new();

    for i in 0u8..50 {
        let p = peer(i);
        registers.push(LWWRegister::with_timestamp(format!("val_{i}"), ts, p));
    }

    // Forward merge
    let mut forward = registers[0].clone();
    for r in &registers[1..] {
        forward.merge(r);
    }

    // Reverse merge
    let mut reverse = registers[49].clone();
    for r in registers[..49].iter().rev() {
        reverse.merge(r);
    }

    // Random-ish order
    let mut shuffled = registers[25].clone();
    for &i in &[0, 49, 12, 37, 5, 44, 18, 31, 7, 42] {
        shuffled.merge(&registers[i]);
    }
    for r in &registers {
        shuffled.merge(r);
    }

    assert_eq!(forward.value(), reverse.value(), "Forward vs reverse tiebreak mismatch");
    assert_eq!(reverse.value(), shuffled.value(), "Reverse vs shuffled tiebreak mismatch");

    // Highest peer ID wins (peer(49))
    assert_eq!(*forward.value(), "val_49");
}
