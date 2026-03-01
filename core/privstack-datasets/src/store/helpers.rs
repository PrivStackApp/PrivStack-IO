//! Shared helper functions for dataset store operations.

use crate::error::DatasetResult;
use crate::types::{DatasetColumn, DatasetColumnType};
use privstack_db::rusqlite::Connection;

/// Introspect column names and types from an existing table via `PRAGMA table_info()`.
pub(crate) fn introspect_columns(
    conn: &Connection,
    table: &str,
) -> DatasetResult<Vec<DatasetColumn>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info('{table}')"))?;

    let columns = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,    // cid (column index)
                row.get::<_, String>(1)?,  // name
                row.get::<_, String>(2)?,  // type
            ))
        })?
        .filter_map(|r| r.ok())
        .map(|(cid, name, dtype)| DatasetColumn {
            name,
            column_type: DatasetColumnType::from_sqlite(&dtype),
            ordinal: cid,
        })
        .collect();

    Ok(columns)
}

/// Build a WHERE clause that LIKEs the filter text across all text columns.
pub(crate) fn build_filter_clause(
    columns: &[DatasetColumn],
    filter_text: Option<&str>,
) -> String {
    match filter_text {
        Some(text) if !text.is_empty() => {
            let escaped = text.replace('\'', "''");
            let text_cols: Vec<&DatasetColumn> = columns
                .iter()
                .filter(|c| c.column_type == DatasetColumnType::Text)
                .collect();
            if text_cols.is_empty() {
                return String::new();
            }
            let conditions: Vec<String> = text_cols
                .iter()
                .map(|c| format!("\"{}\" LIKE '%{escaped}%'", sanitize_identifier(&c.name)))
                .collect();
            format!(" WHERE {}", conditions.join(" OR "))
        }
        _ => String::new(),
    }
}

/// Escape a column name for use in double-quoted SQL identifiers.
/// We only need to escape embedded double quotes (by doubling them).
pub(crate) fn sanitize_identifier(name: &str) -> String {
    name.replace('"', "\"\"")
}

/// Extract a single row value as serde_json::Value from a rusqlite Row.
///
/// SQLite uses dynamic typing, so we try each type in order of specificity.
pub(crate) fn row_value_to_json(row: &privstack_db::rusqlite::Row<'_>, idx: usize) -> serde_json::Value {
    use privstack_db::rusqlite::types::ValueRef;

    match row.get_ref(idx) {
        Ok(ValueRef::Null) => serde_json::Value::Null,
        Ok(ValueRef::Integer(i)) => serde_json::Value::Number(i.into()),
        Ok(ValueRef::Real(f)) => {
            serde_json::Number::from_f64(f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        Ok(ValueRef::Text(bytes)) => {
            let s = String::from_utf8_lossy(bytes).to_string();
            serde_json::Value::String(s)
        }
        Ok(ValueRef::Blob(bytes)) => {
            // Encode blob as base64 string for JSON representation
            use serde_json::Value;
            Value::String(format!("<blob:{} bytes>", bytes.len()))
        }
        Err(_) => serde_json::Value::Null,
    }
}

/// Build a SELECT clause that quotes all column names.
/// Unlike DuckDB, SQLite doesn't need temporal casting — text columns store
/// date/timestamp values as ISO strings already.
pub(crate) fn build_typed_select(columns: &[DatasetColumn]) -> String {
    columns
        .iter()
        .map(|c| {
            let name = sanitize_identifier(&c.name);
            format!("\"{name}\"")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Current time in milliseconds since Unix epoch.
pub(crate) fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
