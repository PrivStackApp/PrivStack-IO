use privstack_crdt::LWWRegister;
use privstack_types::{HybridTimestamp, PeerId};

#[test]
fn new_register() {
    let peer = PeerId::new();
    let reg = LWWRegister::new(42, peer);
    assert_eq!(*reg.value(), 42);
    assert_eq!(reg.peer_id(), peer);
}

#[test]
fn with_timestamp() {
    let peer = PeerId::new();
    let ts = HybridTimestamp::new(999, 5);
    let reg = LWWRegister::with_timestamp("hi", ts, peer);
    assert_eq!(*reg.value(), "hi");
    assert_eq!(reg.timestamp(), ts);
    assert_eq!(reg.peer_id(), peer);
}

#[test]
fn default_register() {
    let reg: LWWRegister<i32> = LWWRegister::default();
    assert_eq!(*reg.value(), 0);
}

#[test]
fn set_updates_value_and_timestamp() {
    let peer = PeerId::new();
    let mut reg = LWWRegister::new(1, peer);
    let old_ts = reg.timestamp();
    reg.set(2, peer);
    assert_eq!(*reg.value(), 2);
    assert!(reg.timestamp() > old_ts);
}

#[test]
fn set_updates_peer_id() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let mut reg = LWWRegister::new(1, p1);
    reg.set(2, p2);
    assert_eq!(reg.peer_id(), p2);
}

// ── set_with_timestamp ───────────────────────────────────────────

#[test]
fn set_with_timestamp_accepts_newer() {
    let peer = PeerId::new();
    let ts1 = HybridTimestamp::new(100, 0);
    let ts2 = HybridTimestamp::new(200, 0);
    let mut reg = LWWRegister::with_timestamp("old", ts1, peer);
    assert!(reg.set_with_timestamp("new", ts2, peer));
    assert_eq!(*reg.value(), "new");
    assert_eq!(reg.timestamp(), ts2);
}

#[test]
fn set_with_timestamp_rejects_older() {
    let peer = PeerId::new();
    let ts1 = HybridTimestamp::new(200, 0);
    let ts2 = HybridTimestamp::new(100, 0);
    let mut reg = LWWRegister::with_timestamp("keep", ts1, peer);
    assert!(!reg.set_with_timestamp("lose", ts2, peer));
    assert_eq!(*reg.value(), "keep");
}

#[test]
fn set_with_timestamp_tie_uses_peer_id() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let ts = HybridTimestamp::new(100, 0);
    let mut reg = LWWRegister::with_timestamp("a", ts, p1);

    let updated = reg.set_with_timestamp("b", ts, p2);
    // Should update only if p2 > p1
    assert_eq!(updated, p2.as_uuid() > p1.as_uuid());
}

// ── Merge ────────────────────────────────────────────────────────

#[test]
fn merge_higher_timestamp_wins() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let mut r1 = LWWRegister::with_timestamp("old", HybridTimestamp::new(100, 0), p1);
    let r2 = LWWRegister::with_timestamp("new", HybridTimestamp::new(200, 0), p2);
    r1.merge(&r2);
    assert_eq!(*r1.value(), "new");
}

#[test]
fn merge_lower_timestamp_loses() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let mut r1 = LWWRegister::with_timestamp("keep", HybridTimestamp::new(200, 0), p1);
    let r2 = LWWRegister::with_timestamp("lose", HybridTimestamp::new(100, 0), p2);
    r1.merge(&r2);
    assert_eq!(*r1.value(), "keep");
}

#[test]
fn merge_is_commutative() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let r1 = LWWRegister::with_timestamp("a", HybridTimestamp::new(100, 0), p1);
    let r2 = LWWRegister::with_timestamp("b", HybridTimestamp::new(200, 0), p2);
    assert_eq!(r1.merged(&r2), r2.merged(&r1));
}

#[test]
fn merge_is_idempotent() {
    let peer = PeerId::new();
    let reg = LWWRegister::new(42, peer);
    let once = reg.merged(&reg);
    let twice = once.merged(&reg);
    assert_eq!(once, twice);
}

#[test]
fn merge_is_associative() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let p3 = PeerId::new();
    let a = LWWRegister::with_timestamp("a", HybridTimestamp::new(100, 0), p1);
    let b = LWWRegister::with_timestamp("b", HybridTimestamp::new(200, 0), p2);
    let c = LWWRegister::with_timestamp("c", HybridTimestamp::new(150, 0), p3);

    let ab_c = a.merged(&b).merged(&c);
    let a_bc = a.merged(&b.merged(&c));
    assert_eq!(ab_c, a_bc);
}

#[test]
fn tie_breaker_uses_peer_id() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let ts = HybridTimestamp::new(1000, 0);
    let r1 = LWWRegister::with_timestamp("p1", ts, p1);
    let r2 = LWWRegister::with_timestamp("p2", ts, p2);
    let merged = r1.merged(&r2);
    let expected = if p1.as_uuid() > p2.as_uuid() { "p1" } else { "p2" };
    assert_eq!(*merged.value(), expected);
}

// ── PartialEq ────────────────────────────────────────────────────

#[test]
fn equality_same_value_same_ts() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let ts = HybridTimestamp::new(100, 0);
    let a = LWWRegister::with_timestamp(42, ts, p1);
    let b = LWWRegister::with_timestamp(42, ts, p2);
    assert_eq!(a, b); // same value & timestamp → equal
}

#[test]
fn inequality_different_value() {
    let peer = PeerId::new();
    let ts = HybridTimestamp::new(100, 0);
    let a = LWWRegister::with_timestamp(1, ts, peer);
    let b = LWWRegister::with_timestamp(2, ts, peer);
    assert_ne!(a, b);
}

// ── Serde ────────────────────────────────────────────────────────

#[test]
fn serialization_roundtrip() {
    let peer = PeerId::new();
    let reg = LWWRegister::new("test value", peer);
    let json = serde_json::to_string(&reg).unwrap();
    let parsed: LWWRegister<&str> = serde_json::from_str(&json).unwrap();
    assert_eq!(*reg.value(), *parsed.value());
    assert_eq!(reg.timestamp(), parsed.timestamp());
}

#[test]
fn serialization_roundtrip_i32() {
    let peer = PeerId::new();
    let reg = LWWRegister::new(42, peer);
    let json = serde_json::to_string(&reg).unwrap();
    let parsed: LWWRegister<i32> = serde_json::from_str(&json).unwrap();
    assert_eq!(*parsed.value(), 42);
}
