use privstack_crdt::PNCounter;
use privstack_types::PeerId;

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
    let mut a = PNCounter::new();
    let mut b = PNCounter::new();
    let mut c = PNCounter::new();

    a.increment(peer(1), 10);
    b.increment(peer(2), 20);
    b.decrement(peer(2), 5);
    c.decrement(peer(3), 3);

    let a_snap = a.clone();
    let b_snap = b.clone();
    let c_snap = c.clone();

    a.merge(&b_snap);
    a.merge(&c_snap);
    b.merge(&a_snap);
    b.merge(&c_snap);
    c.merge(&a_snap);
    c.merge(&b_snap);

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
    let mut a = PNCounter::new();
    a.increment(peer(1), 5);

    let mut b = PNCounter::new();
    b.increment(peer(2), 5);

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
