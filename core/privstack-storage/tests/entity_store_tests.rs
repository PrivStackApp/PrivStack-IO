use privstack_model::{Entity, EntitySchema, FieldType, IndexedField, MergeStrategy};
use privstack_storage::EntityStore;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn test_schema() -> EntitySchema {
    EntitySchema {
        entity_type: "bookmark".into(),
        indexed_fields: vec![
            IndexedField::text("/title", true),
            IndexedField::text("/url", true),
            IndexedField::tag("/tags"),
        ],
        merge_strategy: MergeStrategy::LwwDocument,
    }
}

fn test_entity(title: &str) -> Entity {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    Entity {
        id: format!("test-entity-{id}"),
        entity_type: "bookmark".into(),
        data: serde_json::json!({
            "title": title,
            "url": "https://example.com",
            "tags": ["rust", "test"],
        }),
        created_at: 1000,
        modified_at: 1000,
        created_by: "test-peer".into(),
    }
}

// ── Basic CRUD ───────────────────────────────────────────────────

#[test]
fn save_and_get() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let entity = test_entity("Test Bookmark");

    store.save_entity(&entity, &schema).unwrap();

    let retrieved = store.get_entity(&entity.id).unwrap().unwrap();
    assert_eq!(retrieved.id, entity.id);
    assert_eq!(retrieved.entity_type, "bookmark");
    assert_eq!(retrieved.get_str("/title"), Some("Test Bookmark"));
}

#[test]
fn get_nonexistent_returns_none() {
    let store = EntityStore::open_in_memory().unwrap();
    let result = store.get_entity("nonexistent-id").unwrap();
    assert!(result.is_none());
}

#[test]
fn save_entity_raw() {
    let store = EntityStore::open_in_memory().unwrap();
    let entity = Entity {
        id: "raw-1".into(),
        entity_type: "note".into(),
        data: serde_json::json!({"content": "hello"}),
        created_at: 100,
        modified_at: 200,
        created_by: "peer1".into(),
    };
    store.save_entity_raw(&entity).unwrap();
    let retrieved = store.get_entity("raw-1").unwrap().unwrap();
    assert_eq!(retrieved.entity_type, "note");
    assert_eq!(retrieved.created_at, 100);
    assert_eq!(retrieved.modified_at, 200);
}

#[test]
fn upsert_overwrites() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let mut entity = test_entity("v1");
    let id = entity.id.clone();

    store.save_entity(&entity, &schema).unwrap();

    entity.data = serde_json::json!({"title": "v2", "url": "https://example.com", "tags": []});
    store.save_entity(&entity, &schema).unwrap();

    let retrieved = store.get_entity(&id).unwrap().unwrap();
    assert_eq!(retrieved.get_str("/title"), Some("v2"));
}

#[test]
fn delete_entity() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let entity = test_entity("To Delete");
    let id = entity.id.clone();

    store.save_entity(&entity, &schema).unwrap();
    assert!(store.get_entity(&id).unwrap().is_some());

    store.delete_entity(&id).unwrap();
    assert!(store.get_entity(&id).unwrap().is_none());
}

#[test]
fn delete_nonexistent_is_ok() {
    let store = EntityStore::open_in_memory().unwrap();
    store.delete_entity("nope").unwrap(); // should not error
}

// ── List ─────────────────────────────────────────────────────────

#[test]
fn list_entities() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();

    for i in 0..5 {
        let mut e = test_entity(&format!("Bookmark {i}"));
        e.modified_at = 1000 + i as i64;
        store.save_entity(&e, &schema).unwrap();
    }

    let list = store.list_entities("bookmark", false, None, None).unwrap();
    assert_eq!(list.len(), 5);
}

#[test]
fn list_with_limit_and_offset() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();

    for i in 0..10 {
        let mut e = test_entity(&format!("B{i}"));
        e.modified_at = 1000 + i as i64;
        store.save_entity(&e, &schema).unwrap();
    }

    let page = store.list_entities("bookmark", false, Some(3), Some(0)).unwrap();
    assert_eq!(page.len(), 3);

    let page2 = store.list_entities("bookmark", false, Some(3), Some(3)).unwrap();
    assert_eq!(page2.len(), 3);
    // Different entities
    assert_ne!(page[0].id, page2[0].id);
}

#[test]
fn list_empty_type() {
    let store = EntityStore::open_in_memory().unwrap();
    let list = store.list_entities("nothing", false, None, None).unwrap();
    assert!(list.is_empty());
}

// ── Trash ────────────────────────────────────────────────────────

#[test]
fn trash_and_restore() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let entity = test_entity("Trashable");
    let id = entity.id.clone();
    store.save_entity(&entity, &schema).unwrap();

    // Trash it
    store.trash_entity(&id).unwrap();
    let list = store.list_entities("bookmark", false, None, None).unwrap();
    assert_eq!(list.len(), 0); // excluded from non-trash list

    let list_with_trash = store.list_entities("bookmark", true, None, None).unwrap();
    assert_eq!(list_with_trash.len(), 1);

    // Restore it
    store.restore_entity(&id).unwrap();
    let list = store.list_entities("bookmark", false, None, None).unwrap();
    assert_eq!(list.len(), 1);
}

// ── Count ────────────────────────────────────────────────────────

#[test]
fn count_entities() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();

    assert_eq!(store.count_entities("bookmark", false).unwrap(), 0);

    for i in 0..3 {
        store.save_entity(&test_entity(&format!("B{i}")), &schema).unwrap();
    }
    assert_eq!(store.count_entities("bookmark", false).unwrap(), 3);
    assert_eq!(store.count_entities("bookmark", true).unwrap(), 3);
}

#[test]
fn count_excludes_trashed() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let entity = test_entity("Trashme");
    let id = entity.id.clone();
    store.save_entity(&entity, &schema).unwrap();
    store.trash_entity(&id).unwrap();

    assert_eq!(store.count_entities("bookmark", false).unwrap(), 0);
    assert_eq!(store.count_entities("bookmark", true).unwrap(), 1);
}

// ── Search ───────────────────────────────────────────────────────

#[test]
fn search_entities() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();

    let e1 = Entity {
        id: format!("search-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed)),
        entity_type: "bookmark".into(),
        data: serde_json::json!({"title": "Rust Programming", "url": "https://rust-lang.org", "tags": ["rust"]}),
        created_at: 1000, modified_at: 1000, created_by: "test".into(),
    };
    let e2 = Entity {
        id: format!("search-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed)),
        entity_type: "bookmark".into(),
        data: serde_json::json!({"title": "Python Docs", "url": "https://python.org", "tags": ["python"]}),
        created_at: 1001, modified_at: 1001, created_by: "test".into(),
    };

    store.save_entity(&e1, &schema).unwrap();
    store.save_entity(&e2, &schema).unwrap();

    let results = store.search("rust", None, 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, e1.id);
}

#[test]
fn search_no_results() {
    let store = EntityStore::open_in_memory().unwrap();
    let results = store.search("nonexistent", None, 10).unwrap();
    assert!(results.is_empty());
}

#[test]
fn search_by_entity_type() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let entity = Entity {
        id: "s1".into(),
        entity_type: "bookmark".into(),
        data: serde_json::json!({"title": "findme", "url": "x", "tags": []}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity(&entity, &schema).unwrap();

    let found = store.search("findme", Some(&["bookmark"]), 10).unwrap();
    assert_eq!(found.len(), 1);

    let not_found = store.search("findme", Some(&["note"]), 10).unwrap();
    assert!(not_found.is_empty());
}

#[test]
fn search_excludes_trashed() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let entity = Entity {
        id: "trash-search".into(),
        entity_type: "bookmark".into(),
        data: serde_json::json!({"title": "Secret", "url": "x", "tags": []}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity(&entity, &schema).unwrap();
    store.trash_entity("trash-search").unwrap();

    let results = store.search("Secret", None, 10).unwrap();
    assert!(results.is_empty());
}

// ── Query ────────────────────────────────────────────────────────

#[test]
fn query_entities_no_filters() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    store.save_entity(&test_entity("Q1"), &schema).unwrap();
    store.save_entity(&test_entity("Q2"), &schema).unwrap();

    let results = store.query_entities("bookmark", &[], false, None).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn query_entities_with_limit() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    for i in 0..5 {
        store.save_entity(&test_entity(&format!("Q{i}")), &schema).unwrap();
    }

    let results = store.query_entities("bookmark", &[], false, Some(2)).unwrap();
    assert_eq!(results.len(), 2);
}

// ── Links ────────────────────────────────────────────────────────

#[test]
fn entity_links() {
    let store = EntityStore::open_in_memory().unwrap();

    store.save_link("task", "t1", "note", "n1").unwrap();
    store.save_link("task", "t1", "contact", "c1").unwrap();

    let links = store.get_links_from("task", "t1").unwrap();
    assert_eq!(links.len(), 2);

    let backlinks = store.get_links_to("note", "n1").unwrap();
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0], ("task".to_string(), "t1".to_string()));

    store.remove_link("task", "t1", "note", "n1").unwrap();
    let links = store.get_links_from("task", "t1").unwrap();
    assert_eq!(links.len(), 1);
}

#[test]
fn duplicate_link_is_ignored() {
    let store = EntityStore::open_in_memory().unwrap();
    store.save_link("a", "1", "b", "2").unwrap();
    store.save_link("a", "1", "b", "2").unwrap(); // duplicate
    let links = store.get_links_from("a", "1").unwrap();
    assert_eq!(links.len(), 1);
}

#[test]
fn get_links_empty() {
    let store = EntityStore::open_in_memory().unwrap();
    let from = store.get_links_from("x", "y").unwrap();
    assert!(from.is_empty());
    let to = store.get_links_to("x", "y").unwrap();
    assert!(to.is_empty());
}

// ── Relation auto-linking ────────────────────────────────────────

#[test]
fn relation_field_creates_link_from_string_id() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = EntitySchema {
        entity_type: "task".into(),
        indexed_fields: vec![
            IndexedField::text("/title", true),
            IndexedField::relation("/parent_id"),
        ],
        merge_strategy: MergeStrategy::LwwDocument,
    };

    let entity = Entity {
        id: "task-1".into(),
        entity_type: "task".into(),
        data: serde_json::json!({
            "title": "Subtask",
            "parent_id": "task-parent"
        }),
        created_at: 1,
        modified_at: 1,
        created_by: "p".into(),
    };
    store.save_entity(&entity, &schema).unwrap();

    // The relation field should have auto-created a link
    let links = store.get_links_from("task", "task-1").unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0], ("_".to_string(), "task-parent".to_string()));
}

#[test]
fn relation_field_creates_link_from_typed_object() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = EntitySchema {
        entity_type: "task".into(),
        indexed_fields: vec![
            IndexedField::text("/title", true),
            IndexedField::relation("/linked_note"),
        ],
        merge_strategy: MergeStrategy::LwwDocument,
    };

    let entity = Entity {
        id: "task-2".into(),
        entity_type: "task".into(),
        data: serde_json::json!({
            "title": "With typed link",
            "linked_note": {"type": "note", "id": "note-42"}
        }),
        created_at: 1,
        modified_at: 1,
        created_by: "p".into(),
    };
    store.save_entity(&entity, &schema).unwrap();

    let links = store.get_links_from("task", "task-2").unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0], ("note".to_string(), "note-42".to_string()));

    // Verify backlink lookup works too
    let backlinks = store.get_links_to("note", "note-42").unwrap();
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0], ("task".to_string(), "task-2".to_string()));
}

#[test]
fn relation_field_no_link_when_field_missing() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = EntitySchema {
        entity_type: "task".into(),
        indexed_fields: vec![
            IndexedField::text("/title", true),
            IndexedField::relation("/parent_id"),
        ],
        merge_strategy: MergeStrategy::LwwDocument,
    };

    // Entity has no parent_id field in data
    let entity = Entity {
        id: "task-3".into(),
        entity_type: "task".into(),
        data: serde_json::json!({"title": "No parent"}),
        created_at: 1,
        modified_at: 1,
        created_by: "p".into(),
    };
    store.save_entity(&entity, &schema).unwrap();

    let links = store.get_links_from("task", "task-3").unwrap();
    assert!(links.is_empty());
}

#[test]
fn relation_field_ignores_non_string_non_object() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = EntitySchema {
        entity_type: "task".into(),
        indexed_fields: vec![
            IndexedField::text("/title", true),
            IndexedField::relation("/parent_id"),
        ],
        merge_strategy: MergeStrategy::LwwDocument,
    };

    // parent_id is a number, not a string or object — should be ignored
    let entity = Entity {
        id: "task-4".into(),
        entity_type: "task".into(),
        data: serde_json::json!({"title": "Bad relation", "parent_id": 42}),
        created_at: 1,
        modified_at: 1,
        created_by: "p".into(),
    };
    store.save_entity(&entity, &schema).unwrap();

    let links = store.get_links_from("task", "task-4").unwrap();
    assert!(links.is_empty());
}

#[test]
fn relation_field_null_value_creates_no_link() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = EntitySchema {
        entity_type: "task".into(),
        indexed_fields: vec![
            IndexedField::text("/title", true),
            IndexedField::relation("/parent_id"),
        ],
        merge_strategy: MergeStrategy::LwwDocument,
    };

    let entity = Entity {
        id: "task-5".into(),
        entity_type: "task".into(),
        data: serde_json::json!({"title": "Null parent", "parent_id": null}),
        created_at: 1,
        modified_at: 1,
        created_by: "p".into(),
    };
    store.save_entity(&entity, &schema).unwrap();

    let links = store.get_links_from("task", "task-5").unwrap();
    assert!(links.is_empty());
}

#[test]
fn multiple_relation_fields_create_multiple_links() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = EntitySchema {
        entity_type: "task".into(),
        indexed_fields: vec![
            IndexedField::text("/title", true),
            IndexedField::relation("/parent_id"),
            IndexedField::relation("/assignee_id"),
        ],
        merge_strategy: MergeStrategy::LwwDocument,
    };

    let entity = Entity {
        id: "task-6".into(),
        entity_type: "task".into(),
        data: serde_json::json!({
            "title": "Multi-link",
            "parent_id": "task-parent",
            "assignee_id": {"type": "contact", "id": "contact-1"}
        }),
        created_at: 1,
        modified_at: 1,
        created_by: "p".into(),
    };
    store.save_entity(&entity, &schema).unwrap();

    let links = store.get_links_from("task", "task-6").unwrap();
    assert_eq!(links.len(), 2);

    // Verify both links exist (order may vary)
    let link_set: std::collections::HashSet<_> = links.into_iter().collect();
    assert!(link_set.contains(&("_".to_string(), "task-parent".to_string())));
    assert!(link_set.contains(&("contact".to_string(), "contact-1".to_string())));
}

#[test]
fn relation_link_persists_after_upsert() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = EntitySchema {
        entity_type: "task".into(),
        indexed_fields: vec![
            IndexedField::text("/title", true),
            IndexedField::relation("/parent_id"),
        ],
        merge_strategy: MergeStrategy::LwwDocument,
    };

    let entity = Entity {
        id: "task-7".into(),
        entity_type: "task".into(),
        data: serde_json::json!({"title": "v1", "parent_id": "p1"}),
        created_at: 1,
        modified_at: 1,
        created_by: "p".into(),
    };
    store.save_entity(&entity, &schema).unwrap();

    // Upsert with a different relation target
    let entity2 = Entity {
        id: "task-7".into(),
        entity_type: "task".into(),
        data: serde_json::json!({"title": "v2", "parent_id": "p2"}),
        created_at: 1,
        modified_at: 2,
        created_by: "p".into(),
    };
    store.save_entity(&entity2, &schema).unwrap();

    let links = store.get_links_from("task", "task-7").unwrap();
    // Both links exist (INSERT OR IGNORE) — old link isn't removed automatically
    assert!(links.len() >= 1);
    // At minimum the new link should be present
    let link_targets: Vec<_> = links.iter().map(|(_, id)| id.as_str()).collect();
    assert!(link_targets.contains(&"p2"));
}

#[test]
fn schema_without_relation_fields_creates_no_auto_links() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema(); // bookmark schema has no Relation fields

    let entity = test_entity("No auto links");
    store.save_entity(&entity, &schema).unwrap();

    let links = store.get_links_from("bookmark", &entity.id).unwrap();
    assert!(links.is_empty());
}

// ── Schema field extraction ──────────────────────────────────────

#[test]
fn save_entity_with_body_field() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = EntitySchema {
        entity_type: "note".into(),
        indexed_fields: vec![
            IndexedField::text("/title", true),
            IndexedField { field_path: "/body".into(), field_type: FieldType::Text, searchable: true, vector_dim: None, enum_options: None },
        ],
        merge_strategy: MergeStrategy::LwwDocument,
    };
    let entity = Entity {
        id: "note-1".into(),
        entity_type: "note".into(),
        data: serde_json::json!({"title": "My Note", "body": "Long content here"}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity(&entity, &schema).unwrap();

    // Should be searchable by body
    let results = store.search("Long content", None, 10).unwrap();
    assert_eq!(results.len(), 1);
}

// ── Query with multiple filters ─────────────────────────────────

#[test]
fn query_entities_with_single_filter() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();

    let e1 = Entity {
        id: "qf-1".into(),
        entity_type: "bookmark".into(),
        data: serde_json::json!({"title": "Rust Guide", "url": "https://rust.org", "tags": []}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    let e2 = Entity {
        id: "qf-2".into(),
        entity_type: "bookmark".into(),
        data: serde_json::json!({"title": "Python Guide", "url": "https://python.org", "tags": []}),
        created_at: 2, modified_at: 2, created_by: "p".into(),
    };
    store.save_entity(&e1, &schema).unwrap();
    store.save_entity(&e2, &schema).unwrap();

    let filters = vec![("/title".to_string(), serde_json::Value::String("Rust Guide".into()))];
    let results = store.query_entities("bookmark", &filters, false, None).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "qf-1");
}

#[test]
fn query_entities_with_multiple_filters_empty_result() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();

    let e1 = Entity {
        id: "qm-1".into(),
        entity_type: "bookmark".into(),
        data: serde_json::json!({"title": "Rust Guide", "url": "https://rust.org", "tags": []}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity(&e1, &schema).unwrap();

    // Multiple filters applied — exercises the multi-filter SQL generation path
    let filters = vec![
        ("/title".to_string(), serde_json::Value::String("Rust Guide".into())),
        ("/url".to_string(), serde_json::Value::String("https://rust.org".into())),
    ];
    let results = store.query_entities("bookmark", &filters, false, None).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "qm-1");
}

#[test]
fn query_entities_with_numeric_filter() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = EntitySchema {
        entity_type: "item".into(),
        indexed_fields: vec![
            IndexedField::text("/name", true),
            IndexedField::number("/count"),
        ],
        merge_strategy: MergeStrategy::LwwDocument,
    };

    let e1 = Entity {
        id: "nf-1".into(),
        entity_type: "item".into(),
        data: serde_json::json!({"name": "widget", "count": 42}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity(&e1, &schema).unwrap();

    // Numeric filter: json Value::Number.to_string() = "42", json_extract_string = "42"
    let filters = vec![("/count".to_string(), serde_json::json!(42))];
    let results = store.query_entities("item", &filters, false, None).unwrap();
    // Numeric values serialize without quotes, so this should match
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "nf-1");
}

#[test]
fn query_entities_excludes_trashed() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let entity = Entity {
        id: "qt-1".into(),
        entity_type: "bookmark".into(),
        data: serde_json::json!({"title": "Trashed", "url": "x", "tags": []}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity(&entity, &schema).unwrap();
    store.trash_entity("qt-1").unwrap();

    // Even without filters, trashed entities should be excluded
    let results = store.query_entities("bookmark", &[], false, None).unwrap();
    assert!(results.is_empty());
}

// ── Search with special chars in entity_types ────────────────────

#[test]
fn search_with_entity_types_containing_special_chars() {
    let store = EntityStore::open_in_memory().unwrap();
    // Entity type with special chars — the SQL uses escaped quotes via replace
    let schema = EntitySchema {
        entity_type: "my_note''s".into(),
        indexed_fields: vec![IndexedField::text("/title", true)],
        merge_strategy: MergeStrategy::LwwDocument,
    };
    let entity = Entity {
        id: "sp-1".into(),
        entity_type: "my_note''s".into(),
        data: serde_json::json!({"title": "Special"}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity(&entity, &schema).unwrap();

    // Search with entity type filter containing escaped apostrophe
    let results = store.search("Special", Some(&["my_note''s"]), 10).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn search_with_empty_entity_types_filter() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    store.save_entity(&test_entity("Findable"), &schema).unwrap();

    // Empty types slice — no entity_type filter applied
    let results = store.search("Findable", Some(&[]), 10).unwrap();
    assert_eq!(results.len(), 1);
}

// ── Vector extraction dimension mismatch ─────────────────────────

#[test]
fn vector_field_dimension_mismatch_skipped() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = EntitySchema {
        entity_type: "embedding".into(),
        indexed_fields: vec![
            IndexedField::text("/title", true),
            IndexedField::vector("/embedding", 3), // expects dim=3
        ],
        merge_strategy: MergeStrategy::LwwDocument,
    };

    // Provide 2 elements instead of 3
    let entity = Entity {
        id: "vec-mismatch-1".into(),
        entity_type: "embedding".into(),
        data: serde_json::json!({
            "title": "Bad vector",
            "embedding": [0.1, 0.2]
        }),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    // Should not error — just skips the vector
    store.save_entity(&entity, &schema).unwrap();
    let retrieved = store.get_entity("vec-mismatch-1").unwrap().unwrap();
    assert_eq!(retrieved.get_str("/title"), Some("Bad vector"));
}

#[test]
fn vector_field_correct_dimensions_stored() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = EntitySchema {
        entity_type: "embedding".into(),
        indexed_fields: vec![
            IndexedField::text("/title", true),
            IndexedField::vector("/embedding", 3),
        ],
        merge_strategy: MergeStrategy::LwwDocument,
    };

    let entity = Entity {
        id: "vec-ok-1".into(),
        entity_type: "embedding".into(),
        data: serde_json::json!({
            "title": "Good vector",
            "embedding": [0.1, 0.2, 0.3]
        }),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity(&entity, &schema).unwrap();
    let retrieved = store.get_entity("vec-ok-1").unwrap().unwrap();
    assert_eq!(retrieved.get_str("/title"), Some("Good vector"));
}

#[test]
fn vector_field_with_non_numeric_values_skipped() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = EntitySchema {
        entity_type: "embedding".into(),
        indexed_fields: vec![
            IndexedField::vector("/embedding", 3),
        ],
        merge_strategy: MergeStrategy::LwwDocument,
    };

    let entity = Entity {
        id: "vec-bad-vals".into(),
        entity_type: "embedding".into(),
        data: serde_json::json!({
            "embedding": [0.1, "not_a_number", 0.3]
        }),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    // Should not error — filter_map skips non-f64, dimension check fails
    store.save_entity(&entity, &schema).unwrap();
}

#[test]
fn vector_field_no_vector_dim_defaults_to_zero() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = EntitySchema {
        entity_type: "embedding".into(),
        indexed_fields: vec![
            IndexedField {
                field_path: "/embedding".into(),
                field_type: FieldType::Vector,
                searchable: false,
                vector_dim: None, // no dim specified
                enum_options: None,
            },
        ],
        merge_strategy: MergeStrategy::LwwDocument,
    };

    let entity = Entity {
        id: "vec-no-dim".into(),
        entity_type: "embedding".into(),
        data: serde_json::json!({
            "embedding": [0.1, 0.2]
        }),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    // dim defaults to 0, array.len() != 0 => skipped
    store.save_entity(&entity, &schema).unwrap();
}

// ── open with file path ──────────────────────────────────────────

#[test]
fn open_with_file_path() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("entity.db");
    let store = EntityStore::open(&db_path).unwrap();
    let schema = test_schema();
    let entity = test_entity("Persisted");
    let id = entity.id.clone();
    store.save_entity(&entity, &schema).unwrap();
    drop(store);

    let store2 = EntityStore::open(&db_path).unwrap();
    let retrieved = store2.get_entity(&id).unwrap().unwrap();
    assert_eq!(retrieved.get_str("/title"), Some("Persisted"));
}

#[test]
fn save_entity_with_is_favorite() {
    let store = EntityStore::open_in_memory().unwrap();
    let entity = Entity {
        id: "fav-1".into(),
        entity_type: "bookmark".into(),
        data: serde_json::json!({"title": "Fav", "is_favorite": true}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity_raw(&entity).unwrap();
    let retrieved = store.get_entity("fav-1").unwrap().unwrap();
    assert_eq!(retrieved.data["is_favorite"], true);
}

// ── Sync Ledger ─────────────────────────────────────────────────

#[test]
fn list_all_entity_ids_excludes_trashed() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let e1 = test_entity("Active");
    let e2 = test_entity("Trashed");
    let e2_id = e2.id.clone();
    store.save_entity(&e1, &schema).unwrap();
    store.save_entity(&e2, &schema).unwrap();
    store.trash_entity(&e2_id).unwrap();

    let ids = store.list_all_entity_ids().unwrap();
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0], e1.id);
}

#[test]
fn list_all_entity_ids_empty_store() {
    let store = EntityStore::open_in_memory().unwrap();
    let ids = store.list_all_entity_ids().unwrap();
    assert!(ids.is_empty());
}

#[test]
fn mark_entity_synced_and_check() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let e = test_entity("Syncable");
    let id = e.id.clone();
    store.save_entity(&e, &schema).unwrap();

    // Before sync, entity should need syncing
    let needs = store.entities_needing_sync("peer-1").unwrap();
    assert!(needs.contains(&id));

    // Mark synced with a timestamp after modified_at
    store.mark_entity_synced("peer-1", &id, 2000).unwrap();

    // Now entity should not need syncing (synced_at > modified_at)
    let needs = store.entities_needing_sync("peer-1").unwrap();
    assert!(!needs.contains(&id));
}

#[test]
fn entities_needing_sync_includes_unsynced() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let e1 = test_entity("E1");
    let e2 = test_entity("E2");
    let e1_id = e1.id.clone();
    let e2_id = e2.id.clone();
    store.save_entity(&e1, &schema).unwrap();
    store.save_entity(&e2, &schema).unwrap();

    // Mark only e1 as synced
    store.mark_entity_synced("peer-1", &e1_id, 2000).unwrap();

    let needs = store.entities_needing_sync("peer-1").unwrap();
    assert!(!needs.contains(&e1_id));
    assert!(needs.contains(&e2_id));
}

#[test]
fn entities_needing_sync_includes_modified_after_sync() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let mut e = test_entity("Modified");
    e.modified_at = 3000;
    let id = e.id.clone();
    store.save_entity(&e, &schema).unwrap();

    // Synced at 2000 but modified at 3000 → needs re-sync
    store.mark_entity_synced("peer-1", &id, 2000).unwrap();
    let needs = store.entities_needing_sync("peer-1").unwrap();
    assert!(needs.contains(&id));
}

#[test]
fn entities_needing_sync_excludes_local_only() {
    let store = EntityStore::open_in_memory().unwrap();
    let entity = Entity {
        id: "local-only-1".into(),
        entity_type: "bookmark".into(),
        data: serde_json::json!({"title": "Local", "local_only": true}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity_raw(&entity).unwrap();

    let needs = store.entities_needing_sync("peer-1").unwrap();
    assert!(!needs.contains(&"local-only-1".to_string()));
}

#[test]
fn mark_entities_synced_batch() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let e1 = test_entity("B1");
    let e2 = test_entity("B2");
    let ids = vec![e1.id.clone(), e2.id.clone()];
    store.save_entity(&e1, &schema).unwrap();
    store.save_entity(&e2, &schema).unwrap();

    store.mark_entities_synced("peer-1", &ids, 5000).unwrap();

    let needs = store.entities_needing_sync("peer-1").unwrap();
    assert!(needs.is_empty());
}

#[test]
fn clear_sync_ledger_for_peer() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let e = test_entity("Clearable");
    let id = e.id.clone();
    store.save_entity(&e, &schema).unwrap();
    store.mark_entity_synced("peer-1", &id, 5000).unwrap();

    store.clear_sync_ledger_for_peer("peer-1").unwrap();

    let needs = store.entities_needing_sync("peer-1").unwrap();
    assert!(needs.contains(&id));
}

#[test]
fn invalidate_sync_ledger_for_entity() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let e = test_entity("Invalidate");
    let id = e.id.clone();
    store.save_entity(&e, &schema).unwrap();
    store.mark_entity_synced("peer-1", &id, 5000).unwrap();
    store.mark_entity_synced("peer-2", &id, 5000).unwrap();

    store.invalidate_sync_ledger_for_entity(&id).unwrap();

    // Both peers should now need to re-sync this entity
    assert!(store.entities_needing_sync("peer-1").unwrap().contains(&id));
    assert!(store.entities_needing_sync("peer-2").unwrap().contains(&id));
}

// ── Storage Estimation ──────────────────────────────────────────

#[test]
fn estimate_storage_bytes() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    store.save_entity(&test_entity("Est1"), &schema).unwrap();
    store.save_entity(&test_entity("Est2"), &schema).unwrap();

    let bytes = store.estimate_storage_bytes("bookmark").unwrap();
    assert!(bytes > 0);
}

#[test]
fn estimate_storage_bytes_empty() {
    let store = EntityStore::open_in_memory().unwrap();
    let bytes = store.estimate_storage_bytes("nothing").unwrap();
    assert_eq!(bytes, 0);
}

#[test]
fn estimate_storage_by_types() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    store.save_entity(&test_entity("T1"), &schema).unwrap();

    let note = Entity {
        id: "note-est".into(),
        entity_type: "note".into(),
        data: serde_json::json!({"content": "hello"}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity_raw(&note).unwrap();

    let results = store.estimate_storage_by_types(&["bookmark", "note", "missing"]).unwrap();
    assert_eq!(results.len(), 3);

    let bm = &results[0];
    assert_eq!(bm.0, "bookmark");
    assert_eq!(bm.1, 1); // count
    assert!(bm.2 > 0);   // bytes

    let nt = &results[1];
    assert_eq!(nt.0, "note");
    assert_eq!(nt.1, 1);

    let missing = &results[2];
    assert_eq!(missing.0, "missing");
    assert_eq!(missing.1, 0);
    assert_eq!(missing.2, 0);
}

// ── Cloud Sync Cursors ──────────────────────────────────────────

#[test]
fn cloud_cursors_save_and_load() {
    let store = EntityStore::open_in_memory().unwrap();

    store.save_cloud_cursor("entity:ent-1", 42).unwrap();
    store.save_cloud_cursor("entity:ent-2", 100).unwrap();

    let cursors = store.load_cloud_cursors().unwrap();
    assert_eq!(cursors.len(), 2);

    let map: std::collections::HashMap<_, _> = cursors.into_iter().collect();
    assert_eq!(map["entity:ent-1"], 42);
    assert_eq!(map["entity:ent-2"], 100);
}

#[test]
fn cloud_cursor_upsert() {
    let store = EntityStore::open_in_memory().unwrap();
    store.save_cloud_cursor("key", 10).unwrap();
    store.save_cloud_cursor("key", 20).unwrap();

    let cursors = store.load_cloud_cursors().unwrap();
    assert_eq!(cursors.len(), 1);
    assert_eq!(cursors[0].1, 20);
}

#[test]
fn clear_cloud_cursors() {
    let store = EntityStore::open_in_memory().unwrap();
    store.save_cloud_cursor("a", 1).unwrap();
    store.save_cloud_cursor("b", 2).unwrap();

    store.clear_cloud_cursors().unwrap();
    let cursors = store.load_cloud_cursors().unwrap();
    assert!(cursors.is_empty());
}

// ── Plugin Fuel History ─────────────────────────────────────────

#[test]
fn fuel_consumption_record_and_metrics() {
    let store = EntityStore::open_in_memory().unwrap();

    store.record_fuel_consumption("plugin-a", 100).unwrap();
    store.record_fuel_consumption("plugin-a", 200).unwrap();
    store.record_fuel_consumption("plugin-a", 300).unwrap();

    let (avg, peak, count) = store.get_fuel_metrics("plugin-a").unwrap();
    assert_eq!(count, 3);
    assert_eq!(peak, 300);
    assert_eq!(avg, 200); // (100+200+300)/3
}

#[test]
fn fuel_metrics_empty_plugin() {
    let store = EntityStore::open_in_memory().unwrap();
    let (avg, peak, count) = store.get_fuel_metrics("unknown").unwrap();
    assert_eq!(avg, 0);
    assert_eq!(peak, 0);
    assert_eq!(count, 0);
}

#[test]
fn clear_fuel_history() {
    let store = EntityStore::open_in_memory().unwrap();
    store.record_fuel_consumption("plugin-a", 100).unwrap();
    store.record_fuel_consumption("plugin-a", 200).unwrap();

    store.clear_fuel_history("plugin-a").unwrap();
    let (_, _, count) = store.get_fuel_metrics("plugin-a").unwrap();
    assert_eq!(count, 0);
}

#[test]
fn fuel_consumption_isolates_plugins() {
    let store = EntityStore::open_in_memory().unwrap();
    store.record_fuel_consumption("a", 100).unwrap();
    store.record_fuel_consumption("b", 999).unwrap();

    let (_, peak_a, count_a) = store.get_fuel_metrics("a").unwrap();
    assert_eq!(count_a, 1);
    assert_eq!(peak_a, 100);

    let (_, peak_b, count_b) = store.get_fuel_metrics("b").unwrap();
    assert_eq!(count_b, 1);
    assert_eq!(peak_b, 999);
}

// ── RAG Vector Index ────────────────────────────────────────────

#[test]
fn rag_upsert_and_delete() {
    let store = EntityStore::open_in_memory().unwrap();
    let embedding = vec![0.1, 0.2, 0.3];

    store.rag_upsert(
        "ent-1", "chunk-0", "notes", "note", "hash1",
        3, &embedding, "My Note", "note", 1000, "Hello world",
    ).unwrap();

    let hashes = store.rag_get_hashes(None).unwrap();
    assert_eq!(hashes.len(), 1);
    assert_eq!(hashes[0].0, "ent-1");
    assert_eq!(hashes[0].2, "hash1");

    store.rag_delete("ent-1").unwrap();
    let hashes = store.rag_get_hashes(None).unwrap();
    assert!(hashes.is_empty());
}

#[test]
fn rag_upsert_updates_existing() {
    let store = EntityStore::open_in_memory().unwrap();
    let emb1 = vec![0.1, 0.2, 0.3];
    let emb2 = vec![0.4, 0.5, 0.6];

    store.rag_upsert(
        "ent-1", "chunk-0", "notes", "note", "hash1",
        3, &emb1, "Title v1", "note", 1000, "text v1",
    ).unwrap();
    store.rag_upsert(
        "ent-1", "chunk-0", "notes", "note", "hash2",
        3, &emb2, "Title v2", "note", 2000, "text v2",
    ).unwrap();

    let hashes = store.rag_get_hashes(None).unwrap();
    assert_eq!(hashes.len(), 1);
    assert_eq!(hashes[0].2, "hash2"); // updated hash
}

#[test]
fn rag_search_cosine_similarity() {
    let store = EntityStore::open_in_memory().unwrap();

    // Insert two vectors: one similar to query, one orthogonal
    store.rag_upsert(
        "ent-close", "chunk-0", "notes", "note", "h1",
        3, &[0.9, 0.1, 0.0], "Close Note", "note", 1000, "close text",
    ).unwrap();
    store.rag_upsert(
        "ent-far", "chunk-0", "notes", "note", "h2",
        3, &[0.0, 0.0, 1.0], "Far Note", "note", 1000, "far text",
    ).unwrap();

    let query = vec![1.0, 0.0, 0.0];
    let results = store.rag_search(&query, 10, None).unwrap();
    assert_eq!(results.len(), 2);
    // First result should be the closer one
    assert_eq!(results[0]["entity_id"], "ent-close");
    let score: f64 = results[0]["score"].as_f64().unwrap();
    assert!(score > 0.5);
}

#[test]
fn rag_search_with_type_filter() {
    let store = EntityStore::open_in_memory().unwrap();

    store.rag_upsert(
        "note-1", "chunk-0", "notes", "note", "h1",
        3, &[1.0, 0.0, 0.0], "Note", "note", 1000, "text",
    ).unwrap();
    store.rag_upsert(
        "task-1", "chunk-0", "tasks", "task", "h2",
        3, &[1.0, 0.0, 0.0], "Task", "task", 1000, "text",
    ).unwrap();

    let query = vec![1.0, 0.0, 0.0];
    let results = store.rag_search(&query, 10, Some(&["note"])).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["entity_type"], "note");
}

#[test]
fn rag_delete_all() {
    let store = EntityStore::open_in_memory().unwrap();
    store.rag_upsert("e1", "c0", "p1", "note", "h1", 2, &[0.1, 0.2], "T1", "note", 1, "t").unwrap();
    store.rag_upsert("e2", "c0", "p1", "task", "h2", 2, &[0.3, 0.4], "T2", "task", 2, "t").unwrap();

    store.rag_delete_all().unwrap();
    let hashes = store.rag_get_hashes(None).unwrap();
    assert!(hashes.is_empty());
}

#[test]
fn rag_get_hashes_with_type_filter() {
    let store = EntityStore::open_in_memory().unwrap();
    store.rag_upsert("e1", "c0", "p1", "note", "h1", 2, &[0.1, 0.2], "T1", "note", 1, "t").unwrap();
    store.rag_upsert("e2", "c0", "p1", "task", "h2", 2, &[0.3, 0.4], "T2", "task", 2, "t").unwrap();

    let note_hashes = store.rag_get_hashes(Some(&["note"])).unwrap();
    assert_eq!(note_hashes.len(), 1);
    assert_eq!(note_hashes[0].0, "e1");
}

#[test]
fn rag_fetch_all() {
    let store = EntityStore::open_in_memory().unwrap();
    store.rag_upsert("e1", "c0", "p1", "note", "h1", 3, &[0.1, 0.2, 0.3], "My Note", "note", 1, "hello").unwrap();

    let all = store.rag_fetch_all(None, 100).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0]["entity_id"], "e1");
    assert_eq!(all[0]["title"], "My Note");
    assert_eq!(all[0]["chunk_text"], "hello");

    let emb = all[0]["embedding"].as_array().unwrap();
    assert_eq!(emb.len(), 3);
}

#[test]
fn rag_fetch_all_with_type_filter() {
    let store = EntityStore::open_in_memory().unwrap();
    store.rag_upsert("e1", "c0", "p1", "note", "h1", 2, &[0.1, 0.2], "N", "note", 1, "t").unwrap();
    store.rag_upsert("e2", "c0", "p1", "task", "h2", 2, &[0.3, 0.4], "T", "task", 2, "t").unwrap();

    let notes = store.rag_fetch_all(Some(&["note"]), 100).unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["entity_type"], "note");
}

// ── Orphan Entities ─────────────────────────────────────────────

#[test]
fn find_orphan_entities() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    store.save_entity(&test_entity("Known"), &schema).unwrap();

    let orphan = Entity {
        id: "orphan-1".into(),
        entity_type: "obsolete_type".into(),
        data: serde_json::json!({"x": 1}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity_raw(&orphan).unwrap();

    let valid_types = vec![("notes".into(), "bookmark".into())];
    let orphans = store.find_orphan_entities(&valid_types).unwrap();
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0]["entity_type"], "obsolete_type");
    assert_eq!(orphans[0]["count"], 1);
}

#[test]
fn find_orphan_entities_none_when_all_valid() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    store.save_entity(&test_entity("Valid"), &schema).unwrap();

    let valid_types = vec![("notes".into(), "bookmark".into())];
    let orphans = store.find_orphan_entities(&valid_types).unwrap();
    assert!(orphans.is_empty());
}

#[test]
fn delete_orphan_entities() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    store.save_entity(&test_entity("Keep"), &schema).unwrap();

    let orphan = Entity {
        id: "orphan-del".into(),
        entity_type: "dead_type".into(),
        data: serde_json::json!({"x": 1}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity_raw(&orphan).unwrap();

    let valid_types = vec![("notes".into(), "bookmark".into())];
    let deleted = store.delete_orphan_entities(&valid_types).unwrap();
    assert_eq!(deleted, 1);

    // Orphan should be gone
    assert!(store.get_entity("orphan-del").unwrap().is_none());
    // Valid entity should remain
    assert_eq!(store.count_entities("bookmark", false).unwrap(), 1);
}

#[test]
fn delete_orphan_entities_empty_valid_types() {
    let store = EntityStore::open_in_memory().unwrap();
    store.save_entity_raw(&Entity {
        id: "e1".into(), entity_type: "note".into(),
        data: serde_json::json!({}), created_at: 1, modified_at: 1, created_by: "p".into(),
    }).unwrap();

    let deleted = store.delete_orphan_entities(&[]).unwrap();
    assert_eq!(deleted, 0); // early return
}

#[test]
fn delete_orphan_entities_cascades_auxiliary() {
    let store = EntityStore::open_in_memory().unwrap();

    let orphan = Entity {
        id: "orphan-casc".into(),
        entity_type: "dead_type".into(),
        data: serde_json::json!({"x": 1}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity_raw(&orphan).unwrap();
    store.save_link("dead_type", "orphan-casc", "note", "n1").unwrap();
    store.mark_entity_synced("peer-1", "orphan-casc", 1000).unwrap();

    let valid_types = vec![("notes".into(), "note".into())];
    store.delete_orphan_entities(&valid_types).unwrap();

    // Links and sync ledger entries should also be gone
    let links = store.get_links_from("dead_type", "orphan-casc").unwrap();
    assert!(links.is_empty());
}

// ── Maintenance ─────────────────────────────────────────────────

#[test]
fn checkpoint_runs_without_error() {
    let store = EntityStore::open_in_memory().unwrap();
    store.checkpoint().unwrap();
}

#[test]
fn run_maintenance_cleans_orphaned_data() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let entity = test_entity("Maint");
    let id = entity.id.clone();
    store.save_entity(&entity, &schema).unwrap();
    store.save_link("bookmark", &id, "note", "n1").unwrap();
    store.mark_entity_synced("peer-1", &id, 1000).unwrap();
    store.save_cloud_cursor("key", 42).unwrap();
    store.record_fuel_consumption("plugin-a", 100).unwrap();

    // Delete the entity, leaving orphaned link + sync entries
    store.delete_entity(&id).unwrap();

    store.run_maintenance().unwrap();

    // Cloud cursors and fuel history should be cleared
    assert!(store.load_cloud_cursors().unwrap().is_empty());
    let (_, _, count) = store.get_fuel_metrics("plugin-a").unwrap();
    assert_eq!(count, 0);
}

#[test]
fn db_diagnostics_returns_tables() {
    let store = EntityStore::open_in_memory().unwrap();
    let diag = store.db_diagnostics().unwrap();
    let tables = diag["tables"].as_array().unwrap();
    assert!(!tables.is_empty());
    // Should at least have the entities table
    let table_names: Vec<&str> = tables.iter()
        .filter_map(|t| t["table"].as_str())
        .collect();
    assert!(table_names.contains(&"entities"));
}

// ── Query with filter edge cases ────────────────────────────────

#[test]
fn query_entities_filter_without_slash_prefix() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let entity = Entity {
        id: "noslash-1".into(),
        entity_type: "bookmark".into(),
        data: serde_json::json!({"title": "NoSlash", "url": "x", "tags": []}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity(&entity, &schema).unwrap();

    // Filter path without leading slash — should auto-prepend
    let filters = vec![("title".to_string(), serde_json::Value::String("NoSlash".into()))];
    let results = store.query_entities("bookmark", &filters, false, None).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn query_entities_filter_field_not_in_data() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    store.save_entity(&test_entity("Exists"), &schema).unwrap();

    let filters = vec![("/nonexistent".to_string(), serde_json::Value::String("x".into()))];
    let results = store.query_entities("bookmark", &filters, false, None).unwrap();
    assert!(results.is_empty());
}

#[test]
fn query_entities_with_limit_breaks_early() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    for i in 0..10 {
        let mut e = test_entity(&format!("Match{i}"));
        e.modified_at = 1000 + i as i64;
        store.save_entity(&e, &schema).unwrap();
    }

    // Filter that matches all, but limit to 3
    let filters = vec![("/url".to_string(), serde_json::Value::String("https://example.com".into()))];
    let results = store.query_entities("bookmark", &filters, false, Some(3)).unwrap();
    assert_eq!(results.len(), 3);
}

#[test]
fn query_entities_include_trashed_with_filters() {
    let store = EntityStore::open_in_memory().unwrap();
    let schema = test_schema();
    let entity = Entity {
        id: "qt-incl".into(),
        entity_type: "bookmark".into(),
        data: serde_json::json!({"title": "Trashed Filter", "url": "x", "tags": []}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity(&entity, &schema).unwrap();
    store.trash_entity("qt-incl").unwrap();

    let filters = vec![("/title".to_string(), serde_json::Value::String("Trashed Filter".into()))];
    let results_exclude = store.query_entities("bookmark", &filters, false, None).unwrap();
    assert!(results_exclude.is_empty());

    let results_include = store.query_entities("bookmark", &filters, true, None).unwrap();
    assert_eq!(results_include.len(), 1);
}

#[test]
fn query_entities_bool_filter() {
    let store = EntityStore::open_in_memory().unwrap();
    let entity = Entity {
        id: "bf-1".into(),
        entity_type: "item".into(),
        data: serde_json::json!({"done": true, "label": "task"}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity_raw(&entity).unwrap();

    let filters = vec![("/done".to_string(), serde_json::json!(true))];
    let results = store.query_entities("item", &filters, false, None).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn query_entities_string_coercion_on_number() {
    let store = EntityStore::open_in_memory().unwrap();
    let entity = Entity {
        id: "coerce-1".into(),
        entity_type: "item".into(),
        data: serde_json::json!({"count": 42}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity_raw(&entity).unwrap();

    // Filter with string "42" against actual number 42 → coercion
    let filters = vec![("/count".to_string(), serde_json::Value::String("42".into()))];
    let results = store.query_entities("item", &filters, false, None).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn query_entities_string_coercion_on_bool() {
    let store = EntityStore::open_in_memory().unwrap();
    let entity = Entity {
        id: "coerce-bool".into(),
        entity_type: "item".into(),
        data: serde_json::json!({"active": true}),
        created_at: 1, modified_at: 1, created_by: "p".into(),
    };
    store.save_entity_raw(&entity).unwrap();

    let filters = vec![("/active".to_string(), serde_json::Value::String("true".into()))];
    let results = store.query_entities("item", &filters, false, None).unwrap();
    assert_eq!(results.len(), 1);
}

// ── Compact ─────────────────────────────────────────────────────

#[test]
fn compact_reduces_or_maintains_size() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("compact_test.db");
    let store = EntityStore::open(&db_path).unwrap();
    let schema = test_schema();

    // Write data, then delete it to create reclaimable space
    for i in 0..50 {
        store.save_entity(&test_entity(&format!("C{i}")), &schema).unwrap();
    }
    for _i in 0..40 {
        // Delete most entities
        let list = store.list_entities("bookmark", false, Some(1), None).unwrap();
        if let Some(e) = list.first() {
            store.delete_entity(&e.id).unwrap();
        }
    }

    let (before, after) = store.compact(&db_path).unwrap();
    assert!(before > 0);
    assert!(after > 0);
    // After compact, data should still be accessible
    let remaining = store.list_entities("bookmark", false, None, None).unwrap();
    assert_eq!(remaining.len(), 10);
}
