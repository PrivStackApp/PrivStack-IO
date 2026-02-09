use privstack_crdt::{ORSet, Tag};
use privstack_types::PeerId;
use std::collections::HashSet;

#[test]
fn new_set_is_empty() {
    let set: ORSet<i32> = ORSet::new();
    assert!(set.is_empty());
    assert_eq!(set.len(), 0);
}

#[test]
fn default_set_is_empty() {
    let set: ORSet<String> = ORSet::default();
    assert!(set.is_empty());
}

#[test]
fn add_and_contains() {
    let peer = PeerId::new();
    let mut set = ORSet::new();
    set.add(1, peer);
    set.add(2, peer);
    assert!(set.contains(&1));
    assert!(set.contains(&2));
    assert!(!set.contains(&3));
    assert_eq!(set.len(), 2);
}

#[test]
fn add_returns_unique_tags() {
    let peer = PeerId::new();
    let mut set = ORSet::new();
    let t1 = set.add(1, peer);
    let t2 = set.add(1, peer);
    assert_ne!(t1, t2); // Tags not exposed via PartialEq? Actually they derive it.
}

#[test]
fn remove_element() {
    let peer = PeerId::new();
    let mut set = ORSet::new();
    set.add(1, peer);
    assert!(set.contains(&1));

    let removed = set.remove(&1);
    assert!(!removed.is_empty());
    assert!(!set.contains(&1));
    assert!(set.is_empty());
}

#[test]
fn remove_nonexistent_returns_empty() {
    let mut set: ORSet<i32> = ORSet::new();
    let removed = set.remove(&999);
    assert!(removed.is_empty());
}

#[test]
fn add_after_remove() {
    let peer = PeerId::new();
    let mut set = ORSet::new();
    set.add(1, peer);
    set.remove(&1);
    set.add(1, peer);
    assert!(set.contains(&1));
}

#[test]
fn tags_for_element() {
    let peer = PeerId::new();
    let mut set = ORSet::new();
    set.add("x", peer);
    set.add("x", peer);
    let tags = set.tags_for(&"x").unwrap();
    assert_eq!(tags.len(), 2);
}

#[test]
fn tags_for_nonexistent_returns_none() {
    let set: ORSet<i32> = ORSet::new();
    assert!(set.tags_for(&1).is_none());
}

#[test]
fn tombstones_track_removed_tags() {
    let peer = PeerId::new();
    let mut set = ORSet::new();
    set.add(1, peer);
    assert!(set.tombstones().is_empty());
    set.remove(&1);
    assert!(!set.tombstones().is_empty());
}

#[test]
fn add_with_tag_works() {
    let mut set = ORSet::new();
    let tag = Tag::new();
    set.add_with_tag(42, tag);
    assert!(set.contains(&42));
}

#[test]
fn add_with_tombstoned_tag_is_noop() {
    let peer = PeerId::new();
    let mut set = ORSet::new();
    let tag = set.add(1, peer);
    set.remove(&1); // tombstones that tag
    set.add_with_tag(1, tag); // re-add same tag — should be rejected
    assert!(!set.contains(&1));
}

#[test]
fn remove_tags_specific() {
    let peer = PeerId::new();
    let mut set = ORSet::new();
    let t1 = set.add(1, peer);
    let _t2 = set.add(1, peer);
    set.remove_tags(&[t1]);
    // Still contains element because t2 is alive
    assert!(set.contains(&1));
    assert_eq!(set.tags_for(&1).unwrap().len(), 1);
}

#[test]
fn gc_tombstones() {
    let peer = PeerId::new();
    let mut set = ORSet::new();
    set.add(1, peer);
    set.remove(&1);
    assert!(!set.tombstones().is_empty());
    set.gc_tombstones(|_| false); // remove all tombstones
    assert!(set.tombstones().is_empty());
}

#[test]
fn gc_tombstones_keep_some() {
    let peer = PeerId::new();
    let mut set = ORSet::new();
    set.add(1, peer);
    set.add(2, peer);
    set.remove(&1);
    set.remove(&2);
    let count_before = set.tombstones().len();
    assert!(count_before >= 2);
    // Keep all — no change
    set.gc_tombstones(|_| true);
    assert_eq!(set.tombstones().len(), count_before);
}

// ── Concurrent add/remove ────────────────────────────────────────

#[test]
fn concurrent_add_remove_add_wins() {
    let peer1 = PeerId::new();
    let mut set1 = ORSet::new();
    set1.add("item", peer1);

    let mut set2 = set1.clone();
    set2.remove(&"item");
    set1.add("item", peer1); // concurrent re-add

    set1.merge(&set2);
    assert!(set1.contains(&"item")); // add wins

    set2.merge(&set1);
    assert!(set2.contains(&"item"));
}

// ── Merge properties ─────────────────────────────────────────────

#[test]
fn merge_is_commutative() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let mut s1 = ORSet::new();
    s1.add(1, p1);
    s1.add(2, p1);
    let mut s2 = ORSet::new();
    s2.add(2, p2);
    s2.add(3, p2);

    let m12 = s1.merged(&s2);
    let m21 = s2.merged(&s1);

    for &v in &[1, 2, 3] {
        assert_eq!(m12.contains(&v), m21.contains(&v));
    }
}

#[test]
fn merge_is_idempotent() {
    let peer = PeerId::new();
    let mut set = ORSet::new();
    set.add(1, peer);
    set.add(2, peer);
    let once = set.merged(&set);
    let twice = once.merged(&set);
    assert_eq!(once.len(), twice.len());
}

#[test]
fn merge_is_associative() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let p3 = PeerId::new();
    let mut a = ORSet::new();
    a.add(1, p1);
    let mut b = ORSet::new();
    b.add(2, p2);
    let mut c = ORSet::new();
    c.add(3, p3);

    let ab_c = a.merged(&b).merged(&c);
    let a_bc = a.merged(&b.merged(&c));
    for &v in &[1, 2, 3] {
        assert_eq!(ab_c.contains(&v), a_bc.contains(&v));
    }
}

// ── Iteration ────────────────────────────────────────────────────

#[test]
fn iterate_elements() {
    let peer = PeerId::new();
    let mut set = ORSet::new();
    set.add(1, peer);
    set.add(2, peer);
    set.add(3, peer);
    set.remove(&2);
    let elements: HashSet<_> = set.iter().copied().collect();
    assert_eq!(elements, HashSet::from([1, 3]));
}

#[test]
fn from_iterator() {
    let set: ORSet<i32> = vec![1, 2, 3].into_iter().collect();
    assert_eq!(set.len(), 3);
    assert!(set.contains(&1));
    assert!(set.contains(&2));
    assert!(set.contains(&3));
}

// ── Tag ──────────────────────────────────────────────────────────

#[test]
fn tag_unique() {
    let t1 = Tag::new();
    let t2 = Tag::new();
    assert_ne!(t1, t2);
}

#[test]
fn tag_default_unique() {
    let t1 = Tag::default();
    let t2 = Tag::default();
    assert_ne!(t1, t2);
}

#[test]
fn tag_clone_and_copy() {
    let t = Tag::new();
    let t2 = t;
    assert_eq!(t, t2);
}

#[test]
fn tag_hash() {
    let t = Tag::new();
    let mut set = HashSet::new();
    set.insert(t);
    set.insert(t);
    assert_eq!(set.len(), 1);
}

// ── Serde ────────────────────────────────────────────────────────

#[test]
fn serialization_roundtrip() {
    let peer = PeerId::new();
    let mut set = ORSet::new();
    set.add("a", peer);
    set.add("b", peer);
    let json = serde_json::to_string(&set).unwrap();
    let parsed: ORSet<&str> = serde_json::from_str(&json).unwrap();
    assert!(parsed.contains(&"a"));
    assert!(parsed.contains(&"b"));
}
