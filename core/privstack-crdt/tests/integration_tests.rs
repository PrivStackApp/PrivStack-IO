use privstack_crdt::{LWWRegister, ORSet, RGA};
use privstack_types::PeerId;

/// Integration test: entity with title (LWW) and tags (ORSet)
#[test]
fn entity_model_integration() {
    let peer1 = PeerId::new();
    let peer2 = PeerId::new();

    let mut title1 = LWWRegister::new("Draft".to_string(), peer1);
    let mut tags1: ORSet<String> = ORSet::new();
    tags1.add("work".to_string(), peer1);

    let mut title2 = title1.clone();
    let mut tags2 = tags1.clone();

    title1.set("Final Draft".to_string(), peer1);
    tags1.add("important".to_string(), peer1);

    title2.set("Revision".to_string(), peer2);
    tags2.add("review".to_string(), peer2);
    tags2.remove(&"work".to_string());

    title1.merge(&title2);
    tags1.merge(&tags2);

    assert!(title1.value() == "Final Draft" || title1.value() == "Revision");
    assert!(tags1.contains(&"important".to_string()));
    assert!(tags1.contains(&"review".to_string()));
}

/// Integration test: collaborative text editing with RGA
#[test]
fn collaborative_text_editing() {
    let peer1 = PeerId::new();
    let peer2 = PeerId::new();

    let mut text1 = RGA::from_str("Hello", peer1);
    let mut text2 = text1.clone();
    text2.set_peer_id(peer2);

    text1.insert_str(5, " World");
    text2.insert(5, '!');

    text1.merge(&text2);
    text2.merge(&text1);

    assert_eq!(text1.as_string(), text2.as_string());

    let result = text1.as_string();
    assert!(result.starts_with("Hello"));
    assert!(result.contains("World") || result.contains("orld"));
    assert!(result.contains('!'));
    assert_eq!(result.chars().count(), 12);
}
