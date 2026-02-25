//! Comprehensive tests for the DatasetStore: CRUD, mutations, queries,
//! relations, row-page linking, saved queries, views, and the SQL preprocessor.

use pretty_assertions::assert_eq;
use privstack_datasets::*;
use serde_json::json;

// ── Helpers ─────────────────────────────────────────────────────────────

fn store() -> DatasetStore {
    DatasetStore::open_in_memory().expect("in-memory store must open")
}

fn create_test_dataset(store: &DatasetStore) -> DatasetMeta {
    store
        .create_empty(
            "test_ds",
            &[
                ColumnDef { name: "name".into(), column_type: "VARCHAR".into() },
                ColumnDef { name: "age".into(), column_type: "INTEGER".into() },
                ColumnDef { name: "score".into(), column_type: "DOUBLE".into() },
            ],
            None,
        )
        .expect("create_empty must succeed")
}

fn create_and_populate(store: &DatasetStore) -> DatasetMeta {
    let meta = create_test_dataset(store);
    store.insert_row(&meta.id, &[("name", json!("Alice")), ("age", json!(30)), ("score", json!(95.5))]).unwrap();
    store.insert_row(&meta.id, &[("name", json!("Bob")), ("age", json!(25)), ("score", json!(88.0))]).unwrap();
    store.insert_row(&meta.id, &[("name", json!("Carol")), ("age", json!(35)), ("score", json!(92.3))]).unwrap();
    // Refresh metadata (row count updated)
    store.get(&meta.id).unwrap()
}

// ── DatasetStore basics ─────────────────────────────────────────────────

#[test]
fn open_in_memory_succeeds() {
    let _store = store();
}

#[test]
fn checkpoint_succeeds() {
    let s = store();
    s.checkpoint().unwrap();
}

#[test]
fn maintenance_succeeds() {
    let s = store();
    s.maintenance().unwrap();
}

// ── CRUD: create_empty ──────────────────────────────────────────────────

#[test]
fn create_empty_dataset() {
    let s = store();
    let meta = create_test_dataset(&s);
    assert_eq!(meta.name, "test_ds");
    assert_eq!(meta.row_count, 0);
    assert_eq!(meta.columns.len(), 3);
    assert_eq!(meta.columns[0].name, "name");
    assert_eq!(meta.columns[0].column_type, DatasetColumnType::Text);
    assert_eq!(meta.columns[1].column_type, DatasetColumnType::Integer);
    assert_eq!(meta.columns[2].column_type, DatasetColumnType::Float);
    assert!(meta.source_file_name.is_none());
    assert!(meta.category.is_none());
}

#[test]
fn create_empty_with_category() {
    let s = store();
    let meta = s
        .create_empty("categorized", &[ColumnDef { name: "x".into(), column_type: "INTEGER".into() }], Some("ai_generated"))
        .unwrap();
    assert_eq!(meta.category.as_deref(), Some("ai_generated"));
}

#[test]
fn create_empty_no_columns_errors() {
    let s = store();
    let err = s.create_empty("empty", &[], None);
    assert!(err.is_err());
    let msg = format!("{}", err.unwrap_err());
    assert!(msg.contains("At least one column"), "Expected column error, got: {msg}");
}

// ── CRUD: list & get ────────────────────────────────────────────────────

#[test]
fn list_empty_returns_empty() {
    let s = store();
    let list = s.list().unwrap();
    assert!(list.is_empty());
}

#[test]
fn list_returns_created_datasets() {
    let s = store();
    create_test_dataset(&s);
    s.create_empty("second", &[ColumnDef { name: "a".into(), column_type: "VARCHAR".into() }], None).unwrap();
    let list = s.list().unwrap();
    assert_eq!(list.len(), 2);
}

#[test]
fn get_existing_dataset() {
    let s = store();
    let meta = create_test_dataset(&s);
    let fetched = s.get(&meta.id).unwrap();
    assert_eq!(fetched.name, "test_ds");
    assert_eq!(fetched.id, meta.id);
}

#[test]
fn get_nonexistent_returns_not_found() {
    let s = store();
    let fake_id = DatasetId::new();
    let err = s.get(&fake_id);
    assert!(err.is_err());
}

// ── CRUD: delete ────────────────────────────────────────────────────────

#[test]
fn delete_existing_dataset() {
    let s = store();
    let meta = create_test_dataset(&s);
    s.delete(&meta.id).unwrap();
    let list = s.list().unwrap();
    assert!(list.is_empty());
}

#[test]
fn delete_nonexistent_returns_not_found() {
    let s = store();
    let fake_id = DatasetId::new();
    let err = s.delete(&fake_id);
    assert!(err.is_err());
}

#[test]
fn delete_cascades_relations_and_views() {
    let s = store();
    let ds1 = create_test_dataset(&s);
    let ds2 = s.create_empty("ds2", &[ColumnDef { name: "x".into(), column_type: "INTEGER".into() }], None).unwrap();
    s.create_relation(&ds1.id, "name", &ds2.id, "x").unwrap();
    s.create_view(&ds1.id, "view1", &ViewConfig { visible_columns: None, filters: vec![], sorts: vec![], group_by: None }).unwrap();
    s.link_row_to_page(&ds1.id, "row-1", "page-1").unwrap();

    s.delete(&ds1.id).unwrap();

    assert!(s.list_relations(&ds1.id).unwrap().is_empty());
    assert!(s.list_views(&ds1.id).unwrap().is_empty());
    assert!(s.get_page_for_row(&ds1.id, "row-1").unwrap().is_none());
}

// ── CRUD: rename & set_category ─────────────────────────────────────────

#[test]
fn rename_dataset() {
    let s = store();
    let meta = create_test_dataset(&s);
    s.rename(&meta.id, "renamed_ds").unwrap();
    let fetched = s.get(&meta.id).unwrap();
    assert_eq!(fetched.name, "renamed_ds");
}

#[test]
fn rename_nonexistent_returns_not_found() {
    let s = store();
    let err = s.rename(&DatasetId::new(), "new_name");
    assert!(err.is_err());
}

#[test]
fn set_category_then_clear() {
    let s = store();
    let meta = create_test_dataset(&s);
    s.set_category(&meta.id, Some("reports")).unwrap();
    let fetched = s.get(&meta.id).unwrap();
    assert_eq!(fetched.category.as_deref(), Some("reports"));

    s.set_category(&meta.id, None).unwrap();
    let fetched = s.get(&meta.id).unwrap();
    assert!(fetched.category.is_none());
}

#[test]
fn set_category_nonexistent_returns_not_found() {
    let s = store();
    let err = s.set_category(&DatasetId::new(), Some("x"));
    assert!(err.is_err());
}

// ── Mutations: insert_row ───────────────────────────────────────────────

#[test]
fn insert_row_increments_count() {
    let s = store();
    let meta = create_test_dataset(&s);
    s.insert_row(&meta.id, &[("name", json!("Alice")), ("age", json!(30)), ("score", json!(95.5))]).unwrap();
    let fetched = s.get(&meta.id).unwrap();
    assert_eq!(fetched.row_count, 1);
}

#[test]
fn insert_multiple_rows() {
    let s = store();
    let meta = create_and_populate(&s);
    assert_eq!(meta.row_count, 3);
}

// ── Mutations: update_cell ──────────────────────────────────────────────

#[test]
fn update_cell_value() {
    let s = store();
    let meta = create_and_populate(&s);
    s.update_cell(&meta.id, 0, "name", json!("Alicia")).unwrap();
    let result = s.query_dataset(&meta.id, 0, 10, None, None, false).unwrap();
    let first_row_name = &result.rows[0][0];
    assert_eq!(first_row_name, &json!("Alicia"));
}

// ── Mutations: delete_rows ──────────────────────────────────────────────

#[test]
fn delete_rows_removes_rows() {
    let s = store();
    let meta = create_and_populate(&s);
    s.delete_rows(&meta.id, &[0]).unwrap();
    let fetched = s.get(&meta.id).unwrap();
    assert_eq!(fetched.row_count, 2);
}

#[test]
fn delete_rows_empty_indices_is_noop() {
    let s = store();
    let meta = create_and_populate(&s);
    s.delete_rows(&meta.id, &[]).unwrap();
    let fetched = s.get(&meta.id).unwrap();
    assert_eq!(fetched.row_count, 3);
}

// ── Mutations: add_column, drop_column, rename_column, alter_column_type

#[test]
fn add_column_updates_metadata() {
    let s = store();
    let meta = create_test_dataset(&s);
    s.add_column(&meta.id, "email", "VARCHAR", None).unwrap();
    let fetched = s.get(&meta.id).unwrap();
    assert_eq!(fetched.columns.len(), 4);
    assert!(fetched.columns.iter().any(|c| c.name == "email"));
}

#[test]
fn add_column_with_default() {
    let s = store();
    let meta = create_and_populate(&s);
    s.add_column(&meta.id, "active", "BOOLEAN", Some("true")).unwrap();
    let result = s.query_dataset(&meta.id, 0, 10, None, None, false).unwrap();
    assert!(result.columns.contains(&"active".to_string()));
}

#[test]
fn drop_column_removes_column() {
    let s = store();
    let meta = create_test_dataset(&s);
    assert_eq!(meta.columns.len(), 3);
    s.drop_column(&meta.id, "score").unwrap();
    let fetched = s.get(&meta.id).unwrap();
    assert_eq!(fetched.columns.len(), 2);
    assert!(!fetched.columns.iter().any(|c| c.name == "score"));
}

#[test]
fn rename_column_updates_metadata() {
    let s = store();
    let meta = create_test_dataset(&s);
    s.rename_column(&meta.id, "name", "full_name").unwrap();
    let fetched = s.get(&meta.id).unwrap();
    assert!(fetched.columns.iter().any(|c| c.name == "full_name"));
    assert!(!fetched.columns.iter().any(|c| c.name == "name"));
}

#[test]
fn alter_column_type_valid() {
    let s = store();
    let meta = create_test_dataset(&s);
    s.alter_column_type(&meta.id, "age", "BIGINT").unwrap();
    let fetched = s.get(&meta.id).unwrap();
    let age_col = fetched.columns.iter().find(|c| c.name == "age").unwrap();
    assert_eq!(age_col.column_type, DatasetColumnType::Integer);
}

#[test]
fn alter_column_type_invalid_rejected() {
    let s = store();
    let meta = create_test_dataset(&s);
    let err = s.alter_column_type(&meta.id, "age", "JSONB");
    assert!(err.is_err());
    let msg = format!("{}", err.unwrap_err());
    assert!(msg.contains("Unsupported column type"));
}

// ── Mutations: duplicate ────────────────────────────────────────────────

#[test]
fn duplicate_copies_data_and_schema() {
    let s = store();
    let meta = create_and_populate(&s);
    let dup = s.duplicate(&meta.id, "copy_of_test").unwrap();
    assert_eq!(dup.name, "copy_of_test");
    assert_eq!(dup.row_count, 3);
    assert_eq!(dup.columns.len(), meta.columns.len());
    assert_ne!(dup.id, meta.id);
}

#[test]
fn duplicate_preserves_category() {
    let s = store();
    let meta = create_test_dataset(&s);
    s.set_category(&meta.id, Some("reports")).unwrap();
    let dup = s.duplicate(&meta.id, "dup").unwrap();
    assert_eq!(dup.category.as_deref(), Some("reports"));
}

// ── Mutations: import_csv_content ───────────────────────────────────────

#[test]
fn import_csv_content_creates_dataset() {
    let s = store();
    let csv = "name,age\nAlice,30\nBob,25";
    let meta = s.import_csv_content(csv, "from_clipboard", None).unwrap();
    assert_eq!(meta.name, "from_clipboard");
    assert_eq!(meta.row_count, 2);
    assert_eq!(meta.columns.len(), 2);
}

#[test]
fn import_csv_content_with_category() {
    let s = store();
    let csv = "x\n1\n2";
    let meta = s.import_csv_content(csv, "cat_test", Some("ai_generated")).unwrap();
    assert_eq!(meta.category.as_deref(), Some("ai_generated"));
}

// ── Mutations: execute_mutation ─────────────────────────────────────────

#[test]
fn execute_mutation_committed() {
    let s = store();
    let meta = create_and_populate(&s);
    let table = meta.id.table_name();
    let sql = format!("DELETE FROM {table} WHERE \"age\" < 30");
    let result = s.execute_mutation(&sql, false).unwrap();
    assert!(result.committed);
    assert_eq!(result.affected_rows, 1); // Bob (age 25)
}

#[test]
fn execute_mutation_dry_run_does_not_persist() {
    let s = store();
    let meta = create_and_populate(&s);
    let table = meta.id.table_name();
    let sql = format!("DELETE FROM {table} WHERE \"age\" < 30");
    let result = s.execute_mutation(&sql, true).unwrap();
    assert!(!result.committed);
    assert_eq!(result.affected_rows, 1);

    // Data should still be there — verify via query (not get, which reads meta)
    let qr = s.query_dataset(&meta.id, 0, 100, None, None, false).unwrap();
    assert_eq!(qr.total_count, 3);
}

// ── Queries: query_dataset ──────────────────────────────────────────────

#[test]
fn query_dataset_basic() {
    let s = store();
    let meta = create_and_populate(&s);
    let result = s.query_dataset(&meta.id, 0, 10, None, None, false).unwrap();
    assert_eq!(result.total_count, 3);
    assert_eq!(result.rows.len(), 3);
    assert_eq!(result.page, 0);
    assert_eq!(result.page_size, 10);
}

#[test]
fn query_dataset_pagination() {
    let s = store();
    let meta = create_and_populate(&s);
    let page1 = s.query_dataset(&meta.id, 0, 2, None, None, false).unwrap();
    assert_eq!(page1.rows.len(), 2);
    assert_eq!(page1.total_count, 3);

    let page2 = s.query_dataset(&meta.id, 1, 2, None, None, false).unwrap();
    assert_eq!(page2.rows.len(), 1);
}

#[test]
fn query_dataset_sort_asc() {
    let s = store();
    let meta = create_and_populate(&s);
    let result = s.query_dataset(&meta.id, 0, 10, None, Some("age"), false).unwrap();
    let ages: Vec<&serde_json::Value> = result.rows.iter().map(|r| &r[1]).collect();
    assert_eq!(ages, vec![&json!(25), &json!(30), &json!(35)]);
}

#[test]
fn query_dataset_sort_desc() {
    let s = store();
    let meta = create_and_populate(&s);
    let result = s.query_dataset(&meta.id, 0, 10, None, Some("age"), true).unwrap();
    let ages: Vec<&serde_json::Value> = result.rows.iter().map(|r| &r[1]).collect();
    assert_eq!(ages, vec![&json!(35), &json!(30), &json!(25)]);
}

#[test]
fn query_dataset_filter_text() {
    let s = store();
    let meta = create_and_populate(&s);
    let result = s.query_dataset(&meta.id, 0, 10, Some("Alice"), None, false).unwrap();
    assert_eq!(result.total_count, 1);
    assert_eq!(result.rows[0][0], json!("Alice"));
}

// ── Queries: get_columns ────────────────────────────────────────────────

#[test]
fn get_columns_returns_schema() {
    let s = store();
    let meta = create_test_dataset(&s);
    let cols = s.get_columns(&meta.id).unwrap();
    assert_eq!(cols.len(), 3);
    assert_eq!(cols[0].name, "name");
}

// ── Queries: execute_raw_query ──────────────────────────────────────────

#[test]
fn execute_raw_query_select() {
    let s = store();
    let meta = create_and_populate(&s);
    let table = meta.id.table_name();
    let result = s.execute_raw_query(&format!("SELECT * FROM {table}"), 0, 100).unwrap();
    assert_eq!(result.total_count, 3);
}

#[test]
fn execute_raw_query_rejects_non_select() {
    let s = store();
    let err = s.execute_raw_query("DELETE FROM foo", 0, 100);
    assert!(err.is_err());
    let msg = format!("{}", err.unwrap_err());
    assert!(msg.contains("Only SELECT"));
}

#[test]
fn execute_raw_query_with_semicolons() {
    let s = store();
    let meta = create_and_populate(&s);
    let table = meta.id.table_name();
    let result = s.execute_raw_query(&format!("SELECT * FROM {table}  ;  "), 0, 100).unwrap();
    assert_eq!(result.total_count, 3);
}

// ── Queries: aggregate_query ────────────────────────────────────────────

#[test]
fn aggregate_query_no_aggregation() {
    let s = store();
    let meta = create_and_populate(&s);
    let result = s.aggregate_query(&meta.id, "name", "score", None, None, None).unwrap();
    assert_eq!(result.len(), 3);
}

#[test]
fn aggregate_query_with_sum() {
    let s = store();
    let meta = create_and_populate(&s);
    let result = s.aggregate_query(&meta.id, "name", "score", Some("SUM"), None, None).unwrap();
    assert_eq!(result.len(), 3); // 3 unique names, each with their sum
}

// ── Queries: aggregate_query_grouped ────────────────────────────────────

#[test]
fn aggregate_query_grouped_basic() {
    let s = store();
    let meta = create_and_populate(&s);
    let result = s.aggregate_query_grouped(&meta.id, "name", "score", "age", Some("SUM"), None).unwrap();
    assert_eq!(result.len(), 3); // Each name+age combo is unique
}

// ── Queries: execute_sql_v2 ─────────────────────────────────────────────

#[test]
fn execute_sql_v2_select() {
    let s = store();
    let meta = create_and_populate(&s);
    let table = meta.id.table_name();
    let result = s.execute_sql_v2(&format!("SELECT * FROM {table}"), 0, 100, false).unwrap();
    match result {
        SqlExecutionResult::Query(qr) => assert_eq!(qr.total_count, 3),
        SqlExecutionResult::Mutation(_) => panic!("Expected query result"),
    }
}

#[test]
fn execute_sql_v2_mutation() {
    let s = store();
    let meta = create_and_populate(&s);
    let table = meta.id.table_name();
    let result = s.execute_sql_v2(&format!("INSERT INTO {table} VALUES ('Dave', 40, 77.0)"), 0, 100, false).unwrap();
    match result {
        SqlExecutionResult::Mutation(mr) => {
            assert!(mr.committed);
            assert_eq!(mr.affected_rows, 1);
        }
        SqlExecutionResult::Query(_) => panic!("Expected mutation result"),
    }
}

#[test]
fn execute_sql_v2_empty_sql_errors() {
    let s = store();
    let err = s.execute_sql_v2("   ", 0, 100, false);
    assert!(err.is_err());
}

#[test]
fn execute_sql_v2_with_source_alias() {
    let s = store();
    let meta = s.create_empty("Sales", &[ColumnDef { name: "amount".into(), column_type: "INTEGER".into() }], None).unwrap();
    s.insert_row(&meta.id, &[("amount", json!(100))]).unwrap();

    let result = s.execute_sql_v2("SELECT * FROM source:Sales", 0, 100, false).unwrap();
    match result {
        SqlExecutionResult::Query(qr) => assert_eq!(qr.total_count, 1),
        _ => panic!("Expected query result"),
    }
}

#[test]
fn execute_sql_v2_source_alias_quoted() {
    let s = store();
    let meta = s.create_empty("My Data Set", &[ColumnDef { name: "x".into(), column_type: "INTEGER".into() }], None).unwrap();
    s.insert_row(&meta.id, &[("x", json!(42))]).unwrap();

    let result = s.execute_sql_v2("SELECT * FROM source:\"My Data Set\"", 0, 100, false).unwrap();
    match result {
        SqlExecutionResult::Query(qr) => {
            assert_eq!(qr.total_count, 1);
            assert_eq!(qr.rows[0][0], json!(42));
        }
        _ => panic!("Expected query result"),
    }
}

#[test]
fn execute_sql_v2_source_alias_not_found() {
    let s = store();
    let err = s.execute_sql_v2("SELECT * FROM source:NonExistent", 0, 100, false);
    assert!(err.is_err());
    let msg = format!("{}", err.unwrap_err());
    assert!(msg.contains("not found"));
}

// ── Relations ───────────────────────────────────────────────────────────

#[test]
fn create_and_list_relation() {
    let s = store();
    let ds1 = create_test_dataset(&s);
    let ds2 = s.create_empty("ds2", &[ColumnDef { name: "name_ref".into(), column_type: "VARCHAR".into() }], None).unwrap();

    let rel = s.create_relation(&ds1.id, "name", &ds2.id, "name_ref").unwrap();
    assert_eq!(rel.source_column, "name");
    assert_eq!(rel.target_column, "name_ref");
    assert_eq!(rel.relation_type, RelationType::ManyToOne);

    let rels = s.list_relations(&ds1.id).unwrap();
    assert_eq!(rels.len(), 1);

    // Also visible from target side
    let rels2 = s.list_relations(&ds2.id).unwrap();
    assert_eq!(rels2.len(), 1);
}

#[test]
fn delete_relation() {
    let s = store();
    let ds1 = create_test_dataset(&s);
    let ds2 = s.create_empty("ds2", &[ColumnDef { name: "x".into(), column_type: "INTEGER".into() }], None).unwrap();
    let rel = s.create_relation(&ds1.id, "name", &ds2.id, "x").unwrap();
    s.delete_relation(&rel.id).unwrap();
    let rels = s.list_relations(&ds1.id).unwrap();
    assert!(rels.is_empty());
}

// ── Row-Page Linking ────────────────────────────────────────────────────

#[test]
fn link_row_to_page_and_retrieve() {
    let s = store();
    let meta = create_test_dataset(&s);
    s.link_row_to_page(&meta.id, "row-key-1", "page-abc").unwrap();

    let page = s.get_page_for_row(&meta.id, "row-key-1").unwrap();
    assert_eq!(page.as_deref(), Some("page-abc"));

    let (ds_id, row_key) = s.get_row_for_page("page-abc").unwrap().unwrap();
    assert_eq!(ds_id, meta.id);
    assert_eq!(row_key, "row-key-1");
}

#[test]
fn get_page_for_nonexistent_row_returns_none() {
    let s = store();
    let meta = create_test_dataset(&s);
    let page = s.get_page_for_row(&meta.id, "no-such-row").unwrap();
    assert!(page.is_none());
}

#[test]
fn get_row_for_nonexistent_page_returns_none() {
    let s = store();
    let result = s.get_row_for_page("no-such-page").unwrap();
    assert!(result.is_none());
}

#[test]
fn unlink_row_page() {
    let s = store();
    let meta = create_test_dataset(&s);
    s.link_row_to_page(&meta.id, "r1", "p1").unwrap();
    s.unlink_row_page(&meta.id, "r1").unwrap();
    let page = s.get_page_for_row(&meta.id, "r1").unwrap();
    assert!(page.is_none());
}

// ── Saved Queries ───────────────────────────────────────────────────────

#[test]
fn create_and_list_saved_queries() {
    let s = store();
    let sq = s.create_saved_query("My Query", "SELECT 1", Some("a test"), false).unwrap();
    assert_eq!(sq.name, "My Query");
    assert_eq!(sq.sql, "SELECT 1");
    assert_eq!(sq.description.as_deref(), Some("a test"));
    assert!(!sq.is_view);

    let list = s.list_saved_queries().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "My Query");
}

#[test]
fn create_saved_query_as_view() {
    let s = store();
    let sq = s.create_saved_query("View 1", "SELECT 42", None, true).unwrap();
    assert!(sq.is_view);
    let list = s.list_saved_queries().unwrap();
    assert!(list[0].is_view);
}

#[test]
fn update_saved_query() {
    let s = store();
    let sq = s.create_saved_query("q", "SELECT 1", None, false).unwrap();
    s.update_saved_query(&sq.id, "updated_q", "SELECT 2", Some("new desc"), true).unwrap();
    let list = s.list_saved_queries().unwrap();
    assert_eq!(list[0].name, "updated_q");
    assert_eq!(list[0].sql, "SELECT 2");
    assert!(list[0].is_view);
}

#[test]
fn delete_saved_query() {
    let s = store();
    let sq = s.create_saved_query("to_delete", "SELECT 1", None, false).unwrap();
    s.delete_saved_query(&sq.id).unwrap();
    let list = s.list_saved_queries().unwrap();
    assert!(list.is_empty());
}

// ── Views ───────────────────────────────────────────────────────────────

#[test]
fn create_and_list_views() {
    let s = store();
    let meta = create_test_dataset(&s);
    let config = ViewConfig {
        visible_columns: Some(vec!["name".into(), "age".into()]),
        filters: vec![ViewFilter {
            column: "age".into(),
            operator: FilterOperator::GreaterThan,
            value: "25".into(),
        }],
        sorts: vec![ViewSort {
            column: "name".into(),
            direction: SortDirection::Asc,
        }],
        group_by: None,
    };
    let view = s.create_view(&meta.id, "filtered_view", &config).unwrap();
    assert_eq!(view.name, "filtered_view");
    assert!(!view.is_default);

    let views = s.list_views(&meta.id).unwrap();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].config.filters.len(), 1);
    assert_eq!(views[0].config.sorts.len(), 1);
}

#[test]
fn update_view_config() {
    let s = store();
    let meta = create_test_dataset(&s);
    let config = ViewConfig { visible_columns: None, filters: vec![], sorts: vec![], group_by: None };
    let view = s.create_view(&meta.id, "v", &config).unwrap();

    let new_config = ViewConfig {
        visible_columns: Some(vec!["name".into()]),
        filters: vec![],
        sorts: vec![],
        group_by: Some("name".into()),
    };
    s.update_view(&view.id, &new_config).unwrap();

    let views = s.list_views(&meta.id).unwrap();
    assert_eq!(views[0].config.group_by.as_deref(), Some("name"));
    assert_eq!(views[0].config.visible_columns.as_ref().unwrap().len(), 1);
}

#[test]
fn delete_view() {
    let s = store();
    let meta = create_test_dataset(&s);
    let config = ViewConfig { visible_columns: None, filters: vec![], sorts: vec![], group_by: None };
    let view = s.create_view(&meta.id, "v", &config).unwrap();
    s.delete_view(&view.id).unwrap();
    let views = s.list_views(&meta.id).unwrap();
    assert!(views.is_empty());
}

#[test]
fn list_views_empty_for_no_views() {
    let s = store();
    let meta = create_test_dataset(&s);
    let views = s.list_views(&meta.id).unwrap();
    assert!(views.is_empty());
}

// ── Types ───────────────────────────────────────────────────────────────

#[test]
fn dataset_id_table_name_format() {
    let id = DatasetId::new();
    let table = id.table_name();
    assert!(table.starts_with("ds_"));
    assert!(!table.contains('-')); // UUID hyphens stripped
}

#[test]
fn dataset_id_display() {
    let id = DatasetId::new();
    let display = format!("{id}");
    assert!(display.contains('-')); // Display uses standard UUID format
}

#[test]
fn dataset_column_type_from_duckdb() {
    assert_eq!(DatasetColumnType::from_duckdb("VARCHAR"), DatasetColumnType::Text);
    assert_eq!(DatasetColumnType::from_duckdb("TEXT"), DatasetColumnType::Text);
    assert_eq!(DatasetColumnType::from_duckdb("INTEGER"), DatasetColumnType::Integer);
    assert_eq!(DatasetColumnType::from_duckdb("BIGINT"), DatasetColumnType::Integer);
    assert_eq!(DatasetColumnType::from_duckdb("SMALLINT"), DatasetColumnType::Integer);
    assert_eq!(DatasetColumnType::from_duckdb("TINYINT"), DatasetColumnType::Integer);
    assert_eq!(DatasetColumnType::from_duckdb("HUGEINT"), DatasetColumnType::Integer);
    assert_eq!(DatasetColumnType::from_duckdb("DOUBLE"), DatasetColumnType::Float);
    assert_eq!(DatasetColumnType::from_duckdb("FLOAT"), DatasetColumnType::Float);
    assert_eq!(DatasetColumnType::from_duckdb("DECIMAL(10,2)"), DatasetColumnType::Float);
    assert_eq!(DatasetColumnType::from_duckdb("BOOLEAN"), DatasetColumnType::Boolean);
    assert_eq!(DatasetColumnType::from_duckdb("DATE"), DatasetColumnType::Date);
    assert_eq!(DatasetColumnType::from_duckdb("TIMESTAMP"), DatasetColumnType::Timestamp);
    assert_eq!(DatasetColumnType::from_duckdb("DATETIME"), DatasetColumnType::Timestamp);
    assert_eq!(DatasetColumnType::from_duckdb("BLOB"), DatasetColumnType::Blob);
    assert_eq!(DatasetColumnType::from_duckdb("GEOMETRY"), DatasetColumnType::Unknown);
}

#[test]
fn relation_type_roundtrip() {
    assert_eq!(RelationType::from_str(RelationType::ManyToOne.as_str()), RelationType::ManyToOne);
    assert_eq!(RelationType::from_str(RelationType::ManyToMany.as_str()), RelationType::ManyToMany);
    assert_eq!(RelationType::from_str("unknown"), RelationType::ManyToOne); // default
}

#[test]
fn statement_type_serde() {
    let json = serde_json::to_string(&StatementType::Select).unwrap();
    assert_eq!(json, "\"select\"");
    let deserialized: StatementType = serde_json::from_str("\"insert\"").unwrap();
    assert_eq!(deserialized, StatementType::Insert);
}

#[test]
fn filter_operator_serde() {
    let json = serde_json::to_string(&FilterOperator::Contains).unwrap();
    assert_eq!(json, "\"contains\"");
    let deserialized: FilterOperator = serde_json::from_str("\"equals\"").unwrap();
    assert_eq!(deserialized, FilterOperator::Equals);
}

#[test]
fn sort_direction_serde() {
    let json = serde_json::to_string(&SortDirection::Desc).unwrap();
    assert_eq!(json, "\"desc\"");
}
