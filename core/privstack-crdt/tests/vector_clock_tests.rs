use privstack_crdt::{CausalOrder, VectorClock};
use privstack_types::PeerId;

#[test]
fn new_clock_is_empty() {
    let clock = VectorClock::new();
    assert!(clock.is_empty());
    assert_eq!(clock.len(), 0);
}

#[test]
fn default_is_empty() {
    let clock = VectorClock::default();
    assert!(clock.is_empty());
}

#[test]
fn for_peer_creates_single_entry() {
    let peer = PeerId::new();
    let clock = VectorClock::for_peer(peer);
    assert_eq!(clock.len(), 1);
    assert_eq!(clock.get(&peer), 0);
    assert!(!clock.is_empty());
}

#[test]
fn get_unknown_peer_returns_zero() {
    let clock = VectorClock::new();
    assert_eq!(clock.get(&PeerId::new()), 0);
}

#[test]
fn increment_increases_time() {
    let peer = PeerId::new();
    let mut clock = VectorClock::new();

    assert_eq!(clock.get(&peer), 0);
    assert_eq!(clock.increment(peer), 1);
    assert_eq!(clock.get(&peer), 1);
    assert_eq!(clock.increment(peer), 2);
    assert_eq!(clock.get(&peer), 2);
}

#[test]
fn increment_adds_peer_to_clock() {
    let peer = PeerId::new();
    let mut clock = VectorClock::new();
    assert_eq!(clock.len(), 0);
    clock.increment(peer);
    assert_eq!(clock.len(), 1);
}

#[test]
fn update_higher_value() {
    let peer = PeerId::new();
    let mut clock = VectorClock::new();
    clock.update(peer, 5);
    assert_eq!(clock.get(&peer), 5);
}

#[test]
fn update_lower_value_is_noop() {
    let peer = PeerId::new();
    let mut clock = VectorClock::new();
    clock.update(peer, 10);
    clock.update(peer, 3);
    assert_eq!(clock.get(&peer), 10);
}

#[test]
fn update_equal_value_is_noop() {
    let peer = PeerId::new();
    let mut clock = VectorClock::new();
    clock.update(peer, 5);
    clock.update(peer, 5);
    assert_eq!(clock.get(&peer), 5);
}

#[test]
fn peers_iterator() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let mut clock = VectorClock::new();
    clock.increment(p1);
    clock.increment(p2);
    let peers: Vec<_> = clock.peers().collect();
    assert_eq!(peers.len(), 2);
}

// ── Compare ──────────────────────────────────────────────────────

#[test]
fn compare_empty_clocks_are_equal() {
    let a = VectorClock::new();
    let b = VectorClock::new();
    assert_eq!(a.compare(&b), CausalOrder::Equal);
}

#[test]
fn compare_equal_clocks() {
    let peer = PeerId::new();
    let mut a = VectorClock::new();
    let mut b = VectorClock::new();
    a.increment(peer);
    b.increment(peer);
    assert_eq!(a.compare(&b), CausalOrder::Equal);
    assert_eq!(a, b);
}

#[test]
fn compare_before_after() {
    let peer = PeerId::new();
    let mut a = VectorClock::new();
    let mut b = VectorClock::new();
    a.increment(peer);
    b.increment(peer);
    b.increment(peer);

    assert_eq!(a.compare(&b), CausalOrder::Before);
    assert_eq!(b.compare(&a), CausalOrder::After);
    assert!(a.is_before(&b));
    assert!(b.is_after(&a));
}

#[test]
fn compare_concurrent() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let mut a = VectorClock::new();
    let mut b = VectorClock::new();
    a.increment(p1);
    b.increment(p2);

    assert_eq!(a.compare(&b), CausalOrder::Concurrent);
    assert!(a.is_concurrent(&b));
    assert!(!a.is_before(&b));
    assert!(!a.is_after(&b));
}

#[test]
fn dominates_after() {
    let peer = PeerId::new();
    let mut a = VectorClock::new();
    let mut b = VectorClock::new();
    a.increment(peer);
    a.increment(peer);
    b.increment(peer);
    assert!(a.dominates(&b));
    assert!(!b.dominates(&a));
}

#[test]
fn dominates_equal() {
    let peer = PeerId::new();
    let mut a = VectorClock::new();
    a.increment(peer);
    let b = a.clone();
    assert!(a.dominates(&b));
    assert!(b.dominates(&a));
}

#[test]
fn dominates_concurrent_neither() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let mut a = VectorClock::new();
    let mut b = VectorClock::new();
    a.increment(p1);
    b.increment(p2);
    assert!(!a.dominates(&b));
    assert!(!b.dominates(&a));
}

// ── Merge ────────────────────────────────────────────────────────

#[test]
fn merge_takes_maximum() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let mut a = VectorClock::new();
    a.increment(p1);
    a.increment(p1);

    let mut b = VectorClock::new();
    b.increment(p1);
    b.increment(p2);
    b.increment(p2);
    b.increment(p2);

    a.merge(&b);
    assert_eq!(a.get(&p1), 2);
    assert_eq!(a.get(&p2), 3);
}

#[test]
fn merge_is_commutative() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let mut a = VectorClock::new();
    a.increment(p1);
    a.increment(p1);
    let mut b = VectorClock::new();
    b.increment(p2);

    assert_eq!(a.merged(&b), b.merged(&a));
}

#[test]
fn merge_is_idempotent() {
    let peer = PeerId::new();
    let mut clock = VectorClock::new();
    clock.increment(peer);
    clock.increment(peer);

    let once = clock.merged(&clock);
    let twice = once.merged(&clock);
    assert_eq!(once, twice);
}

#[test]
fn merge_is_associative() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let p3 = PeerId::new();

    let mut a = VectorClock::new();
    a.increment(p1);
    let mut b = VectorClock::new();
    b.increment(p2);
    let mut c = VectorClock::new();
    c.increment(p3);

    let ab_c = a.merged(&b).merged(&c);
    let a_bc = a.merged(&b.merged(&c));
    assert_eq!(ab_c, a_bc);
}

#[test]
fn merged_returns_new_clock() {
    let peer = PeerId::new();
    let mut a = VectorClock::new();
    a.increment(peer);
    let b = VectorClock::new();

    let result = a.merged(&b);
    assert_eq!(result.get(&peer), 1);
    // Original unchanged
    assert_eq!(a.get(&peer), 1);
}

// ── PartialEq ────────────────────────────────────────────────────

#[test]
fn partial_eq_symmetric() {
    let peer = PeerId::new();
    let mut a = VectorClock::new();
    a.increment(peer);
    let b = a.clone();
    assert_eq!(a, b);
    assert_eq!(b, a);
}

#[test]
fn partial_eq_different() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let mut a = VectorClock::new();
    a.increment(p1);
    let mut b = VectorClock::new();
    b.increment(p2);
    assert_ne!(a, b);
}

// ── Serde ────────────────────────────────────────────────────────

#[test]
fn serialization_roundtrip() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let mut clock = VectorClock::new();
    clock.increment(p1);
    clock.increment(p1);
    clock.increment(p2);

    let json = serde_json::to_string(&clock).unwrap();
    let parsed: VectorClock = serde_json::from_str(&json).unwrap();
    assert_eq!(clock, parsed);
}
