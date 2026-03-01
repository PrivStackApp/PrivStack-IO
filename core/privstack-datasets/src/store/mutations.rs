//! Mutation operations: dataset creation, row CRUD, column CRUD, SQL mutations with dry-run.

use super::helpers::{introspect_columns, now_millis, row_value_to_json, sanitize_identifier};
use super::DatasetStore;
use crate::error::{DatasetError, DatasetResult};
use crate::schema::dataset_table_name;
use crate::types::{ColumnDef, DatasetId, DatasetMeta, DatasetQueryResult, MutationResult};
use privstack_db::rusqlite::params;
use tracing::info;

impl DatasetStore {
    /// Create an empty dataset with a defined schema.
    pub fn create_empty(
        &self,
        name: &str,
        columns: &[ColumnDef],
        category: Option<&str>,
    ) -> DatasetResult<DatasetMeta> {
        if columns.is_empty() {
            return Err(DatasetError::InvalidQuery(
                "At least one column is required".to_string(),
            ));
        }

        let id = DatasetId::new();
        let table = dataset_table_name(&id);
        let now = now_millis();

        let col_defs: Vec<String> = columns
            .iter()
            .map(|c| {
                format!(
                    "\"{}\" {}",
                    sanitize_identifier(&c.name),
                    normalize_sql_type(&c.column_type)
                )
            })
            .collect();
        let create_sql = format!("CREATE TABLE {table} ({})", col_defs.join(", "));

        let conn = self.lock_conn();
        conn.execute_batch(&create_sql)?;

        let ds_columns = introspect_columns(&conn, &table)?;
        let columns_json = serde_json::to_string(&ds_columns)?;

        conn.execute(
            r#"INSERT INTO _datasets_meta (id, name, source_file_name, row_count, columns_json, category, created_at, modified_at)
               VALUES (?1, ?2, NULL, 0, ?3, ?4, ?5, ?6)"#,
            params![id.to_string(), name, columns_json, category, now, now],
        )?;

        info!(dataset_id = %id, name, "Empty dataset created");

        Ok(DatasetMeta {
            id,
            name: name.to_string(),
            source_file_name: None,
            row_count: 0,
            columns: ds_columns,
            category: category.map(|s| s.to_string()),
            created_at: now,
            modified_at: now,
        })
    }

    /// Duplicate an existing dataset (schema + data).
    pub fn duplicate(
        &self,
        source_id: &DatasetId,
        new_name: &str,
    ) -> DatasetResult<DatasetMeta> {
        let source_table = dataset_table_name(source_id);
        let new_id = DatasetId::new();
        let new_table = dataset_table_name(&new_id);
        let now = now_millis();

        let conn = self.lock_conn();

        // Read source category before duplicating
        let source_category: Option<String> = conn
            .query_row(
                "SELECT category FROM _datasets_meta WHERE id = ?1",
                params![source_id.to_string()],
                |row| row.get(0),
            )
            .unwrap_or(None);

        let create_sql = format!("CREATE TABLE {new_table} AS SELECT * FROM {source_table}");
        conn.execute_batch(&create_sql).map_err(|e| {
            DatasetError::ImportFailed(format!("Failed to duplicate dataset: {e}"))
        })?;

        let columns = introspect_columns(&conn, &new_table)?;
        let row_count: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {new_table}"), [], |row| {
                row.get(0)
            })?;
        let columns_json = serde_json::to_string(&columns)?;

        conn.execute(
            r#"INSERT INTO _datasets_meta (id, name, source_file_name, row_count, columns_json, category, created_at, modified_at)
               VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7)"#,
            params![new_id.to_string(), new_name, row_count, columns_json, source_category, now, now],
        )?;

        info!(dataset_id = %new_id, new_name, "Dataset duplicated from {}", source_id);

        Ok(DatasetMeta {
            id: new_id,
            name: new_name.to_string(),
            source_file_name: None,
            row_count,
            columns,
            category: source_category,
            created_at: now,
            modified_at: now,
        })
    }

    /// Import dataset from CSV content string (for clipboard paste).
    pub fn import_csv_content(
        &self,
        csv_content: &str,
        name: &str,
        category: Option<&str>,
    ) -> DatasetResult<DatasetMeta> {
        self.import_csv_content_inner(csv_content, name, category, None)
    }

    /// Insert a new row into a dataset.
    pub fn insert_row(
        &self,
        id: &DatasetId,
        values: &[(&str, serde_json::Value)],
    ) -> DatasetResult<()> {
        let table = dataset_table_name(id);
        let now = now_millis();

        let col_names: Vec<String> = values
            .iter()
            .map(|(name, _)| format!("\"{}\"", sanitize_identifier(name)))
            .collect();
        let placeholders: Vec<String> = (1..=values.len()).map(|i| format!("?{i}")).collect();

        let sql = format!(
            "INSERT INTO {table} ({}) VALUES ({})",
            col_names.join(", "),
            placeholders.join(", ")
        );

        let conn = self.lock_conn();

        // Build parameter list from JSON values
        let str_vals: Vec<String> = values
            .iter()
            .map(|(_, v)| json_value_to_sql_string(v))
            .collect();
        let param_refs: Vec<&dyn privstack_db::rusqlite::types::ToSql> = str_vals
            .iter()
            .map(|s| s as &dyn privstack_db::rusqlite::types::ToSql)
            .collect();

        conn.execute(&sql, param_refs.as_slice())?;
        self.update_row_count_and_meta(&conn, id, &table, now)?;

        Ok(())
    }

    /// Update a single cell value.
    pub fn update_cell(
        &self,
        id: &DatasetId,
        row_index: i64,
        column: &str,
        value: serde_json::Value,
    ) -> DatasetResult<()> {
        let table = dataset_table_name(id);
        let now = now_millis();
        let col = sanitize_identifier(column);
        let val_str = json_value_to_sql_string(&value);

        // Use SQLite's rowid pseudo-column for stable row addressing
        let sql = format!(
            "UPDATE {table} SET \"{col}\" = ?1 WHERE rowid = (SELECT rowid FROM {table} LIMIT 1 OFFSET ?2)"
        );

        let conn = self.lock_conn();
        conn.execute(&sql, params![val_str, row_index])?;

        conn.execute(
            "UPDATE _datasets_meta SET modified_at = ?1 WHERE id = ?2",
            params![now, id.to_string()],
        )?;

        Ok(())
    }

    /// Delete rows by their indices.
    pub fn delete_rows(
        &self,
        id: &DatasetId,
        row_indices: &[i64],
    ) -> DatasetResult<()> {
        if row_indices.is_empty() {
            return Ok(());
        }

        let table = dataset_table_name(id);
        let now = now_millis();
        let conn = self.lock_conn();

        // Delete using rowid-based subqueries for each index
        for &idx in row_indices {
            let sql = format!(
                "DELETE FROM {table} WHERE rowid = (SELECT rowid FROM {table} LIMIT 1 OFFSET ?1)"
            );
            conn.execute(&sql, params![idx])?;
        }

        self.update_row_count_and_meta(&conn, id, &table, now)?;
        Ok(())
    }

    /// Add a column to a dataset.
    pub fn add_column(
        &self,
        id: &DatasetId,
        name: &str,
        col_type: &str,
        default: Option<&str>,
    ) -> DatasetResult<()> {
        let table = dataset_table_name(id);
        let now = now_millis();
        let col = sanitize_identifier(name);
        let dtype = normalize_sql_type(col_type);

        let default_clause = match default {
            Some(d) => format!(" DEFAULT '{}'", d.replace('\'', "''")),
            None => String::new(),
        };

        let sql = format!("ALTER TABLE {table} ADD COLUMN \"{col}\" {dtype}{default_clause}");

        let conn = self.lock_conn();
        conn.execute_batch(&sql)?;

        // Refresh column metadata
        let columns = introspect_columns(&conn, &table)?;
        let columns_json = serde_json::to_string(&columns)?;
        conn.execute(
            "UPDATE _datasets_meta SET columns_json = ?1, modified_at = ?2 WHERE id = ?3",
            params![columns_json, now, id.to_string()],
        )?;

        Ok(())
    }

    /// Drop a column from a dataset.
    ///
    /// SQLite 3.35.0+ supports `ALTER TABLE DROP COLUMN`.
    pub fn drop_column(&self, id: &DatasetId, name: &str) -> DatasetResult<()> {
        let table = dataset_table_name(id);
        let now = now_millis();
        let col = sanitize_identifier(name);

        let sql = format!("ALTER TABLE {table} DROP COLUMN \"{col}\"");

        let conn = self.lock_conn();
        conn.execute_batch(&sql)?;

        let columns = introspect_columns(&conn, &table)?;
        let columns_json = serde_json::to_string(&columns)?;
        conn.execute(
            "UPDATE _datasets_meta SET columns_json = ?1, modified_at = ?2 WHERE id = ?3",
            params![columns_json, now, id.to_string()],
        )?;

        Ok(())
    }

    /// Change a column's data type.
    ///
    /// SQLite doesn't support `ALTER COLUMN SET DATA TYPE` directly, so we
    /// rebuild the table: create a new table with the modified schema, copy
    /// data with CAST, drop the old table, and rename.
    pub fn alter_column_type(
        &self,
        id: &DatasetId,
        column: &str,
        new_type: &str,
    ) -> DatasetResult<()> {
        const VALID_TYPES: &[&str] = &[
            "TEXT", "INTEGER", "BIGINT", "REAL", "DOUBLE", "FLOAT", "BOOLEAN",
            "DATE", "TIMESTAMP", "SMALLINT", "TINYINT", "VARCHAR",
        ];
        let dtype = new_type.to_uppercase();
        if !VALID_TYPES.contains(&dtype.as_str()) {
            return Err(DatasetError::InvalidQuery(
                format!("Unsupported column type: {new_type}"),
            ));
        }

        let table = dataset_table_name(id);
        let now = now_millis();
        let target_col = sanitize_identifier(column);
        let sqlite_type = normalize_sql_type(&dtype);

        let conn = self.lock_conn();

        // Get current columns
        let current_columns = introspect_columns(&conn, &table)?;

        // Build new table schema with the modified column type
        let tmp_table = format!("{table}_rebuild");
        let col_defs: Vec<String> = current_columns
            .iter()
            .map(|c| {
                let safe_name = sanitize_identifier(&c.name);
                if safe_name == target_col {
                    format!("\"{}\" {}", safe_name, sqlite_type)
                } else {
                    format!("\"{}\" {}", safe_name, c.column_type.to_sqlite_type())
                }
            })
            .collect();

        // Build SELECT with CAST for the target column
        let select_cols: Vec<String> = current_columns
            .iter()
            .map(|c| {
                let safe_name = sanitize_identifier(&c.name);
                if safe_name == target_col {
                    format!("CAST(\"{safe_name}\" AS {sqlite_type}) AS \"{safe_name}\"")
                } else {
                    format!("\"{safe_name}\"")
                }
            })
            .collect();

        let rebuild_sql = format!(
            "CREATE TABLE {tmp_table} ({});\n\
             INSERT INTO {tmp_table} SELECT {} FROM {table};\n\
             DROP TABLE {table};\n\
             ALTER TABLE {tmp_table} RENAME TO {table};",
            col_defs.join(", "),
            select_cols.join(", "),
        );

        conn.execute_batch(&rebuild_sql)?;

        let columns = introspect_columns(&conn, &table)?;
        let columns_json = serde_json::to_string(&columns)?;
        conn.execute(
            "UPDATE _datasets_meta SET columns_json = ?1, modified_at = ?2 WHERE id = ?3",
            params![columns_json, now, id.to_string()],
        )?;

        Ok(())
    }

    /// Rename a column in a dataset.
    pub fn rename_column(
        &self,
        id: &DatasetId,
        old_name: &str,
        new_name: &str,
    ) -> DatasetResult<()> {
        let table = dataset_table_name(id);
        let now = now_millis();
        let old_col = sanitize_identifier(old_name);
        let new_col = sanitize_identifier(new_name);

        let sql = format!("ALTER TABLE {table} RENAME COLUMN \"{old_col}\" TO \"{new_col}\"");

        let conn = self.lock_conn();
        conn.execute_batch(&sql)?;

        let columns = introspect_columns(&conn, &table)?;
        let columns_json = serde_json::to_string(&columns)?;
        conn.execute(
            "UPDATE _datasets_meta SET columns_json = ?1, modified_at = ?2 WHERE id = ?3",
            params![columns_json, now, id.to_string()],
        )?;

        Ok(())
    }

    /// Execute a SQL mutation (INSERT/UPDATE/DELETE/CREATE/ALTER) with optional dry-run.
    ///
    /// When `dry_run` is true, the mutation is executed inside a SAVEPOINT that is
    /// rolled back, returning a preview of affected rows without persisting changes.
    pub fn execute_mutation(
        &self,
        sql: &str,
        dry_run: bool,
    ) -> DatasetResult<MutationResult> {
        let stmt_type = classify_statement(sql);
        let conn = self.lock_conn();

        if dry_run {
            conn.execute_batch("SAVEPOINT dry_run")?;

            let execute_result = conn.execute(sql, []);
            match execute_result {
                Ok(affected) => {
                    let preview = self.query_mutation_preview(&conn, sql, &stmt_type);
                    conn.execute_batch("ROLLBACK TO SAVEPOINT dry_run")?;
                    conn.execute_batch("RELEASE SAVEPOINT dry_run")?;

                    Ok(MutationResult {
                        affected_rows: affected as i64,
                        statement_type: stmt_type,
                        committed: false,
                        preview: preview.ok(),
                    })
                }
                Err(e) => {
                    conn.execute_batch("ROLLBACK TO SAVEPOINT dry_run")?;
                    conn.execute_batch("RELEASE SAVEPOINT dry_run")?;
                    Err(DatasetError::Sqlite(e))
                }
            }
        } else {
            let affected = conn.execute(sql, [])?;
            Ok(MutationResult {
                affected_rows: affected as i64,
                statement_type: stmt_type,
                committed: true,
                preview: None,
            })
        }
    }

    /// Query a preview of mutation results (called inside dry-run savepoint).
    fn query_mutation_preview(
        &self,
        conn: &privstack_db::rusqlite::Connection,
        sql: &str,
        _stmt_type: &str,
    ) -> DatasetResult<DatasetQueryResult> {
        // Try to extract table name and query it for preview
        let table_name = extract_table_name(sql);
        if let Some(table) = table_name {
            // Get column names via PRAGMA
            let col_names = {
                let mut pragma_stmt = conn.prepare(&format!("PRAGMA table_info('{table}')"))?;
                let names: Vec<String> = pragma_stmt
                    .query_map([], |row| row.get::<_, String>(1))?
                    .filter_map(|r| r.ok())
                    .collect();
                names
            };
            let col_count = col_names.len();

            let preview_sql = format!("SELECT * FROM {table} LIMIT 50");
            let mut stmt = conn.prepare(&preview_sql)?;
            let rows: Vec<Vec<serde_json::Value>> = stmt
                .query_map([], |row| {
                    let mut vals = Vec::with_capacity(col_count);
                    for i in 0..col_count {
                        vals.push(row_value_to_json(row, i));
                    }
                    Ok(vals)
                })?
                .filter_map(|r| r.ok())
                .collect();

            let total: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap_or(rows.len() as i64);

            return Ok(DatasetQueryResult {
                columns: col_names,
                column_types: vec![],
                rows,
                total_count: total,
                page: 0,
                page_size: 50,
            });
        }

        Ok(DatasetQueryResult {
            columns: vec![],
            column_types: vec![],
            rows: vec![],
            total_count: 0,
            page: 0,
            page_size: 0,
        })
    }

    /// Helper: update row count and column metadata after a mutation.
    fn update_row_count_and_meta(
        &self,
        conn: &privstack_db::rusqlite::Connection,
        id: &DatasetId,
        table: &str,
        now: i64,
    ) -> DatasetResult<()> {
        let row_count: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })?;

        conn.execute(
            "UPDATE _datasets_meta SET row_count = ?1, modified_at = ?2 WHERE id = ?3",
            params![row_count, now, id.to_string()],
        )?;

        Ok(())
    }
}

/// Classify a SQL statement by its first keyword.
fn classify_statement(sql: &str) -> String {
    let upper = sql.trim().to_uppercase();
    if upper.starts_with("SELECT") {
        "SELECT".to_string()
    } else if upper.starts_with("INSERT") {
        "INSERT".to_string()
    } else if upper.starts_with("UPDATE") {
        "UPDATE".to_string()
    } else if upper.starts_with("DELETE") {
        "DELETE".to_string()
    } else if upper.starts_with("CREATE") {
        "CREATE".to_string()
    } else if upper.starts_with("ALTER") {
        "ALTER".to_string()
    } else {
        "OTHER".to_string()
    }
}

/// Extract the target table name from a SQL statement (best-effort).
fn extract_table_name(sql: &str) -> Option<String> {
    let upper = sql.trim().to_uppercase();
    let tokens: Vec<&str> = sql.split_whitespace().collect();

    if upper.starts_with("INSERT INTO") {
        tokens.get(2).map(|t| t.trim_matches('(').to_string())
    } else if upper.starts_with("UPDATE") {
        tokens.get(1).map(|s| s.to_string())
    } else if upper.starts_with("DELETE FROM") {
        tokens.get(2).map(|s| s.to_string())
    } else if upper.starts_with("CREATE TABLE") {
        // Skip IF NOT EXISTS
        if upper.contains("IF NOT EXISTS") {
            tokens.get(5).map(|t| t.trim_matches('(').to_string())
        } else {
            tokens.get(2).map(|t| t.trim_matches('(').to_string())
        }
    } else if upper.starts_with("ALTER TABLE") {
        tokens.get(2).map(|s| s.to_string())
    } else {
        None
    }
}

/// Convert a serde_json::Value to a SQL-safe string for parameterized queries.
fn json_value_to_sql_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Normalize SQLite/generic SQL type names to SQLite-compatible types.
fn normalize_sql_type(type_name: &str) -> String {
    let upper = type_name.to_uppercase();
    match upper.as_str() {
        "VARCHAR" | "TEXT" | "CHAR" | "CLOB" | "STRING" => "TEXT".to_string(),
        "BIGINT" | "SMALLINT" | "TINYINT" | "HUGEINT" | "INT" | "INTEGER" => "INTEGER".to_string(),
        "DOUBLE" | "FLOAT" | "REAL" | "DECIMAL" | "NUMERIC" => "REAL".to_string(),
        "BOOLEAN" | "BOOL" => "INTEGER".to_string(),
        "DATE" | "TIMESTAMP" | "DATETIME" => "TEXT".to_string(),
        "BLOB" => "BLOB".to_string(),
        _ => upper,
    }
}
