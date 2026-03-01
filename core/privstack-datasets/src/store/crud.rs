//! Core CRUD operations: import, list, get, delete, rename.

use super::helpers::now_millis;
use super::DatasetStore;
use crate::error::{DatasetError, DatasetResult};
use crate::schema::dataset_table_name;
use crate::types::{DatasetColumn, DatasetColumnType, DatasetId, DatasetMeta};
use privstack_db::rusqlite::params;
use std::path::Path;
use tracing::info;
use uuid::Uuid;

impl DatasetStore {
    /// Import a CSV file into a new dataset.
    pub fn import_csv(&self, file_path: &Path, name: &str) -> DatasetResult<DatasetMeta> {
        if !file_path.exists() {
            return Err(DatasetError::ImportFailed(format!(
                "File not found: {}",
                file_path.display()
            )));
        }

        let content = std::fs::read_to_string(file_path).map_err(|e| {
            DatasetError::ImportFailed(format!("Failed to read CSV file: {e}"))
        })?;

        let source_file_name = file_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string());

        self.import_csv_content_inner(&content, name, None, source_file_name)
    }

    /// List all datasets.
    pub fn list(&self) -> DatasetResult<Vec<DatasetMeta>> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, source_file_name, row_count, columns_json, created_at, modified_at, category FROM _datasets_meta ORDER BY modified_at DESC"
        )?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>();

        drop(stmt);
        drop(conn);

        rows.into_iter()
            .map(|(id, name, source, row_count, cols_json, created, modified, category)| {
                let columns: Vec<DatasetColumn> =
                    serde_json::from_str(&cols_json).unwrap_or_default();
                Ok(DatasetMeta {
                    id: DatasetId(Uuid::parse_str(&id).map_err(|e| {
                        DatasetError::ImportFailed(format!("Invalid UUID: {e}"))
                    })?),
                    name,
                    source_file_name: source,
                    row_count,
                    columns,
                    category,
                    created_at: created,
                    modified_at: modified,
                })
            })
            .collect()
    }

    /// Get a single dataset's metadata by ID.
    pub fn get(&self, id: &DatasetId) -> DatasetResult<DatasetMeta> {
        let conn = self.lock_conn();
        let result = conn.query_row(
            "SELECT name, source_file_name, row_count, columns_json, created_at, modified_at, category FROM _datasets_meta WHERE id = ?1",
            params![id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            },
        );

        match result {
            Ok((name, source, row_count, cols_json, created, modified, category)) => {
                let columns: Vec<DatasetColumn> =
                    serde_json::from_str(&cols_json).unwrap_or_default();
                Ok(DatasetMeta {
                    id: id.clone(),
                    name,
                    source_file_name: source,
                    row_count,
                    columns,
                    category,
                    created_at: created,
                    modified_at: modified,
                })
            }
            Err(privstack_db::rusqlite::Error::QueryReturnedNoRows) => {
                Err(DatasetError::NotFound(id.to_string()))
            }
            Err(e) => Err(DatasetError::Sqlite(e)),
        }
    }

    /// Delete a dataset and its backing table.
    pub fn delete(&self, id: &DatasetId) -> DatasetResult<()> {
        let table = dataset_table_name(id);
        let conn = self.lock_conn();

        conn.execute_batch(&format!("DROP TABLE IF EXISTS {table}"))?;

        // Delete FK-dependent rows before the meta row to avoid constraint violations
        conn.execute(
            "DELETE FROM _dataset_relations WHERE source_dataset_id = ?1 OR target_dataset_id = ?2",
            params![id.to_string(), id.to_string()],
        )?;
        conn.execute(
            "DELETE FROM _dataset_row_pages WHERE dataset_id = ?1",
            params![id.to_string()],
        )?;
        conn.execute(
            "DELETE FROM _dataset_views WHERE dataset_id = ?1",
            params![id.to_string()],
        )?;

        let deleted = conn.execute(
            "DELETE FROM _datasets_meta WHERE id = ?1",
            params![id.to_string()],
        )?;

        if deleted == 0 {
            return Err(DatasetError::NotFound(id.to_string()));
        }

        info!(dataset_id = %id, "Dataset deleted");
        Ok(())
    }

    /// Set or clear the category for a dataset.
    pub fn set_category(&self, id: &DatasetId, category: Option<&str>) -> DatasetResult<()> {
        let now = now_millis();
        let conn = self.lock_conn();
        let updated = conn.execute(
            "UPDATE _datasets_meta SET category = ?1, modified_at = ?2 WHERE id = ?3",
            params![category, now, id.to_string()],
        )?;

        if updated == 0 {
            return Err(DatasetError::NotFound(id.to_string()));
        }
        Ok(())
    }

    /// Rename a dataset.
    pub fn rename(&self, id: &DatasetId, new_name: &str) -> DatasetResult<()> {
        let now = now_millis();
        let conn = self.lock_conn();
        let updated = conn.execute(
            "UPDATE _datasets_meta SET name = ?1, modified_at = ?2 WHERE id = ?3",
            params![new_name, now, id.to_string()],
        )?;

        if updated == 0 {
            return Err(DatasetError::NotFound(id.to_string()));
        }
        Ok(())
    }

    /// Internal CSV import implementation shared by `import_csv` and `import_csv_content`.
    pub(crate) fn import_csv_content_inner(
        &self,
        csv_content: &str,
        name: &str,
        category: Option<&str>,
        source_file_name: Option<String>,
    ) -> DatasetResult<DatasetMeta> {
        let id = DatasetId::new();
        let table = dataset_table_name(&id);
        let now = now_millis();

        // Parse CSV headers and records
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(csv_content.as_bytes());

        let headers: Vec<String> = reader
            .headers()
            .map_err(|e| DatasetError::ImportFailed(format!("Failed to read CSV headers: {e}")))?
            .iter()
            .map(|h| h.to_string())
            .collect();

        if headers.is_empty() {
            return Err(DatasetError::ImportFailed(
                "CSV has no columns".to_string(),
            ));
        }

        // Read all records into memory for type inference
        let records: Vec<csv::StringRecord> = reader
            .records()
            .filter_map(|r| r.ok())
            .collect();

        // Infer column types from the data (sample up to 100 rows)
        let column_types = infer_column_types(&headers, &records);

        // Create the table with inferred schema
        let col_defs: Vec<String> = headers
            .iter()
            .zip(column_types.iter())
            .map(|(name, col_type)| {
                let safe_name = super::helpers::sanitize_identifier(name);
                format!("\"{}\" {}", safe_name, col_type.to_sqlite_type())
            })
            .collect();
        let create_sql = format!("CREATE TABLE {table} ({})", col_defs.join(", "));

        let conn = self.lock_conn();
        conn.execute_batch(&create_sql).map_err(|e| {
            DatasetError::ImportFailed(format!("Failed to create table: {e}"))
        })?;

        // Insert all rows in a transaction
        let placeholders: Vec<&str> = headers.iter().map(|_| "?").collect();
        let insert_sql = format!(
            "INSERT INTO {table} VALUES ({})",
            placeholders.join(", ")
        );

        conn.execute_batch("BEGIN")?;
        {
            let mut insert_stmt = conn.prepare(&insert_sql)?;
            for record in &records {
                let params: Vec<Box<dyn privstack_db::rusqlite::types::ToSql>> = record
                    .iter()
                    .enumerate()
                    .map(|(i, val)| csv_value_to_sql_param(val, &column_types[i]))
                    .collect();
                let param_refs: Vec<&dyn privstack_db::rusqlite::types::ToSql> =
                    params.iter().map(|p| p.as_ref()).collect();
                insert_stmt.execute(param_refs.as_slice())?;
            }
        }
        conn.execute_batch("COMMIT")?;

        let row_count = records.len() as i64;

        let columns: Vec<DatasetColumn> = headers
            .iter()
            .zip(column_types.iter())
            .enumerate()
            .map(|(i, (name, col_type))| DatasetColumn {
                name: name.clone(),
                column_type: col_type.clone(),
                ordinal: i as i32,
            })
            .collect();
        let columns_json = serde_json::to_string(&columns)?;

        conn.execute(
            r#"INSERT INTO _datasets_meta (id, name, source_file_name, row_count, columns_json, category, created_at, modified_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            params![
                id.to_string(),
                name,
                source_file_name,
                row_count,
                columns_json,
                category,
                now,
                now,
            ],
        )?;

        info!(dataset_id = %id, name, row_count, "Dataset imported");

        Ok(DatasetMeta {
            id,
            name: name.to_string(),
            source_file_name,
            row_count,
            columns,
            category: category.map(|s| s.to_string()),
            created_at: now,
            modified_at: now,
        })
    }
}

/// Infer column types from CSV data by sampling records.
fn infer_column_types(
    headers: &[String],
    records: &[csv::StringRecord],
) -> Vec<DatasetColumnType> {
    let sample_size = records.len().min(100);
    let mut types = vec![DatasetColumnType::Integer; headers.len()];

    // Start with Integer (most specific) and widen as needed:
    // Integer -> Float -> Text
    for record in records.iter().take(sample_size) {
        for (i, field) in record.iter().enumerate() {
            if i >= types.len() {
                break;
            }
            let field = field.trim();
            if field.is_empty() {
                continue; // NULL — doesn't affect type inference
            }
            match &types[i] {
                DatasetColumnType::Integer => {
                    if field.parse::<i64>().is_err() {
                        if field.parse::<f64>().is_ok() {
                            types[i] = DatasetColumnType::Float;
                        } else {
                            types[i] = DatasetColumnType::Text;
                        }
                    }
                }
                DatasetColumnType::Float => {
                    if field.parse::<f64>().is_err() {
                        types[i] = DatasetColumnType::Text;
                    }
                }
                DatasetColumnType::Text => {
                    // Text is the widest type — no further widening needed
                }
                _ => {}
            }
        }
    }

    // If no data rows, default all columns to Text
    if records.is_empty() {
        types = vec![DatasetColumnType::Text; headers.len()];
    }

    types
}

/// Convert a CSV field value to a SQLite parameter based on inferred type.
fn csv_value_to_sql_param(
    value: &str,
    col_type: &DatasetColumnType,
) -> Box<dyn privstack_db::rusqlite::types::ToSql> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Box::new(Option::<String>::None);
    }
    match col_type {
        DatasetColumnType::Integer => {
            if let Ok(v) = trimmed.parse::<i64>() {
                Box::new(v)
            } else {
                Box::new(trimmed.to_string())
            }
        }
        DatasetColumnType::Float => {
            if let Ok(v) = trimmed.parse::<f64>() {
                Box::new(v)
            } else {
                Box::new(trimmed.to_string())
            }
        }
        _ => Box::new(trimmed.to_string()),
    }
}
