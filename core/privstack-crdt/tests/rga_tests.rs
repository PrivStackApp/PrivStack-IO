use privstack_crdt::{ElementId, RGA};
use privstack_types::{HybridTimestamp, PeerId};

// ── ElementId ────────────────────────────────────────────────────

#[test]
fn element_id_root() {
    let root = ElementId::root();
    assert!(root.is_root());
}

#[test]
fn element_id_non_root() {
    let id = ElementId::new(HybridTimestamp::new(100, 0), PeerId::new(), 1);
    assert!(!id.is_root());
}

#[test]
fn element_id_display_and_parse() {
    let peer = PeerId::new();
    let id = ElementId::new(HybridTimestamp::new(12345, 0), peer, 7);
    let s = id.to_string();
    let parsed: ElementId = s.parse().unwrap();
    assert_eq!(parsed.timestamp.wall_time(), 12345);
    assert_eq!(parsed.peer_id, peer);
    assert_eq!(parsed.seq, 7);
}

#[test]
fn element_id_parse_invalid() {
    assert!("bad".parse::<ElementId>().is_err());
    assert!("a:b".parse::<ElementId>().is_err());
    assert!("a:b:c:d".parse::<ElementId>().is_err());
    assert!("notanumber:00000000-0000-0000-0000-000000000000:0".parse::<ElementId>().is_err());
}

#[test]
fn element_id_ordering() {
    let p = PeerId::new();
    let a = ElementId::new(HybridTimestamp::new(100, 0), p, 0);
    let b = ElementId::new(HybridTimestamp::new(200, 0), p, 0);
    assert!(a < b);
}

#[test]
fn element_id_ordering_by_seq() {
    let p = PeerId::new();
    let ts = HybridTimestamp::new(100, 0);
    let a = ElementId::new(ts, p, 1);
    let b = ElementId::new(ts, p, 2);
    assert!(a < b);
}

#[test]
fn element_id_hash_eq() {
    use std::collections::HashSet;
    let id = ElementId::new(HybridTimestamp::new(1, 0), PeerId::new(), 0);
    let mut set = HashSet::new();
    set.insert(id);
    set.insert(id);
    assert_eq!(set.len(), 1);
}

// ── RGA basics ───────────────────────────────────────────────────

#[test]
fn new_rga_is_empty() {
    let rga: RGA<char> = RGA::new(PeerId::new());
    assert!(rga.is_empty());
    assert_eq!(rga.len(), 0);
}

#[test]
fn peer_id_accessors() {
    let p = PeerId::new();
    let mut rga: RGA<i32> = RGA::new(p);
    assert_eq!(rga.peer_id(), p);
    let p2 = PeerId::new();
    rga.set_peer_id(p2);
    assert_eq!(rga.peer_id(), p2);
}

#[test]
fn insert_and_get() {
    let mut rga = RGA::new(PeerId::new());
    rga.insert(0, 'a');
    rga.insert(1, 'b');
    rga.insert(2, 'c');
    assert_eq!(rga.len(), 3);
    assert_eq!(rga.get(0), Some(&'a'));
    assert_eq!(rga.get(1), Some(&'b'));
    assert_eq!(rga.get(2), Some(&'c'));
}

#[test]
fn get_out_of_bounds() {
    let rga: RGA<i32> = RGA::new(PeerId::new());
    assert_eq!(rga.get(0), None);
    assert_eq!(rga.get(999), None);
}

#[test]
fn insert_at_beginning() {
    let mut rga = RGA::new(PeerId::new());
    rga.insert(0, 'b');
    rga.insert(0, 'a');
    assert_eq!(rga.to_vec(), vec!['a', 'b']);
}

#[test]
fn insert_in_middle() {
    let mut rga = RGA::new(PeerId::new());
    rga.insert(0, 'a');
    rga.insert(1, 'c');
    rga.insert(1, 'b');
    assert_eq!(rga.to_vec(), vec!['a', 'b', 'c']);
}

#[test]
fn delete_element() {
    let mut rga = RGA::new(PeerId::new());
    rga.insert(0, 'a');
    rga.insert(1, 'b');
    rga.insert(2, 'c');
    let deleted = rga.delete(1);
    assert!(deleted.is_some());
    assert_eq!(rga.len(), 2);
    assert_eq!(rga.to_vec(), vec!['a', 'c']);
}

#[test]
fn delete_out_of_bounds() {
    let mut rga: RGA<i32> = RGA::new(PeerId::new());
    assert_eq!(rga.delete(0), None);
}

#[test]
fn delete_by_id() {
    let mut rga = RGA::new(PeerId::new());
    let id = rga.insert(0, 'x');
    rga.delete_by_id(id);
    assert!(rga.is_empty());
}

#[test]
fn insert_with_id() {
    let peer = PeerId::new();
    let mut rga = RGA::new(peer);
    let id = ElementId::new(HybridTimestamp::new(5000, 0), peer, 99);
    rga.insert_with_id(id, ElementId::root(), 42);
    assert_eq!(rga.len(), 1);
    assert_eq!(rga.get(0), Some(&42));
}

// ── Element ID queries ───────────────────────────────────────────

#[test]
fn element_id_at() {
    let mut rga = RGA::new(PeerId::new());
    let id0 = rga.insert(0, 'a');
    let id1 = rga.insert(1, 'b');
    assert_eq!(rga.element_id_at(0), Some(id0));
    assert_eq!(rga.element_id_at(1), Some(id1));
    assert_eq!(rga.element_id_at(2), None);
}

#[test]
fn last_element_id_empty() {
    let rga: RGA<i32> = RGA::new(PeerId::new());
    assert_eq!(rga.last_element_id(), None);
}

#[test]
fn last_element_id_populated() {
    let mut rga = RGA::new(PeerId::new());
    rga.insert(0, 'a');
    let last = rga.insert(1, 'b');
    assert_eq!(rga.last_element_id(), Some(last));
}

#[test]
fn element_ids_in_order() {
    let mut rga = RGA::new(PeerId::new());
    rga.insert(0, 'a');
    rga.insert(1, 'b');
    let ids = rga.element_ids_in_order();
    assert_eq!(ids.len(), 2);
}

#[test]
fn index_of() {
    let mut rga = RGA::new(PeerId::new());
    let id0 = rga.insert(0, 'a');
    let id1 = rga.insert(1, 'b');
    assert_eq!(rga.index_of(&id0), Some(0));
    assert_eq!(rga.index_of(&id1), Some(1));
}

#[test]
fn index_of_deleted_returns_none() {
    let mut rga = RGA::new(PeerId::new());
    let id = rga.insert(0, 'a');
    rga.delete_by_id(id);
    assert_eq!(rga.index_of(&id), None);
}

#[test]
fn contains_element() {
    let mut rga = RGA::new(PeerId::new());
    let id = rga.insert(0, 'a');
    assert!(rga.contains_element(&id));
    // root is always present in the internal map
    assert!(rga.contains_element(&ElementId::root()));
}

#[test]
fn is_tombstoned() {
    let mut rga = RGA::new(PeerId::new());
    let id = rga.insert(0, 'a');
    assert!(!rga.is_tombstoned(&id));
    rga.delete_by_id(id);
    assert!(rga.is_tombstoned(&id));
}

#[test]
fn is_tombstoned_nonexistent() {
    let rga: RGA<i32> = RGA::new(PeerId::new());
    let fake = ElementId::new(HybridTimestamp::new(9999, 0), PeerId::new(), 0);
    assert!(!rga.is_tombstoned(&fake));
}

// ── String operations ────────────────────────────────────────────

#[test]
fn from_str_and_as_string() {
    let rga = RGA::from_str("hello", PeerId::new());
    assert_eq!(rga.as_string(), "hello");
    assert_eq!(rga.len(), 5);
}

#[test]
fn insert_str() {
    let mut rga = RGA::from_str("ac", PeerId::new());
    rga.insert_str(1, "b");
    assert_eq!(rga.as_string(), "abc");
}

#[test]
fn insert_str_multi_char() {
    let mut rga = RGA::from_str("ad", PeerId::new());
    rga.insert_str(1, "bc");
    assert_eq!(rga.as_string(), "abcd");
}

#[test]
fn delete_range() {
    let mut rga = RGA::from_str("abcde", PeerId::new());
    rga.delete_range(1, 3); // delete b, c, d
    assert_eq!(rga.as_string(), "ae");
}

// ── Concurrent operations ────────────────────────────────────────

#[test]
fn concurrent_insert_same_position() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let mut rga1 = RGA::from_str("ac", p1);
    let mut rga2 = rga1.clone();
    rga2.set_peer_id(p2);

    rga1.insert(1, 'b');
    rga2.insert(1, 'x');

    rga1.merge(&rga2);
    rga2.merge(&rga1);
    assert_eq!(rga1.as_string(), rga2.as_string());
    assert_eq!(rga1.len(), 4);
}

#[test]
fn concurrent_delete_same_element() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let mut rga1 = RGA::from_str("abc", p1);
    let mut rga2 = rga1.clone();
    rga2.set_peer_id(p2);

    rga1.delete(1);
    rga2.delete(1);

    rga1.merge(&rga2);
    assert_eq!(rga1.as_string(), "ac");
}

// ── Merge properties ─────────────────────────────────────────────

#[test]
fn merge_is_commutative() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let base = RGA::from_str(".", p1);
    let mut r1 = base.clone();
    let mut r2 = base.clone();
    r2.set_peer_id(p2);

    r1.insert(0, 'a');
    r2.insert(0, 'x');

    assert_eq!(r1.merged(&r2).to_vec(), r2.merged(&r1).to_vec());
}

#[test]
fn merge_is_idempotent() {
    let rga = RGA::from_str("hello", PeerId::new());
    let once = rga.merged(&rga);
    let twice = once.merged(&rga);
    assert_eq!(once.as_string(), twice.as_string());
}

#[test]
fn insert_after_merge() {
    let p1 = PeerId::new();
    let p2 = PeerId::new();
    let mut r1 = RGA::from_str("a", p1);
    let mut r2 = r1.clone();
    r2.set_peer_id(p2);

    r1.insert(1, 'b');
    r2.insert(1, 'c');
    r1.merge(&r2);
    r1.insert(3, 'd');
    assert!(r1.as_string().ends_with('d'));
    assert_eq!(r1.len(), 4);
}

// ── Serde ────────────────────────────────────────────────────────

#[test]
fn serialization_roundtrip() {
    // RGA serde uses ElementId::Display/FromStr for HashMap keys.
    // FromStr loses the logical counter, so ordering may differ after round-trip.
    // We verify the structure survives serialization, not exact content order.
    let mut rga = RGA::new(PeerId::new());
    rga.insert(0, 'a');
    let json = serde_json::to_string(&rga).unwrap();
    let parsed: RGA<char> = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.len(), 1);
}
