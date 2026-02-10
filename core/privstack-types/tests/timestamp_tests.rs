use privstack_types::HybridTimestamp;

// ── Construction ─────────────────────────────────────────────────

#[test]
fn now_has_zero_logical() {
    let ts = HybridTimestamp::now();
    assert_eq!(ts.logical(), 0);
    assert!(ts.wall_time() > 0);
}

#[test]
fn new_from_components() {
    let ts = HybridTimestamp::new(42, 7);
    assert_eq!(ts.wall_time(), 42);
    assert_eq!(ts.logical(), 7);
}

#[test]
fn default_is_now() {
    let ts = HybridTimestamp::default();
    assert!(ts.wall_time() > 0);
    assert_eq!(ts.logical(), 0);
}

// ── Ordering ─────────────────────────────────────────────────────

#[test]
fn ordering_by_wall_time() {
    let a = HybridTimestamp::new(100, 0);
    let b = HybridTimestamp::new(200, 0);
    assert!(a < b);
}

#[test]
fn ordering_by_logical_when_wall_time_equal() {
    let a = HybridTimestamp::new(100, 0);
    let b = HybridTimestamp::new(100, 1);
    assert!(a < b);
}

#[test]
fn equal_timestamps() {
    let a = HybridTimestamp::new(100, 5);
    let b = HybridTimestamp::new(100, 5);
    assert_eq!(a, b);
    assert!(!(a < b));
    assert!(!(a > b));
}

#[test]
fn partial_ord_consistent_with_ord() {
    let a = HybridTimestamp::new(50, 1);
    let b = HybridTimestamp::new(50, 2);
    assert_eq!(a.partial_cmp(&b), Some(std::cmp::Ordering::Less));
}

// ── is_before / is_after ─────────────────────────────────────────

#[test]
fn is_before() {
    let a = HybridTimestamp::new(1, 0);
    let b = HybridTimestamp::new(2, 0);
    assert!(a.is_before(&b));
    assert!(!b.is_before(&a));
}

#[test]
fn is_after() {
    let a = HybridTimestamp::new(1, 0);
    let b = HybridTimestamp::new(2, 0);
    assert!(b.is_after(&a));
    assert!(!a.is_after(&b));
}

#[test]
fn is_before_and_after_equal() {
    let a = HybridTimestamp::new(5, 5);
    let b = HybridTimestamp::new(5, 5);
    assert!(!a.is_before(&b));
    assert!(!a.is_after(&b));
}

// ── tick ─────────────────────────────────────────────────────────

#[test]
fn tick_is_monotonic() {
    let t1 = HybridTimestamp::now();
    let t2 = t1.tick();
    let t3 = t2.tick();
    assert!(t1 < t2);
    assert!(t2 < t3);
}

#[test]
fn tick_increments_logical_when_wall_time_same() {
    // Use a far-future wall time so `now()` inside tick will be less
    let ts = HybridTimestamp::new(u64::MAX / 2, 0);
    let ticked = ts.tick();
    assert_eq!(ticked.wall_time(), ts.wall_time());
    assert_eq!(ticked.logical(), 1);
}

#[test]
fn tick_resets_logical_when_wall_time_advances() {
    // Use a wall time in the past so `now()` inside tick will be greater
    let ts = HybridTimestamp::new(1, 99);
    let ticked = ts.tick();
    assert!(ticked.wall_time() > 1);
    assert_eq!(ticked.logical(), 0);
}

// ── receive ──────────────────────────────────────────────────────

#[test]
fn receive_both_same_wall_time_both_behind_now() {
    // Both have wall_time in the past → now is max → logical resets to 0
    let local = HybridTimestamp::new(1, 5);
    let remote = HybridTimestamp::new(1, 10);
    let merged = local.receive(&remote);
    assert!(merged.wall_time() > 1);
    assert_eq!(merged.logical(), 0);
}

#[test]
fn receive_local_and_remote_same_wall_time_ahead_of_now() {
    // Both have the same far-future wall time
    let local = HybridTimestamp::new(u64::MAX / 2, 5);
    let remote = HybridTimestamp::new(u64::MAX / 2, 10);
    let merged = local.receive(&remote);
    assert_eq!(merged.wall_time(), u64::MAX / 2);
    // max(5,10) + 1 = 11
    assert_eq!(merged.logical(), 11);
}

#[test]
fn receive_local_ahead_of_remote_and_now() {
    let local = HybridTimestamp::new(u64::MAX / 2, 3);
    let remote = HybridTimestamp::new(1, 0);
    let merged = local.receive(&remote);
    assert_eq!(merged.wall_time(), u64::MAX / 2);
    assert_eq!(merged.logical(), 4); // local.logical + 1
}

#[test]
fn receive_remote_ahead_of_local_and_now() {
    let local = HybridTimestamp::new(1, 0);
    let remote = HybridTimestamp::new(u64::MAX / 2, 7);
    let merged = local.receive(&remote);
    assert_eq!(merged.wall_time(), u64::MAX / 2);
    assert_eq!(merged.logical(), 8); // remote.logical + 1
}

#[test]
fn receive_result_is_greater_than_both() {
    let local = HybridTimestamp::new(1000, 5);
    let remote = HybridTimestamp::new(1000, 10);
    let merged = local.receive(&remote);
    assert!(merged > local);
    assert!(merged > remote);
}

// ── Serde ────────────────────────────────────────────────────────

#[test]
fn serialization_roundtrip() {
    let ts = HybridTimestamp::new(1234567890, 42);
    let json = serde_json::to_string(&ts).unwrap();
    let parsed: HybridTimestamp = serde_json::from_str(&json).unwrap();
    assert_eq!(ts, parsed);
}

// ── Hash ─────────────────────────────────────────────────────────

#[test]
fn hash_consistent_with_eq() {
    use std::collections::HashSet;
    let ts = HybridTimestamp::new(100, 5);
    let mut set = HashSet::new();
    set.insert(ts);
    set.insert(ts);
    assert_eq!(set.len(), 1);
}
