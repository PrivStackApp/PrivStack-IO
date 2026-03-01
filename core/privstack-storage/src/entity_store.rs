//! Generic entity store — stores any entity type as JSON with indexed fields.

use crate::error::{StorageError, StorageResult};
use privstack_db::rusqlite::{params, Connection};
use privstack_model::{Entity, EntitySchema, FieldType, IndexedField};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Generic entity store backed by SQLite.
///
/// Stores entities of any type in a single `entities` table with
/// schema-driven field extraction for indexing and search.
#[derive(Clone)]
pub struct EntityStore {
    conn: Arc<Mutex<Connection>>,
}

impl EntityStore {
    /// Opens or creates an entity store at the given path.
    pub fn open(path: &Path) -> StorageResult<Self> {
        let conn = privstack_db::open_db_unencrypted(path)
            .map_err(StorageError::Db)?;
        privstack_db::register_custom_functions(&conn)
            .map_err(StorageError::Db)?;
        initialize_entity_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Opens an in-memory entity store (for testing).
    pub fn open_in_memory() -> StorageResult<Self> {
        let conn = privstack_db::open_in_memory()
            .map_err(StorageError::Db)?;
        privstack_db::register_custom_functions(&conn)
            .map_err(StorageError::Db)?;
        initialize_entity_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Creates an entity store from a shared connection.
    pub fn open_with_conn(conn: Arc<Mutex<Connection>>) -> StorageResult<Self> {
        {
            let c = conn.lock().unwrap();
            privstack_db::register_custom_functions(&c)
                .map_err(StorageError::Db)?;
            initialize_entity_schema(&c)?;
        }
        Ok(Self { conn })
    }

    /// Save (upsert) an entity with schema-driven field extraction.
    pub fn save_entity(&self, entity: &Entity, schema: &EntitySchema) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();

        let title = extract_field(&entity.data, &schema.indexed_fields, FieldType::Text, "/title");
        let body = extract_field(&entity.data, &schema.indexed_fields, FieldType::Text, "/body");
        let tags = extract_tags(&entity.data, &schema.indexed_fields);
        let tags_str: Option<String> = if tags.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&tags).unwrap_or_default())
        };

        let search_text = build_search_text(&title, &body, &tags);

        // Auto-index Relation fields as entity_links
        extract_relations(&conn, entity, &schema.indexed_fields)?;

        // Auto-index Vector fields into entity_vectors
        extract_vectors(&conn, entity, &schema.indexed_fields)?;

        let data_json = serde_json::to_string(&entity.data)?;

        conn.execute(
            r#"
            INSERT OR REPLACE INTO entities (
                id, entity_type, data_json, title, body, tags,
                is_trashed, is_favorite, local_only,
                created_at, modified_at, created_by, search_text
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            params![
                entity.id,
                entity.entity_type,
                data_json,
                title.as_deref(),
                body.as_deref(),
                tags_str.as_deref(),
                entity.data.pointer("/is_trashed").and_then(|v| v.as_bool()).unwrap_or(false),
                entity.data.pointer("/is_favorite").and_then(|v| v.as_bool()).unwrap_or(false),
                entity.data.pointer("/local_only").and_then(|v| v.as_bool()).unwrap_or(false),
                entity.created_at,
                entity.modified_at,
                entity.created_by,
                search_text,
            ],
        )?;

        Ok(())
    }

    /// Save an entity without a schema (no field extraction, just raw JSON).
    pub fn save_entity_raw(&self, entity: &Entity) -> StorageResult<()> {
        let data_json = serde_json::to_string(&entity.data)?;

        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT OR REPLACE INTO entities (
                id, entity_type, data_json,
                is_trashed, is_favorite, local_only,
                created_at, modified_at, created_by
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            params![
                entity.id,
                entity.entity_type,
                data_json,
                entity.data.pointer("/is_trashed").and_then(|v| v.as_bool()).unwrap_or(false),
                entity.data.pointer("/is_favorite").and_then(|v| v.as_bool()).unwrap_or(false),
                entity.data.pointer("/local_only").and_then(|v| v.as_bool()).unwrap_or(false),
                entity.created_at,
                entity.modified_at,
                entity.created_by,
            ],
        )?;

        Ok(())
    }

    /// Get a single entity by ID.
    pub fn get_entity(&self, id: &str) -> StorageResult<Option<Entity>> {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT id, entity_type, data_json, created_at, modified_at, created_by, is_trashed FROM entities WHERE id = ?",
            params![id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, bool>(6)?,
                ))
            },
        );

        match result {
            Ok((id, entity_type, data_json, created_at, modified_at, created_by, is_trashed)) => {
                let mut data = serde_json::from_str::<serde_json::Value>(&data_json)?;
                // Patch is_trashed from the authoritative DB column
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("is_trashed".into(), serde_json::Value::Bool(is_trashed));
                }
                Ok(Some(Entity {
                    id,
                    entity_type,
                    data,
                    created_at,
                    modified_at,
                    created_by,
                }))
            }
            Err(privstack_db::rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List entities of a given type, ordered by modified_at DESC.
    pub fn list_entities(
        &self,
        entity_type: &str,
        include_trashed: bool,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> StorageResult<Vec<Entity>> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from(
            "SELECT id, entity_type, data_json, created_at, modified_at, created_by, is_trashed FROM entities WHERE entity_type = ?"
        );
        if !include_trashed {
            sql.push_str(" AND is_trashed = 0");
        }
        sql.push_str(" ORDER BY modified_at DESC");
        if let Some(lim) = limit {
            sql.push_str(&format!(" LIMIT {lim}"));
        }
        if let Some(off) = offset {
            sql.push_str(&format!(" OFFSET {off}"));
        }

        let mut stmt = conn.prepare(&sql)?;
        let rows: Vec<(String, String, String, i64, i64, String, bool)> = stmt
            .query_map(params![entity_type], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, bool>(6)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut entities = Vec::with_capacity(rows.len());
        for (id, entity_type, data_json, created_at, modified_at, created_by, is_trashed) in rows {
            if let Ok(mut data) = serde_json::from_str::<serde_json::Value>(&data_json) {
                // Patch is_trashed from the authoritative DB column
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("is_trashed".into(), serde_json::Value::Bool(is_trashed));
                }
                entities.push(Entity { id, entity_type, data, created_at, modified_at, created_by });
            }
        }
        Ok(entities)
    }

    /// Delete an entity by ID, cascading to all referencing tables.
    pub fn delete_entity(&self, id: &str) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM entity_links WHERE source_id = ? OR target_id = ?",
            params![id, id],
        )?;
        conn.execute("DELETE FROM entity_vectors WHERE entity_id = ?", params![id])?;
        conn.execute("DELETE FROM sync_ledger WHERE entity_id = ?", params![id])?;
        conn.execute("DELETE FROM entities WHERE id = ?", params![id])?;
        Ok(())
    }

    /// Soft-delete (trash) an entity.
    pub fn trash_entity(&self, id: &str) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE entities SET is_trashed = 1 WHERE id = ?", params![id])?;
        Ok(())
    }

    /// Restore a trashed entity.
    pub fn restore_entity(&self, id: &str) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE entities SET is_trashed = 0 WHERE id = ?", params![id])?;
        Ok(())
    }

    /// Query entities by field filters applied against JSON data.
    ///
    /// Fetches all rows for the entity type and applies field filters in Rust.
    pub fn query_entities(
        &self,
        entity_type: &str,
        filters: &[(String, serde_json::Value)],
        include_trashed: bool,
        limit: Option<usize>,
    ) -> StorageResult<Vec<Entity>> {
        // No filters -> delegate to list_entities (avoids duplicated SQL)
        if filters.is_empty() {
            return self.list_entities(entity_type, include_trashed, limit, None);
        }

        let conn = self.conn.lock().unwrap();

        let mut sql = String::from(
            "SELECT id, entity_type, data_json, created_at, modified_at, created_by, is_trashed \
             FROM entities WHERE entity_type = ?"
        );
        if !include_trashed {
            sql.push_str(" AND is_trashed = 0");
        }
        let sql = sql + " ORDER BY modified_at DESC";

        let mut stmt = conn.prepare(&sql)?;
        let rows: Vec<(String, String, String, i64, i64, String, bool)> = stmt
            .query_map(params![entity_type], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, bool>(6)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut entities = Vec::new();
        for (id, entity_type, data_json, created_at, modified_at, created_by, is_trashed) in rows {
            if let Ok(mut data) = serde_json::from_str::<serde_json::Value>(&data_json) {
                // Patch is_trashed from the authoritative DB column
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("is_trashed".into(), serde_json::Value::Bool(is_trashed));
                }

                // Apply every filter against the JSON data
                let matches = filters.iter().all(|(field_path, expected)| {
                    let pointer = if field_path.starts_with('/') {
                        field_path.clone()
                    } else {
                        format!("/{field_path}")
                    };
                    match data.pointer(&pointer) {
                        Some(actual) => match_filter_value(actual, expected),
                        None => false,
                    }
                });

                if matches {
                    entities.push(Entity { id, entity_type, data, created_at, modified_at, created_by });
                    if let Some(lim) = limit {
                        if entities.len() >= lim {
                            break;
                        }
                    }
                }
            }
        }
        Ok(entities)
    }

    /// Search entities across all types (or a subset) using text matching.
    /// Searches against plaintext indexed columns (title, search_text).
    pub fn search(
        &self,
        query: &str,
        entity_types: Option<&[&str]>,
        limit: usize,
    ) -> StorageResult<Vec<Entity>> {
        let conn = self.conn.lock().unwrap();

        let pattern = format!("%{query}%");
        let mut sql = String::from(
            "SELECT id, entity_type, data_json, created_at, modified_at, created_by FROM entities WHERE is_trashed = 0 AND (search_text LIKE ? OR title LIKE ?)"
        );

        if let Some(types) = entity_types {
            if !types.is_empty() {
                let in_clause = types.iter().map(|t| format!("'{}'", t.replace('\'', "''"))).collect::<Vec<_>>().join(",");
                sql.push_str(&format!(" AND entity_type IN ({in_clause})"));
            }
        }

        sql.push_str(&format!(" ORDER BY modified_at DESC LIMIT {limit}"));

        let mut stmt = conn.prepare(&sql)?;
        let rows: Vec<(String, String, String, i64, i64, String)> = stmt
            .query_map(params![pattern, pattern], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut entities = Vec::with_capacity(rows.len());
        for (id, entity_type, data_json, created_at, modified_at, created_by) in rows {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&data_json) {
                entities.push(Entity { id, entity_type, data, created_at, modified_at, created_by });
            }
        }
        Ok(entities)
    }

    /// Returns all entity IDs in the store (non-trashed).
    /// Used by the sync orchestrator on first sync with a new peer.
    pub fn list_all_entity_ids(&self) -> StorageResult<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id FROM entities WHERE is_trashed = 0")?;
        let ids: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }

    // -- Sync Ledger --

    /// Returns entity IDs that need syncing with a specific peer.
    pub fn entities_needing_sync(&self, peer_id: &str) -> StorageResult<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT e.id FROM entities e \
             LEFT JOIN sync_ledger sl ON e.id = sl.entity_id AND sl.peer_id = ? \
             WHERE e.is_trashed = 0 \
               AND e.local_only = 0 \
               AND (sl.entity_id IS NULL OR e.modified_at > sl.synced_at) \
             ORDER BY e.modified_at ASC"
        )?;
        let ids: Vec<String> = stmt
            .query_map(params![peer_id], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }

    /// Marks a single entity as synced with a peer (upsert).
    pub fn mark_entity_synced(&self, peer_id: &str, entity_id: &str, synced_at_ms: i64) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO sync_ledger (peer_id, entity_id, synced_at) VALUES (?, ?, ?)",
            params![peer_id, entity_id, synced_at_ms],
        )?;
        Ok(())
    }

    /// Marks multiple entities as synced with a peer in a single transaction.
    pub fn mark_entities_synced(&self, peer_id: &str, entity_ids: &[String], synced_at_ms: i64) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "INSERT OR REPLACE INTO sync_ledger (peer_id, entity_id, synced_at) VALUES (?, ?, ?)"
        )?;
        for eid in entity_ids {
            stmt.execute(params![peer_id, eid, synced_at_ms])?;
        }
        Ok(())
    }

    /// Removes all sync ledger entries for a peer (e.g., when untrusting).
    pub fn clear_sync_ledger_for_peer(&self, peer_id: &str) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM sync_ledger WHERE peer_id = ?", params![peer_id])?;
        Ok(())
    }

    /// Removes all sync ledger entries for an entity across all peers.
    /// Forces the entity to be re-synced with every peer on the next cycle.
    pub fn invalidate_sync_ledger_for_entity(&self, entity_id: &str) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM sync_ledger WHERE entity_id = ?", params![entity_id])?;
        Ok(())
    }

    /// Count entities of a given type.
    pub fn count_entities(&self, entity_type: &str, include_trashed: bool) -> StorageResult<usize> {
        let conn = self.conn.lock().unwrap();

        let sql = if include_trashed {
            "SELECT COUNT(*) FROM entities WHERE entity_type = ?"
        } else {
            "SELECT COUNT(*) FROM entities WHERE entity_type = ? AND is_trashed = 0"
        };

        let count: i64 = conn.query_row(sql, params![entity_type], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Estimate storage bytes used by entities of a given type.
    pub fn estimate_storage_bytes(&self, entity_type: &str) -> StorageResult<usize> {
        let conn = self.conn.lock().unwrap();
        let sql = "SELECT COALESCE(SUM(LENGTH(data_json)), 0) FROM entities WHERE entity_type = ?";
        let bytes: i64 = conn.query_row(sql, params![entity_type], |row| row.get(0))?;
        Ok(bytes as usize)
    }

    /// Estimate storage bytes for multiple entity types at once.
    pub fn estimate_storage_by_types(&self, entity_types: &[&str]) -> StorageResult<Vec<(String, usize, usize)>> {
        let conn = self.conn.lock().unwrap();
        let mut results = Vec::with_capacity(entity_types.len());

        for entity_type in entity_types {
            let sql = "SELECT COUNT(*), COALESCE(SUM(LENGTH(data_json)), 0) FROM entities WHERE entity_type = ?";
            let (count, bytes): (i64, i64) = conn.query_row(sql, params![*entity_type], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
            results.push((entity_type.to_string(), count as usize, bytes as usize));
        }

        Ok(results)
    }

    /// Save a link between two entities.
    pub fn save_link(
        &self,
        source_type: &str,
        source_id: &str,
        target_type: &str,
        target_id: &str,
    ) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO entity_links (source_type, source_id, target_type, target_id) VALUES (?, ?, ?, ?)",
            params![source_type, source_id, target_type, target_id],
        )?;
        Ok(())
    }

    /// Remove a link between two entities.
    pub fn remove_link(
        &self,
        source_type: &str,
        source_id: &str,
        target_type: &str,
        target_id: &str,
    ) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM entity_links WHERE source_type = ? AND source_id = ? AND target_type = ? AND target_id = ?",
            params![source_type, source_id, target_type, target_id],
        )?;
        Ok(())
    }

    /// Get all entities linked from a source entity.
    pub fn get_links_from(
        &self,
        source_type: &str,
        source_id: &str,
    ) -> StorageResult<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT target_type, target_id FROM entity_links WHERE source_type = ? AND source_id = ?"
        )?;
        let links = stmt
            .query_map(params![source_type, source_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(links)
    }

    /// Get all entities linking to a target entity.
    pub fn get_links_to(
        &self,
        target_type: &str,
        target_id: &str,
    ) -> StorageResult<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT source_type, source_id FROM entity_links WHERE target_type = ? AND target_id = ?"
        )?;
        let links = stmt
            .query_map(params![target_type, target_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(links)
    }

    // =========================================================================
    // Plugin Fuel History Methods
    // =========================================================================

    /// Records a fuel consumption entry for a plugin.
    /// Maintains a rolling window of the last 1000 entries per plugin.
    pub fn record_fuel_consumption(&self, plugin_id: &str, fuel_consumed: u64) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        conn.execute(
            "INSERT INTO plugin_fuel_history (plugin_id, fuel_consumed, recorded_at) VALUES (?, ?, ?)",
            params![plugin_id, fuel_consumed as i64, now],
        )?;

        // Prune old records - keep only the most recent 1000 per plugin
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM plugin_fuel_history WHERE plugin_id = ?",
            params![plugin_id],
            |row| row.get(0),
        )?;

        if count > 1000 {
            conn.execute(
                r#"
                DELETE FROM plugin_fuel_history
                WHERE plugin_id = ?
                  AND recorded_at < (
                    SELECT MIN(recorded_at) FROM (
                      SELECT recorded_at FROM plugin_fuel_history
                      WHERE plugin_id = ?
                      ORDER BY recorded_at DESC
                      LIMIT 1000
                    )
                  )
                "#,
                params![plugin_id, plugin_id],
            )?;
        }

        Ok(())
    }

    /// Gets fuel consumption metrics for a plugin.
    /// Returns (average, peak, count) for the stored history.
    pub fn get_fuel_metrics(&self, plugin_id: &str) -> StorageResult<(u64, u64, usize)> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            r#"
            SELECT
                COALESCE(AVG(fuel_consumed), 0) as avg_fuel,
                COALESCE(MAX(fuel_consumed), 0) as peak_fuel,
                COUNT(*) as call_count
            FROM plugin_fuel_history
            WHERE plugin_id = ?
            "#,
        )?;

        let (avg_fuel, peak_fuel, call_count): (f64, i64, i64) = stmt.query_row(params![plugin_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;

        Ok((avg_fuel as u64, peak_fuel as u64, call_count as usize))
    }

    /// Clears all fuel history for a plugin (e.g., on reset).
    pub fn clear_fuel_history(&self, plugin_id: &str) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM plugin_fuel_history WHERE plugin_id = ?",
            params![plugin_id],
        )?;
        Ok(())
    }

    // -- Cloud Sync Cursor Persistence --

    /// Saves a cloud sync cursor value (e.g. per-entity cursor position or last_sync_at).
    pub fn save_cloud_cursor(&self, key: &str, value: i64) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        conn.execute(
            "INSERT OR REPLACE INTO cloud_sync_cursors (cursor_key, cursor_value, updated_at) VALUES (?, ?, ?)",
            params![key, value, now],
        )?;
        Ok(())
    }

    /// Loads all cloud sync cursors as key-value pairs.
    pub fn load_cloud_cursors(&self) -> StorageResult<Vec<(String, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT cursor_key, cursor_value FROM cloud_sync_cursors"
        )?;
        let rows: Vec<(String, i64)> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// Clears all cloud sync cursors (e.g. when switching workspaces).
    pub fn clear_cloud_cursors(&self) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM cloud_sync_cursors", [])?;
        Ok(())
    }

    /// Flushes the WAL to the main database file.
    pub fn checkpoint(&self) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        privstack_db::checkpoint(&conn).map_err(StorageError::Db)?;
        Ok(())
    }

    /// Runs database maintenance: purge orphaned/transient data, then checkpoint.
    pub fn run_maintenance(&self) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "-- Orphaned rows in auxiliary tables (parent entity deleted but these weren't)
             DELETE FROM entity_vectors WHERE entity_id NOT IN (SELECT id FROM entities);
             DELETE FROM sync_ledger WHERE entity_id NOT IN (SELECT id FROM entities);
             DELETE FROM entity_links WHERE source_id NOT IN (SELECT id FROM entities)
                OR target_id NOT IN (SELECT id FROM entities);
             -- Transient data that rebuilds automatically on next sync
             DELETE FROM cloud_sync_cursors;
             DELETE FROM plugin_fuel_history;"
        )?;
        privstack_db::checkpoint(&conn).map_err(StorageError::Db)?;
        Ok(())
    }

    /// Finds orphan entities whose entity_type doesn't match any known plugin schema.
    pub fn find_orphan_entities(
        &self,
        valid_types: &[(String, String)],
    ) -> StorageResult<Vec<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();

        let known_types: std::collections::HashSet<&str> = valid_types
            .iter()
            .map(|(_, etype)| etype.as_str())
            .collect();

        let mut stmt = conn.prepare(
            "SELECT entity_type, COUNT(*) as cnt
             FROM entities
             GROUP BY entity_type"
        )?;

        let db_types: Vec<(String, i64)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut orphans = Vec::new();
        for (entity_type, count) in &db_types {
            if !known_types.contains(entity_type.as_str()) {
                orphans.push(serde_json::json!({
                    "entity_type": entity_type,
                    "count": count,
                }));
            }
        }
        Ok(orphans)
    }

    /// Deletes orphan entities whose entity_type doesn't match any known plugin schema.
    pub fn delete_orphan_entities(
        &self,
        valid_types: &[(String, String)],
    ) -> StorageResult<usize> {
        let conn = self.conn.lock().unwrap();

        if valid_types.is_empty() {
            return Ok(0);
        }

        let known_types: Vec<String> = valid_types
            .iter()
            .map(|(_, etype)| format!("'{}'", etype.replace('\'', "''")))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        let in_clause = known_types.join(",");

        let query = format!(
            "SELECT id FROM entities WHERE entity_type NOT IN ({})",
            in_clause
        );
        let mut stmt = conn.prepare(&query)?;
        let orphan_ids: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();

        if orphan_ids.is_empty() {
            return Ok(0);
        }

        let id_list: Vec<String> = orphan_ids.iter().map(|id| {
            format!("'{}'", id.replace('\'', "''"))
        }).collect();
        let id_in = id_list.join(",");

        conn.execute_batch(&format!(
            "DELETE FROM entity_vectors WHERE entity_id IN ({id_in});
             DELETE FROM sync_ledger WHERE entity_id IN ({id_in});
             DELETE FROM entity_links WHERE source_id IN ({id_in}) OR target_id IN ({id_in});
             DELETE FROM entities WHERE id IN ({id_in});"
        ))?;

        Ok(orphan_ids.len())
    }

    /// Returns diagnostics for the entity store's SQLite connection.
    pub fn db_diagnostics(&self) -> StorageResult<serde_json::Value> {
        let conn = self.conn.lock().unwrap();
        Ok(scan_db_connection(&conn))
    }

    /// Compacts the database using VACUUM INTO, then swaps the file.
    ///
    /// Returns `(size_before, size_after)` in bytes.
    pub fn compact(&self, db_path: &Path) -> StorageResult<(u64, u64)> {
        let mut conn_guard = self.conn.lock().unwrap();

        let size_before = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);

        // Checkpoint to flush WAL first
        privstack_db::checkpoint(&conn_guard).map_err(StorageError::Db)?;

        // Close the managed connection by replacing with in-memory
        let old_conn = std::mem::replace(
            &mut *conn_guard,
            privstack_db::open_in_memory().map_err(StorageError::Db)?,
        );

        // Compact via VACUUM INTO + swap
        let compact_result = privstack_db::compact(&old_conn, db_path);
        drop(old_conn);

        match compact_result {
            Ok((_, size_after)) => {
                // Reopen the connection
                let new_conn = privstack_db::open_db_unencrypted(db_path)
                    .map_err(StorageError::Db)?;
                privstack_db::register_custom_functions(&new_conn)
                    .map_err(StorageError::Db)?;
                *conn_guard = new_conn;

                eprintln!(
                    "[compact] {} -> {} ({:.1}% reduction)",
                    format_bytes_log(size_before),
                    format_bytes_log(size_after),
                    if size_before > 0 {
                        (1.0 - size_after as f64 / size_before as f64) * 100.0
                    } else {
                        0.0
                    }
                );

                Ok((size_before, size_after))
            }
            Err(e) => {
                // Reopen original on failure
                if let Ok(restored) = privstack_db::open_db_unencrypted(db_path) {
                    let _ = privstack_db::register_custom_functions(&restored);
                    *conn_guard = restored;
                }
                Err(StorageError::Db(e))
            }
        }
    }

    // -- RAG Vector Index --

    /// Upsert a RAG vector entry (INSERT OR REPLACE).
    pub fn rag_upsert(
        &self,
        entity_id: &str,
        chunk_path: &str,
        plugin_id: &str,
        entity_type: &str,
        content_hash: &str,
        dim: i32,
        embedding: &[f64],
        title: &str,
        link_type: &str,
        indexed_at: i64,
        chunk_text: &str,
    ) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        let embedding_json = serde_json::to_string(embedding)?;
        conn.execute(
            "INSERT OR REPLACE INTO rag_vectors \
             (entity_id, chunk_path, plugin_id, entity_type, content_hash, dim, embedding, title, link_type, indexed_at, chunk_text) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![entity_id, chunk_path, plugin_id, entity_type, content_hash, dim, embedding_json, title, link_type, indexed_at, chunk_text],
        )?;
        Ok(())
    }

    /// Search RAG vectors by cosine similarity to a query embedding.
    pub fn rag_search(
        &self,
        query_embedding: &[f64],
        limit: usize,
        entity_types: Option<&[&str]>,
    ) -> StorageResult<Vec<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();
        let query_json = serde_json::to_string(query_embedding)?;

        let mut sql = String::from(
            "SELECT entity_id, entity_type, plugin_id, chunk_path, title, link_type, \
             cosine_similarity(embedding, ?) AS score, \
             chunk_text \
             FROM rag_vectors"
        );

        let mut param_values: Vec<Box<dyn privstack_db::rusqlite::types::ToSql>> = Vec::new();
        param_values.push(Box::new(query_json));

        if let Some(types) = entity_types {
            if !types.is_empty() {
                let placeholders: Vec<&str> = types.iter().map(|_| "?").collect();
                sql.push_str(&format!(" WHERE entity_type IN ({})", placeholders.join(",")));
                for t in types {
                    param_values.push(Box::new(t.to_string()));
                }
            }
        }

        sql.push_str(" ORDER BY score DESC LIMIT ?");
        param_values.push(Box::new(limit as i64));

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn privstack_db::rusqlite::types::ToSql> = param_values.iter().map(|b| b.as_ref()).collect();

        let rows: Vec<serde_json::Value> = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(serde_json::json!({
                    "entity_id": row.get::<_, String>(0)?,
                    "entity_type": row.get::<_, String>(1)?,
                    "plugin_id": row.get::<_, String>(2)?,
                    "chunk_path": row.get::<_, String>(3)?,
                    "title": row.get::<_, String>(4)?,
                    "link_type": row.get::<_, String>(5)?,
                    "score": row.get::<_, f64>(6)?,
                    "chunk_text": row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                }))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Delete all RAG vectors for a given entity.
    pub fn rag_delete(&self, entity_id: &str) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM rag_vectors WHERE entity_id = ?",
            params![entity_id],
        )?;
        Ok(())
    }

    /// Delete all RAG vectors (used during data wipe/reseed).
    pub fn rag_delete_all(&self) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM rag_vectors", params![])?;
        Ok(())
    }

    /// Get content hashes for all chunks of given entity types (for incremental skip).
    pub fn rag_get_hashes(
        &self,
        entity_types: Option<&[&str]>,
    ) -> StorageResult<Vec<(String, String, String)>> {
        let conn = self.conn.lock().unwrap();

        let (sql, type_params) = if let Some(types) = entity_types {
            let placeholders: Vec<&str> = types.iter().map(|_| "?").collect();
            (
                format!(
                    "SELECT entity_id, chunk_path, content_hash FROM rag_vectors WHERE entity_type IN ({})",
                    placeholders.join(",")
                ),
                types.to_vec(),
            )
        } else {
            (
                "SELECT entity_id, chunk_path, content_hash FROM rag_vectors".to_string(),
                vec![],
            )
        };

        let mut stmt = conn.prepare(&sql)?;
        let mut param_values: Vec<Box<dyn privstack_db::rusqlite::types::ToSql>> = Vec::new();
        for t in &type_params {
            param_values.push(Box::new(t.to_string()));
        }
        let param_refs: Vec<&dyn privstack_db::rusqlite::types::ToSql> = param_values.iter().map(|b| b.as_ref()).collect();

        let rows: Vec<(String, String, String)> = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Fetch all RAG vector entries with their embeddings for visualization.
    pub fn rag_fetch_all(
        &self,
        entity_types: Option<&[&str]>,
        limit: usize,
    ) -> StorageResult<Vec<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();

        let (type_filter, type_params) = if let Some(types) = entity_types {
            let placeholders: Vec<&str> = types.iter().map(|_| "?").collect();
            (
                format!("WHERE entity_type IN ({})", placeholders.join(",")),
                types.to_vec(),
            )
        } else {
            (String::new(), vec![])
        };

        let sql = format!(
            "SELECT entity_id, entity_type, plugin_id, chunk_path, title, link_type, \
             embedding, chunk_text \
             FROM rag_vectors {} LIMIT ?",
            type_filter
        );

        let mut stmt = conn.prepare(&sql)?;

        let limit_i64 = limit as i64;
        let mut param_values: Vec<Box<dyn privstack_db::rusqlite::types::ToSql>> = Vec::new();
        for t in &type_params {
            param_values.push(Box::new(t.to_string()));
        }
        param_values.push(Box::new(limit_i64));
        let param_refs: Vec<&dyn privstack_db::rusqlite::types::ToSql> = param_values.iter().map(|b| b.as_ref()).collect();

        let rows: Vec<serde_json::Value> = stmt
            .query_map(param_refs.as_slice(), |row| {
                let emb_str: String = row.get::<_, String>(6)?;
                let embedding: Vec<f64> = parse_json_array(&emb_str);
                Ok(serde_json::json!({
                    "entity_id": row.get::<_, String>(0)?,
                    "entity_type": row.get::<_, String>(1)?,
                    "plugin_id": row.get::<_, String>(2)?,
                    "chunk_path": row.get::<_, String>(3)?,
                    "title": row.get::<_, String>(4)?,
                    "link_type": row.get::<_, String>(5)?,
                    "embedding": embedding,
                    "chunk_text": row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                }))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }
}

/// Parses a JSON array string like `[0.1, 0.2, 0.3]` into a `Vec<f64>`.
/// Returns empty vec on parse failure.
fn parse_json_array(s: &str) -> Vec<f64> {
    serde_json::from_str::<Vec<f64>>(s).unwrap_or_default()
}

fn format_bytes_log(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Scans a SQLite connection and returns diagnostics: all tables, indexes, db size.
pub fn scan_db_connection(conn: &privstack_db::rusqlite::Connection) -> serde_json::Value {
    let mut tables = Vec::new();

    if let Ok(table_names) = privstack_db::list_tables(conn) {
        for name in &table_names {
            let row_count: i64 = conn
                .prepare(&format!("SELECT COUNT(*) FROM \"{}\"", name))
                .and_then(|mut s| s.query_row([], |r| r.get(0)))
                .unwrap_or(0);

            // Get column count via PRAGMA table_info
            let column_count: i64 = conn
                .prepare(&format!("PRAGMA table_info('{}')", name))
                .and_then(|mut s| {
                    let cols: Vec<()> = s.query_map([], |_| Ok(()))?.filter_map(|r| r.ok()).collect();
                    Ok(cols.len() as i64)
                })
                .unwrap_or(0);

            tables.push(serde_json::json!({
                "table": name,
                "row_count": row_count,
                "column_count": column_count,
            }));
        }
    }

    // Database size via page_count * page_size
    let db_size = privstack_db::db_size(conn).unwrap_or(0);

    // Indexes: collect per table
    let mut indexes = Vec::new();
    if let Ok(table_names) = privstack_db::list_tables(conn) {
        for table_name in &table_names {
            if let Ok(mut stmt) = conn.prepare(&format!("PRAGMA index_list('{}')", table_name)) {
                let idx_rows: Vec<serde_json::Value> = stmt
                    .query_map([], |row| {
                        Ok(serde_json::json!({
                            "table": table_name,
                            "index": row.get::<_, String>(1).unwrap_or_default(),
                            "unique": row.get::<_, i32>(2).unwrap_or(0) == 1,
                        }))
                    })
                    .ok()
                    .map(|iter| iter.filter_map(|r| r.ok()).collect())
                    .unwrap_or_default();
                indexes.extend(idx_rows);
            }
        }
    }

    // Views
    let mut views = Vec::new();
    if let Ok(mut stmt) = conn.prepare("SELECT name FROM sqlite_master WHERE type='view' ORDER BY name") {
        views = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "view": row.get::<_, String>(0).unwrap_or_default(),
                }))
            })
            .ok()
            .map(|iter| iter.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();
    }

    serde_json::json!({
        "tables": tables,
        "db_size_bytes": db_size,
        "views": views,
        "indexes": indexes,
    })
}

/// Opens a SQLite file and returns diagnostics.
/// Returns None if the file doesn't exist or can't be opened.
pub fn scan_db_file(path: &std::path::Path) -> Option<serde_json::Value> {
    if !path.exists() {
        return None;
    }
    let file_size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);
    let conn = privstack_db::open_db_unencrypted(path).ok()?;

    let mut diag = scan_db_connection(&conn);
    if let Some(obj) = diag.as_object_mut() {
        obj.insert("file_size".to_string(), serde_json::json!(file_size));
    }
    Some(diag)
}

/// Compacts a standalone SQLite file using VACUUM INTO + swap.
/// Returns `(size_before, size_after)` or None on failure.
pub fn compact_db_file(path: &std::path::Path) -> Option<(u64, u64)> {
    if !path.exists() {
        return None;
    }
    let size_before = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    let conn = privstack_db::open_db_unencrypted(path).ok()?;
    privstack_db::checkpoint(&conn).ok()?;

    let result = privstack_db::compact(&conn, path);
    drop(conn);

    match result {
        Ok((_, size_after)) => {
            eprintln!(
                "[compact] {} : {} -> {} ({:.1}% reduction)",
                path.file_name().unwrap_or_default().to_string_lossy(),
                format_bytes_log(size_before),
                format_bytes_log(size_after),
                if size_before > 0 {
                    (1.0 - size_after as f64 / size_before as f64) * 100.0
                } else {
                    0.0
                }
            );
            Some((size_before, size_after))
        }
        Err(_) => None,
    }
}

// -- Field extraction helpers --

fn extract_field(
    data: &serde_json::Value,
    indexed_fields: &[IndexedField],
    target_type: FieldType,
    preferred_path: &str,
) -> Option<String> {
    if let Some(field) = indexed_fields.iter().find(|f| f.field_path == preferred_path && f.field_type == target_type) {
        if let Some(val) = data.pointer(&field.field_path) {
            return Some(val.as_str().unwrap_or(&val.to_string()).to_string());
        }
    }
    for field in indexed_fields {
        if field.field_type == target_type && field.field_path != preferred_path {
            if let Some(val) = data.pointer(&field.field_path) {
                return Some(val.as_str().unwrap_or(&val.to_string()).to_string());
            }
        }
    }
    None
}

fn extract_tags(data: &serde_json::Value, indexed_fields: &[IndexedField]) -> Vec<String> {
    let mut tags = Vec::new();
    for field in indexed_fields {
        if field.field_type == FieldType::Tag {
            if let Some(arr) = data.pointer(&field.field_path).and_then(|v| v.as_array()) {
                for item in arr {
                    if let Some(s) = item.as_str() {
                        tags.push(s.to_string());
                    }
                }
            }
        }
    }
    tags
}

/// Extracts Relation-typed fields and saves them as entity_links.
fn extract_relations(
    conn: &Connection,
    entity: &Entity,
    indexed_fields: &[IndexedField],
) -> StorageResult<()> {
    for field in indexed_fields {
        if field.field_type == FieldType::Relation {
            if let Some(val) = entity.data.pointer(&field.field_path) {
                if let Some(target_id) = val.as_str() {
                    conn.execute(
                        "INSERT OR IGNORE INTO entity_links (source_type, source_id, target_type, target_id) VALUES (?, ?, '_', ?)",
                        params![entity.entity_type, entity.id, target_id],
                    )?;
                } else if let (Some(target_type), Some(target_id)) = (
                    val.pointer("/type").and_then(|v| v.as_str()),
                    val.pointer("/id").and_then(|v| v.as_str()),
                ) {
                    conn.execute(
                        "INSERT OR IGNORE INTO entity_links (source_type, source_id, target_type, target_id) VALUES (?, ?, ?, ?)",
                        params![entity.entity_type, entity.id, target_type, target_id],
                    )?;
                }
            }
        }
    }
    Ok(())
}

/// Extracts Vector-typed fields and stores embeddings in entity_vectors.
fn extract_vectors(
    conn: &Connection,
    entity: &Entity,
    indexed_fields: &[IndexedField],
) -> StorageResult<()> {
    conn.execute(
        "DELETE FROM entity_vectors WHERE entity_id = ?",
        params![entity.id],
    )?;

    for field in indexed_fields {
        if field.field_type == FieldType::Vector {
            let dim = field.vector_dim.unwrap_or(0);
            if let Some(arr) = entity.data.pointer(&field.field_path).and_then(|v| v.as_array()) {
                if arr.len() != dim as usize {
                    continue;
                }
                let components: Vec<f64> = arr
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .collect();
                if components.len() != dim as usize {
                    continue;
                }
                let embedding_json = serde_json::to_string(&components).unwrap_or_default();
                conn.execute(
                    "INSERT INTO entity_vectors (entity_id, field_path, dim, embedding) VALUES (?, ?, ?, ?)",
                    params![entity.id, field.field_path, dim as i32, embedding_json],
                )?;
            }
        }
    }
    Ok(())
}

fn build_search_text(title: &Option<String>, body: &Option<String>, tags: &[String]) -> String {
    let mut text = String::new();
    if let Some(t) = title {
        text.push_str(t);
        text.push(' ');
    }
    if let Some(b) = body {
        text.push_str(b);
        text.push(' ');
    }
    for tag in tags {
        text.push_str(tag);
        text.push(' ');
    }
    text
}

/// Compare a JSON value against a filter value.
fn match_filter_value(actual: &serde_json::Value, expected: &serde_json::Value) -> bool {
    match (actual, expected) {
        (serde_json::Value::String(a), serde_json::Value::String(e)) => a == e,
        (serde_json::Value::Number(a), serde_json::Value::Number(e)) => a == e,
        (serde_json::Value::Bool(a), serde_json::Value::Bool(e)) => a == e,
        // Filter value is always a string from the FFI layer -- coerce actual to string
        (_, serde_json::Value::String(e)) => match actual {
            serde_json::Value::Number(n) => n.to_string() == *e,
            serde_json::Value::Bool(b) => b.to_string() == *e,
            _ => false,
        },
        _ => actual == expected,
    }
}

// -- Schema --

fn initialize_entity_schema(conn: &Connection) -> StorageResult<()> {
    // Migration: add local_only column to existing entities table BEFORE the main
    // batch, because CREATE TABLE IF NOT EXISTS won't add new columns to an existing
    // table, but the batch references local_only in a CREATE INDEX statement.
    if privstack_db::table_exists(conn, "entities").unwrap_or(false) {
        if !privstack_db::column_exists(conn, "entities", "local_only").unwrap_or(true) {
            privstack_db::add_column_if_not_exists(conn, "entities", "local_only", "INTEGER DEFAULT 0")?;
        }
    }

    // Migration: add chunk_text column to rag_vectors if missing
    if privstack_db::table_exists(conn, "rag_vectors").unwrap_or(false) {
        privstack_db::add_column_if_not_exists(conn, "rag_vectors", "chunk_text", "TEXT")
            .ok(); // ok() -- table might not exist yet on first run
    }

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS entities (
            id TEXT PRIMARY KEY,
            entity_type TEXT NOT NULL,
            data_json TEXT NOT NULL,
            title TEXT,
            body TEXT,
            tags TEXT,
            is_trashed INTEGER DEFAULT 0,
            is_favorite INTEGER DEFAULT 0,
            local_only INTEGER DEFAULT 0,
            created_at INTEGER NOT NULL,
            modified_at INTEGER NOT NULL,
            created_by TEXT NOT NULL,
            search_text TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);
        CREATE INDEX IF NOT EXISTS idx_entities_modified ON entities(modified_at DESC);
        CREATE INDEX IF NOT EXISTS idx_entities_trashed ON entities(is_trashed);
        CREATE INDEX IF NOT EXISTS idx_entities_favorite ON entities(is_favorite);
        CREATE INDEX IF NOT EXISTS idx_entities_local_only ON entities(local_only);

        CREATE TABLE IF NOT EXISTS entity_links (
            source_type TEXT NOT NULL,
            source_id TEXT NOT NULL,
            target_type TEXT NOT NULL,
            target_id TEXT NOT NULL,
            PRIMARY KEY (source_type, source_id, target_type, target_id)
        );

        CREATE TABLE IF NOT EXISTS entity_vectors (
            entity_id TEXT NOT NULL,
            field_path TEXT NOT NULL,
            dim INTEGER NOT NULL,
            embedding TEXT,
            PRIMARY KEY (entity_id, field_path)
        );

        -- Sync ledger: tracks which entities have been synced with which peer.
        CREATE TABLE IF NOT EXISTS sync_ledger (
            peer_id   TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            synced_at INTEGER NOT NULL,
            PRIMARY KEY (peer_id, entity_id)
        );

        -- Cloud sync cursor persistence
        CREATE TABLE IF NOT EXISTS cloud_sync_cursors (
            cursor_key TEXT PRIMARY KEY,
            cursor_value INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        -- Plugin fuel consumption history for metrics tracking
        CREATE TABLE IF NOT EXISTS plugin_fuel_history (
            plugin_id TEXT NOT NULL,
            fuel_consumed INTEGER NOT NULL,
            recorded_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_plugin_fuel_plugin_id ON plugin_fuel_history(plugin_id);
        CREATE INDEX IF NOT EXISTS idx_plugin_fuel_recorded_at ON plugin_fuel_history(plugin_id, recorded_at DESC);

        -- RAG vector index for semantic search across plugin content
        CREATE TABLE IF NOT EXISTS rag_vectors (
            entity_id TEXT NOT NULL,
            chunk_path TEXT NOT NULL,
            plugin_id TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            dim INTEGER NOT NULL,
            embedding TEXT,
            title TEXT,
            link_type TEXT,
            indexed_at INTEGER NOT NULL,
            chunk_text TEXT,
            PRIMARY KEY (entity_id, chunk_path)
        );
        CREATE INDEX IF NOT EXISTS idx_rag_vectors_type ON rag_vectors(entity_type);
        "#,
    )?;

    // Migration: drop plugin_fuel_history if it has the old schema with 'id' column
    let needs_migration = conn
        .execute(
            "INSERT INTO plugin_fuel_history (plugin_id, fuel_consumed, recorded_at) VALUES ('__migration_test__', 0, 0)",
            [],
        )
        .is_err();

    if needs_migration {
        let _ = conn.execute_batch(
            r#"
            DROP TABLE IF EXISTS plugin_fuel_history;
            CREATE TABLE plugin_fuel_history (
                plugin_id TEXT NOT NULL,
                fuel_consumed INTEGER NOT NULL,
                recorded_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_plugin_fuel_plugin_id ON plugin_fuel_history(plugin_id);
            CREATE INDEX IF NOT EXISTS idx_plugin_fuel_recorded_at ON plugin_fuel_history(plugin_id, recorded_at DESC);
            "#,
        );
    } else {
        let _ = conn.execute(
            "DELETE FROM plugin_fuel_history WHERE plugin_id = '__migration_test__'",
            [],
        );
    }

    Ok(())
}
