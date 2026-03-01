//! DDL and schema helpers for the datasets database.

use crate::error::DatasetResult;
use privstack_db::rusqlite::Connection;

/// Metadata table DDL — stores info about each imported dataset.
const DATASETS_META_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS _datasets_meta (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    source_file_name TEXT,
    row_count INTEGER NOT NULL DEFAULT 0,
    columns_json TEXT NOT NULL DEFAULT '[]',
    created_at INTEGER NOT NULL,
    modified_at INTEGER NOT NULL
);
"#;

/// Relations table DDL — foreign-key-like links between datasets.
const DATASET_RELATIONS_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS _dataset_relations (
    id TEXT PRIMARY KEY,
    source_dataset_id TEXT NOT NULL,
    source_column TEXT NOT NULL,
    target_dataset_id TEXT NOT NULL,
    target_column TEXT NOT NULL,
    relation_type TEXT DEFAULT 'many_to_one',
    created_at INTEGER NOT NULL,
    FOREIGN KEY (source_dataset_id) REFERENCES _datasets_meta(id),
    FOREIGN KEY (target_dataset_id) REFERENCES _datasets_meta(id)
);
"#;

/// Row-page linking table DDL — maps dataset rows to Notes pages.
const DATASET_ROW_PAGES_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS _dataset_row_pages (
    dataset_id TEXT NOT NULL,
    row_index INTEGER NOT NULL,
    row_key TEXT NOT NULL,
    page_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (dataset_id, row_key)
);
"#;

/// Saved views table DDL — named view configs per dataset.
const DATASET_VIEWS_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS _dataset_views (
    id TEXT PRIMARY KEY,
    dataset_id TEXT NOT NULL,
    name TEXT NOT NULL,
    config_json TEXT NOT NULL,
    is_default INTEGER DEFAULT 0,
    sort_order INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    modified_at INTEGER NOT NULL,
    FOREIGN KEY (dataset_id) REFERENCES _datasets_meta(id)
);
"#;

/// Saved queries table DDL — user-authored SQL queries.
const DATASET_SAVED_QUERIES_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS _dataset_saved_queries (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    sql TEXT NOT NULL,
    description TEXT,
    created_at INTEGER NOT NULL,
    modified_at INTEGER NOT NULL
);
"#;

/// Initialize all dataset schema tables.
pub fn initialize_datasets_schema(conn: &Connection) -> DatasetResult<()> {
    conn.execute_batch(DATASETS_META_DDL)?;
    conn.execute_batch(DATASET_RELATIONS_DDL)?;
    conn.execute_batch(DATASET_ROW_PAGES_DDL)?;
    conn.execute_batch(DATASET_VIEWS_DDL)?;
    conn.execute_batch(DATASET_SAVED_QUERIES_DDL)?;

    // Migrations — use privstack_db helpers for safe ADD COLUMN
    privstack_db::add_column_if_not_exists(
        conn,
        "_dataset_saved_queries",
        "is_view",
        "INTEGER DEFAULT 0",
    )?;
    privstack_db::add_column_if_not_exists(conn, "_datasets_meta", "category", "TEXT")?;
    Ok(())
}

/// Derive the per-dataset table name from a dataset ID.
/// Format: `ds_<uuid_no_hyphens>`.
pub fn dataset_table_name(id: &crate::types::DatasetId) -> String {
    id.table_name()
}
