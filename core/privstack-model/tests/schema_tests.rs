use privstack_model::{EntitySchema, FieldType, IndexedField, MergeStrategy};

// ── IndexedField constructors ────────────────────────────────────

#[test]
fn text_field_searchable() {
    let f = IndexedField::text("/title", true);
    assert_eq!(f.field_path, "/title");
    assert_eq!(f.field_type, FieldType::Text);
    assert!(f.searchable);
}

#[test]
fn text_field_not_searchable() {
    let f = IndexedField::text("/icon", false);
    assert_eq!(f.field_path, "/icon");
    assert_eq!(f.field_type, FieldType::Text);
    assert!(!f.searchable);
}

#[test]
fn tag_field_always_searchable() {
    let f = IndexedField::tag("/tags");
    assert_eq!(f.field_path, "/tags");
    assert_eq!(f.field_type, FieldType::Tag);
    assert!(f.searchable);
}

#[test]
fn datetime_field_not_searchable() {
    let f = IndexedField::datetime("/created_at");
    assert_eq!(f.field_path, "/created_at");
    assert_eq!(f.field_type, FieldType::DateTime);
    assert!(!f.searchable);
}

#[test]
fn number_field_not_searchable() {
    let f = IndexedField::number("/price");
    assert_eq!(f.field_path, "/price");
    assert_eq!(f.field_type, FieldType::Number);
    assert!(!f.searchable);
}

#[test]
fn bool_field_not_searchable() {
    let f = IndexedField::bool("/done");
    assert_eq!(f.field_path, "/done");
    assert_eq!(f.field_type, FieldType::Bool);
    assert!(!f.searchable);
}

// ── FieldType equality ───────────────────────────────────────────

#[test]
fn field_type_equality() {
    assert_eq!(FieldType::Text, FieldType::Text);
    assert_ne!(FieldType::Text, FieldType::Tag);
    assert_ne!(FieldType::Number, FieldType::Bool);
    assert_ne!(FieldType::DateTime, FieldType::Text);
}

#[test]
fn field_type_clone() {
    let ft = FieldType::Tag;
    let ft2 = ft.clone();
    assert_eq!(ft, ft2);
}

// ── MergeStrategy ────────────────────────────────────────────────

#[test]
fn merge_strategy_equality() {
    assert_eq!(MergeStrategy::LwwDocument, MergeStrategy::LwwDocument);
    assert_eq!(MergeStrategy::LwwPerField, MergeStrategy::LwwPerField);
    assert_eq!(MergeStrategy::Custom, MergeStrategy::Custom);
    assert_ne!(MergeStrategy::LwwDocument, MergeStrategy::LwwPerField);
    assert_ne!(MergeStrategy::LwwDocument, MergeStrategy::Custom);
}

#[test]
fn merge_strategy_copy() {
    let ms = MergeStrategy::LwwPerField;
    let ms2 = ms; // Copy
    assert_eq!(ms, ms2);
}

// ── EntitySchema ─────────────────────────────────────────────────

fn make_note_schema() -> EntitySchema {
    EntitySchema {
        entity_type: "note".to_string(),
        indexed_fields: vec![
            IndexedField::text("/title", true),
            IndexedField::text("/body", true),
            IndexedField::tag("/tags"),
            IndexedField::datetime("/created_at"),
        ],
        merge_strategy: MergeStrategy::LwwPerField,
    }
}

#[test]
fn schema_has_correct_entity_type() {
    let s = make_note_schema();
    assert_eq!(s.entity_type, "note");
}

#[test]
fn schema_field_count() {
    let s = make_note_schema();
    assert_eq!(s.indexed_fields.len(), 4);
}

#[test]
fn schema_merge_strategy() {
    let s = make_note_schema();
    assert_eq!(s.merge_strategy, MergeStrategy::LwwPerField);
}

#[test]
fn schema_searchable_fields() {
    let s = make_note_schema();
    let searchable: Vec<&str> = s
        .indexed_fields
        .iter()
        .filter(|f| f.searchable)
        .map(|f| f.field_path.as_str())
        .collect();
    assert_eq!(searchable, vec!["/title", "/body", "/tags"]);
}

#[test]
fn schema_fields_by_type() {
    let s = make_note_schema();
    let text_fields: Vec<&str> = s
        .indexed_fields
        .iter()
        .filter(|f| f.field_type == FieldType::Text)
        .map(|f| f.field_path.as_str())
        .collect();
    assert_eq!(text_fields, vec!["/title", "/body"]);
}

// ── Serde roundtrips ─────────────────────────────────────────────

#[test]
fn indexed_field_serde_roundtrip() {
    let original = IndexedField::text("/title", true);
    let json = serde_json::to_string(&original).unwrap();
    let parsed: IndexedField = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.field_path, original.field_path);
    assert_eq!(parsed.field_type, original.field_type);
    assert_eq!(parsed.searchable, original.searchable);
}

#[test]
fn field_type_serde_uses_snake_case() {
    let json = serde_json::to_string(&FieldType::DateTime).unwrap();
    assert_eq!(json, "\"date_time\"");

    let json = serde_json::to_string(&FieldType::Text).unwrap();
    assert_eq!(json, "\"text\"");
}

#[test]
fn merge_strategy_serde_uses_snake_case() {
    let json = serde_json::to_string(&MergeStrategy::LwwDocument).unwrap();
    assert_eq!(json, "\"lww_document\"");

    let json = serde_json::to_string(&MergeStrategy::LwwPerField).unwrap();
    assert_eq!(json, "\"lww_per_field\"");

    let json = serde_json::to_string(&MergeStrategy::Custom).unwrap();
    assert_eq!(json, "\"custom\"");
}

#[test]
fn entity_schema_serde_roundtrip() {
    let original = make_note_schema();
    let json = serde_json::to_string(&original).unwrap();
    let parsed: EntitySchema = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.entity_type, original.entity_type);
    assert_eq!(parsed.indexed_fields.len(), original.indexed_fields.len());
    assert_eq!(parsed.merge_strategy, original.merge_strategy);

    for (p, o) in parsed.indexed_fields.iter().zip(original.indexed_fields.iter()) {
        assert_eq!(p.field_path, o.field_path);
        assert_eq!(p.field_type, o.field_type);
        assert_eq!(p.searchable, o.searchable);
    }
}

#[test]
fn entity_schema_deserialize_from_json_matches_csharp_contract() {
    // This mirrors what the C# PrivStack.Sdk serializes across FFI
    let json = r#"{
        "entity_type": "task",
        "indexed_fields": [
            {"field_path": "/title", "field_type": "text", "searchable": true},
            {"field_path": "/done", "field_type": "bool", "searchable": false},
            {"field_path": "/due_date", "field_type": "date_time", "searchable": false},
            {"field_path": "/priority", "field_type": "number", "searchable": false},
            {"field_path": "/tags", "field_type": "tag", "searchable": true}
        ],
        "merge_strategy": "lww_per_field"
    }"#;

    let schema: EntitySchema = serde_json::from_str(json).unwrap();
    assert_eq!(schema.entity_type, "task");
    assert_eq!(schema.indexed_fields.len(), 5);
    assert_eq!(schema.merge_strategy, MergeStrategy::LwwPerField);

    assert_eq!(schema.indexed_fields[0].field_type, FieldType::Text);
    assert_eq!(schema.indexed_fields[1].field_type, FieldType::Bool);
    assert_eq!(schema.indexed_fields[2].field_type, FieldType::DateTime);
    assert_eq!(schema.indexed_fields[3].field_type, FieldType::Number);
    assert_eq!(schema.indexed_fields[4].field_type, FieldType::Tag);
}

// ── New field type constructors ──────────────────────────────────

#[test]
fn vector_field() {
    let f = IndexedField::vector("/embedding", 384);
    assert_eq!(f.field_type, FieldType::Vector);
    assert_eq!(f.vector_dim, Some(384));
    assert!(!f.searchable);
}

#[test]
fn counter_field() {
    let f = IndexedField::counter("/view_count");
    assert_eq!(f.field_type, FieldType::Counter);
}

#[test]
fn relation_field() {
    let f = IndexedField::relation("/parent_id");
    assert_eq!(f.field_type, FieldType::Relation);
}

#[test]
fn decimal_field() {
    let f = IndexedField::decimal("/price");
    assert_eq!(f.field_type, FieldType::Decimal);
}

#[test]
fn json_field() {
    let f = IndexedField::json("/metadata");
    assert_eq!(f.field_type, FieldType::Json);
}

#[test]
fn enum_field() {
    let f = IndexedField::enumeration("/status", vec!["open".into(), "closed".into()]);
    assert_eq!(f.field_type, FieldType::Enum);
    assert_eq!(f.enum_options, Some(vec!["open".into(), "closed".into()]));
}

#[test]
fn geo_point_field() {
    let f = IndexedField::geo_point("/location");
    assert_eq!(f.field_type, FieldType::GeoPoint);
}

#[test]
fn duration_field() {
    let f = IndexedField::duration("/time_spent");
    assert_eq!(f.field_type, FieldType::Duration);
}

// ── New field type serde ────────────────────────────────────────

#[test]
fn new_field_types_serde_roundtrip() {
    let types = vec![
        FieldType::Vector,
        FieldType::Counter,
        FieldType::Relation,
        FieldType::Decimal,
        FieldType::Json,
        FieldType::Enum,
        FieldType::GeoPoint,
        FieldType::Duration,
    ];
    for ft in types {
        let json = serde_json::to_string(&ft).unwrap();
        let parsed: FieldType = serde_json::from_str(&json).unwrap();
        assert_eq!(ft, parsed, "round-trip failed for {json}");
    }
}

#[test]
fn vector_serde_json_shape() {
    // FieldType::Vector is now a flat string
    let ft = FieldType::Vector;
    let json = serde_json::to_string(&ft).unwrap();
    assert_eq!(json, "\"vector\"");
}

#[test]
fn enum_serde_json_shape() {
    // FieldType::Enum is now a flat string
    let ft = FieldType::Enum;
    let json = serde_json::to_string(&ft).unwrap();
    assert_eq!(json, "\"enum\"");
}

#[test]
fn vector_field_with_dim_roundtrip() {
    let f = IndexedField::vector("/embedding", 384);
    let json = serde_json::to_string(&f).unwrap();
    let parsed: IndexedField = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.field_type, FieldType::Vector);
    assert_eq!(parsed.vector_dim, Some(384));
}

#[test]
fn enum_field_with_options_roundtrip() {
    let f = IndexedField::enumeration("/status", vec!["open".into(), "closed".into()]);
    let json = serde_json::to_string(&f).unwrap();
    let parsed: IndexedField = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.field_type, FieldType::Enum);
    assert_eq!(parsed.enum_options, Some(vec!["open".into(), "closed".into()]));
}

#[test]
fn new_simple_field_types_serde_snake_case() {
    assert_eq!(serde_json::to_string(&FieldType::Counter).unwrap(), "\"counter\"");
    assert_eq!(serde_json::to_string(&FieldType::Relation).unwrap(), "\"relation\"");
    assert_eq!(serde_json::to_string(&FieldType::Decimal).unwrap(), "\"decimal\"");
    assert_eq!(serde_json::to_string(&FieldType::Json).unwrap(), "\"json\"");
    assert_eq!(serde_json::to_string(&FieldType::GeoPoint).unwrap(), "\"geo_point\"");
    assert_eq!(serde_json::to_string(&FieldType::Duration).unwrap(), "\"duration\"");
}

#[test]
fn new_field_types_deserialize_from_csharp_json_contract() {
    // Mirrors what the C# PrivStack.Sdk actually serializes — flat field_type with
    // vector_dim/enum_options as separate fields on IndexedField
    let json = r#"{
        "entity_type": "invoice",
        "indexed_fields": [
            {"field_path": "/amount", "field_type": "decimal", "searchable": false},
            {"field_path": "/status", "field_type": "enum", "searchable": false, "options": ["draft", "sent", "paid"]},
            {"field_path": "/parent_id", "field_type": "relation", "searchable": false},
            {"field_path": "/location", "field_type": "geo_point", "searchable": false},
            {"field_path": "/time_spent", "field_type": "duration", "searchable": false},
            {"field_path": "/view_count", "field_type": "counter", "searchable": false},
            {"field_path": "/metadata", "field_type": "json", "searchable": false},
            {"field_path": "/embedding", "field_type": "vector", "searchable": false, "dimensions": 256}
        ],
        "merge_strategy": "lww_per_field"
    }"#;

    let schema: EntitySchema = serde_json::from_str(json).unwrap();
    assert_eq!(schema.entity_type, "invoice");
    assert_eq!(schema.indexed_fields.len(), 8);

    assert_eq!(schema.indexed_fields[0].field_type, FieldType::Decimal);
    assert_eq!(schema.indexed_fields[1].field_type, FieldType::Enum);
    assert_eq!(
        schema.indexed_fields[1].enum_options,
        Some(vec!["draft".into(), "sent".into(), "paid".into()])
    );
    assert_eq!(schema.indexed_fields[2].field_type, FieldType::Relation);
    assert_eq!(schema.indexed_fields[3].field_type, FieldType::GeoPoint);
    assert_eq!(schema.indexed_fields[4].field_type, FieldType::Duration);
    assert_eq!(schema.indexed_fields[5].field_type, FieldType::Counter);
    assert_eq!(schema.indexed_fields[6].field_type, FieldType::Json);
    assert_eq!(schema.indexed_fields[7].field_type, FieldType::Vector);
    assert_eq!(schema.indexed_fields[7].vector_dim, Some(256));
}

// ── New field type equality ─────────────────────────────────────

#[test]
fn new_field_type_equality() {
    assert_eq!(FieldType::Counter, FieldType::Counter);
    assert_eq!(FieldType::Relation, FieldType::Relation);
    assert_eq!(FieldType::Decimal, FieldType::Decimal);
    assert_eq!(FieldType::Json, FieldType::Json);
    assert_eq!(FieldType::GeoPoint, FieldType::GeoPoint);
    assert_eq!(FieldType::Duration, FieldType::Duration);
    assert_eq!(FieldType::Vector, FieldType::Vector);
    assert_eq!(FieldType::Enum, FieldType::Enum);

    assert_ne!(FieldType::Counter, FieldType::Relation);
    assert_ne!(FieldType::Decimal, FieldType::Number);
    assert_ne!(FieldType::Json, FieldType::Text);
    assert_ne!(FieldType::Duration, FieldType::DateTime);
    assert_ne!(FieldType::Vector, FieldType::Enum);
}

// ── Clone ───────────────────────────────────────────────────────

#[test]
fn field_type_copy() {
    let ft = FieldType::Vector;
    let ft2 = ft;
    assert_eq!(ft, ft2);
}

// ── IndexedField with new types in schema ───────────────────────

#[test]
fn schema_with_all_new_field_types() {
    let schema = EntitySchema {
        entity_type: "rich_entity".into(),
        indexed_fields: vec![
            IndexedField::text("/title", true),
            IndexedField::vector("/embedding", 384),
            IndexedField::counter("/views"),
            IndexedField::relation("/parent_id"),
            IndexedField::decimal("/amount"),
            IndexedField::json("/metadata"),
            IndexedField::enumeration("/status", vec!["active".into(), "archived".into()]),
            IndexedField::geo_point("/location"),
            IndexedField::duration("/elapsed"),
        ],
        merge_strategy: MergeStrategy::LwwPerField,
    };

    let json = serde_json::to_string(&schema).unwrap();
    let parsed: EntitySchema = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.entity_type, "rich_entity");
    assert_eq!(parsed.indexed_fields.len(), 9);
    for (p, o) in parsed.indexed_fields.iter().zip(schema.indexed_fields.iter()) {
        assert_eq!(p.field_path, o.field_path);
        assert_eq!(p.field_type, o.field_type);
        assert_eq!(p.searchable, o.searchable);
        assert_eq!(p.vector_dim, o.vector_dim);
        assert_eq!(p.enum_options, o.enum_options);
    }
}

// ── Edge cases ───────────────────────────────────────────────────

#[test]
fn schema_with_no_indexed_fields() {
    let schema = EntitySchema {
        entity_type: "blob".to_string(),
        indexed_fields: vec![],
        merge_strategy: MergeStrategy::LwwDocument,
    };
    assert!(schema.indexed_fields.is_empty());
    let json = serde_json::to_string(&schema).unwrap();
    let parsed: EntitySchema = serde_json::from_str(&json).unwrap();
    assert!(parsed.indexed_fields.is_empty());
}

#[test]
fn schema_clone_is_independent() {
    let original = make_note_schema();
    let mut cloned = original.clone();
    cloned.entity_type = "different".to_string();
    cloned.indexed_fields.pop();

    assert_eq!(original.entity_type, "note");
    assert_eq!(original.indexed_fields.len(), 4);
    assert_eq!(cloned.entity_type, "different");
    assert_eq!(cloned.indexed_fields.len(), 3);
}
