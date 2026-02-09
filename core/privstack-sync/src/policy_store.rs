//! Persistent storage for enterprise sync policy state (ACLs, teams, audit log).
//!
//! Uses a separate SQLite file so policy data is isolated from entity/event stores.

use crate::error::SyncError;
use crate::policy::{AuditDecision, AuditEntry, AuditAction, SyncRole};
use privstack_types::{EntityId, PeerId};
use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};

/// Persistent store for policy state backed by SQLite.
pub struct PolicyStore {
    conn: Arc<Mutex<Connection>>,
}

impl PolicyStore {
    /// Opens (or creates) a policy store at the given path.
    pub fn new(path: &str) -> Result<Self, SyncError> {
        let conn = Connection::open(path)
            .map_err(|e| SyncError::Storage(format!("failed to open policy store: {e}")))?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        Ok(store)
    }

    /// Opens an in-memory policy store (for testing).
    pub fn open_in_memory() -> Result<Self, SyncError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| SyncError::Storage(format!("failed to open in-memory policy store: {e}")))?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<(), SyncError> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                peer TEXT NOT NULL,
                entity TEXT,
                action TEXT NOT NULL,
                decision TEXT NOT NULL,
                detail TEXT NOT NULL,
                timestamp TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS acls (
                entity_id TEXT NOT NULL,
                peer_id TEXT NOT NULL,
                role TEXT NOT NULL,
                UNIQUE(entity_id, peer_id)
            );

            CREATE TABLE IF NOT EXISTS acl_defaults (
                entity_id TEXT PRIMARY KEY,
                default_role TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS acl_team_roles (
                entity_id TEXT NOT NULL,
                team_id TEXT NOT NULL,
                role TEXT NOT NULL,
                UNIQUE(entity_id, team_id)
            );

            CREATE TABLE IF NOT EXISTS teams (
                team_id TEXT NOT NULL,
                peer_id TEXT NOT NULL,
                UNIQUE(team_id, peer_id)
            );

            CREATE TABLE IF NOT EXISTS device_limits (
                peer_id TEXT PRIMARY KEY,
                max_devices INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS known_peers (
                peer_id TEXT PRIMARY KEY
            );

            CREATE TABLE IF NOT EXISTS active_devices (
                peer_id TEXT NOT NULL,
                device_id TEXT NOT NULL,
                UNIQUE(peer_id, device_id)
            );
            ",
        )
        .map_err(|e| SyncError::Storage(format!("failed to init policy schema: {e}")))?;
        Ok(())
    }

    // ── Audit log ────────────────────────────────────────────────

    /// Saves an audit entry to the database.
    pub fn save_audit_entry(&self, entry: &AuditEntry) -> Result<(), SyncError> {
        let conn = self.conn.lock().unwrap();
        let ts = entry
            .timestamp
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string();
        conn.execute(
            "INSERT INTO audit_log (peer, entity, action, decision, detail, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                entry.peer.to_string(),
                entry.entity.map(|e| e.to_string()),
                entry.action.to_string(),
                format!("{:?}", entry.decision),
                entry.detail,
                ts,
            ],
        )
        .map_err(|e| SyncError::Storage(format!("failed to save audit entry: {e}")))?;
        Ok(())
    }

    /// Loads audit log entries with pagination.
    pub fn load_audit_log(&self, limit: usize, offset: usize) -> Result<Vec<AuditEntry>, SyncError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT peer, entity, action, decision, detail, timestamp FROM audit_log ORDER BY id DESC LIMIT ?1 OFFSET ?2")
            .map_err(|e| SyncError::Storage(format!("failed to prepare audit query: {e}")))?;

        let entries = stmt
            .query_map(params![limit as i64, offset as i64], |row| {
                let peer_str: String = row.get(0)?;
                let entity_str: Option<String> = row.get(1)?;
                let action_str: String = row.get(2)?;
                let decision_str: String = row.get(3)?;
                let detail: String = row.get(4)?;
                let ts_str: String = row.get(5)?;

                Ok((peer_str, entity_str, action_str, decision_str, detail, ts_str))
            })
            .map_err(|e| SyncError::Storage(format!("failed to query audit log: {e}")))?;

        let mut result = Vec::new();
        for row in entries {
            let (peer_str, entity_str, action_str, decision_str, detail, ts_str) =
                row.map_err(|e| SyncError::Storage(format!("failed to read audit row: {e}")))?;

            let peer: PeerId = peer_str
                .parse()
                .map_err(|e| SyncError::Storage(format!("invalid peer_id in audit: {e}")))?;
            let entity: Option<EntityId> = match entity_str {
                Some(s) => Some(
                    s.parse()
                        .map_err(|e| SyncError::Storage(format!("invalid entity_id in audit: {e}")))?,
                ),
                None => None,
            };
            let action = parse_audit_action(&action_str);
            let decision = parse_audit_decision(&decision_str);
            let ts_millis: u128 = ts_str.parse().unwrap_or(0);
            let timestamp = std::time::UNIX_EPOCH + std::time::Duration::from_millis(ts_millis as u64);

            result.push(AuditEntry {
                peer,
                entity,
                action,
                decision,
                detail,
                timestamp,
            });
        }
        Ok(result)
    }

    /// Returns the total number of audit log entries.
    pub fn audit_log_count(&self) -> Result<usize, SyncError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
            .map_err(|e| SyncError::Storage(format!("failed to count audit log: {e}")))?;
        Ok(count as usize)
    }

    // ── ACL persistence ──────────────────────────────────────────

    /// Saves a peer-level ACL entry.
    pub fn save_acl(&self, entity_id: &EntityId, peer_id: &PeerId, role: SyncRole) -> Result<(), SyncError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO acls (entity_id, peer_id, role) VALUES (?1, ?2, ?3)",
            params![entity_id.to_string(), peer_id.to_string(), format!("{role}")],
        )
        .map_err(|e| SyncError::Storage(format!("failed to save acl: {e}")))?;
        Ok(())
    }

    /// Removes a peer-level ACL entry.
    pub fn remove_acl(&self, entity_id: &EntityId, peer_id: &PeerId) -> Result<(), SyncError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM acls WHERE entity_id = ?1 AND peer_id = ?2",
            params![entity_id.to_string(), peer_id.to_string()],
        )
        .map_err(|e| SyncError::Storage(format!("failed to remove acl: {e}")))?;
        Ok(())
    }

    /// Loads all peer-level ACLs. Returns (entity_id, peer_id, role) tuples.
    pub fn load_acls(&self) -> Result<Vec<(EntityId, PeerId, SyncRole)>, SyncError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT entity_id, peer_id, role FROM acls")
            .map_err(|e| SyncError::Storage(format!("failed to prepare acl query: {e}")))?;
        let rows = stmt
            .query_map([], |row| {
                let eid: String = row.get(0)?;
                let pid: String = row.get(1)?;
                let role: String = row.get(2)?;
                Ok((eid, pid, role))
            })
            .map_err(|e| SyncError::Storage(format!("failed to query acls: {e}")))?;

        let mut result = Vec::new();
        for row in rows {
            let (eid, pid, role_str) =
                row.map_err(|e| SyncError::Storage(format!("failed to read acl row: {e}")))?;
            let entity_id: EntityId = eid.parse().map_err(|e| SyncError::Storage(format!("{e}")))?;
            let peer_id: PeerId = pid.parse().map_err(|e| SyncError::Storage(format!("{e}")))?;
            let role = parse_sync_role(&role_str);
            result.push((entity_id, peer_id, role));
        }
        Ok(result)
    }

    /// Saves a default role for an entity.
    pub fn save_default_role(&self, entity_id: &EntityId, role: SyncRole) -> Result<(), SyncError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO acl_defaults (entity_id, default_role) VALUES (?1, ?2)",
            params![entity_id.to_string(), format!("{role}")],
        )
        .map_err(|e| SyncError::Storage(format!("failed to save default role: {e}")))?;
        Ok(())
    }

    /// Removes a default role for an entity.
    pub fn remove_default_role(&self, entity_id: &EntityId) -> Result<(), SyncError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM acl_defaults WHERE entity_id = ?1",
            params![entity_id.to_string()],
        )
        .map_err(|e| SyncError::Storage(format!("failed to remove default role: {e}")))?;
        Ok(())
    }

    /// Loads all default roles. Returns (entity_id, role) tuples.
    pub fn load_default_roles(&self) -> Result<Vec<(EntityId, SyncRole)>, SyncError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT entity_id, default_role FROM acl_defaults")
            .map_err(|e| SyncError::Storage(format!("{e}")))?;
        let rows = stmt
            .query_map([], |row| {
                let eid: String = row.get(0)?;
                let role: String = row.get(1)?;
                Ok((eid, role))
            })
            .map_err(|e| SyncError::Storage(format!("{e}")))?;

        let mut result = Vec::new();
        for row in rows {
            let (eid, role_str) = row.map_err(|e| SyncError::Storage(format!("{e}")))?;
            let entity_id: EntityId = eid.parse().map_err(|e| SyncError::Storage(format!("{e}")))?;
            result.push((entity_id, parse_sync_role(&role_str)));
        }
        Ok(result)
    }

    /// Saves a team-level ACL entry.
    pub fn save_team_role(
        &self,
        entity_id: &EntityId,
        team_id: &str,
        role: SyncRole,
    ) -> Result<(), SyncError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO acl_team_roles (entity_id, team_id, role) VALUES (?1, ?2, ?3)",
            params![entity_id.to_string(), team_id, format!("{role}")],
        )
        .map_err(|e| SyncError::Storage(format!("{e}")))?;
        Ok(())
    }

    /// Removes a team-level ACL entry.
    pub fn remove_team_role(&self, entity_id: &EntityId, team_id: &str) -> Result<(), SyncError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM acl_team_roles WHERE entity_id = ?1 AND team_id = ?2",
            params![entity_id.to_string(), team_id],
        )
        .map_err(|e| SyncError::Storage(format!("{e}")))?;
        Ok(())
    }

    /// Loads all team-level ACLs. Returns (entity_id, team_id, role) tuples.
    pub fn load_team_roles(&self) -> Result<Vec<(EntityId, String, SyncRole)>, SyncError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT entity_id, team_id, role FROM acl_team_roles")
            .map_err(|e| SyncError::Storage(format!("{e}")))?;
        let rows = stmt
            .query_map([], |row| {
                let eid: String = row.get(0)?;
                let tid: String = row.get(1)?;
                let role: String = row.get(2)?;
                Ok((eid, tid, role))
            })
            .map_err(|e| SyncError::Storage(format!("{e}")))?;

        let mut result = Vec::new();
        for row in rows {
            let (eid, tid, role_str) = row.map_err(|e| SyncError::Storage(format!("{e}")))?;
            let entity_id: EntityId = eid.parse().map_err(|e| SyncError::Storage(format!("{e}")))?;
            result.push((entity_id, tid, parse_sync_role(&role_str)));
        }
        Ok(result)
    }

    // ── Team membership ──────────────────────────────────────────

    /// Saves a team membership entry.
    pub fn save_team_member(&self, team_id: &str, peer_id: &PeerId) -> Result<(), SyncError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO teams (team_id, peer_id) VALUES (?1, ?2)",
            params![team_id, peer_id.to_string()],
        )
        .map_err(|e| SyncError::Storage(format!("{e}")))?;
        Ok(())
    }

    /// Removes a team membership entry.
    pub fn remove_team_member(&self, team_id: &str, peer_id: &PeerId) -> Result<(), SyncError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM teams WHERE team_id = ?1 AND peer_id = ?2",
            params![team_id, peer_id.to_string()],
        )
        .map_err(|e| SyncError::Storage(format!("{e}")))?;
        Ok(())
    }

    /// Loads all team memberships. Returns (team_id, peer_id) tuples.
    pub fn load_teams(&self) -> Result<Vec<(String, PeerId)>, SyncError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT team_id, peer_id FROM teams")
            .map_err(|e| SyncError::Storage(format!("{e}")))?;
        let rows = stmt
            .query_map([], |row| {
                let tid: String = row.get(0)?;
                let pid: String = row.get(1)?;
                Ok((tid, pid))
            })
            .map_err(|e| SyncError::Storage(format!("{e}")))?;

        let mut result = Vec::new();
        for row in rows {
            let (tid, pid_str) = row.map_err(|e| SyncError::Storage(format!("{e}")))?;
            let peer_id: PeerId = pid_str.parse().map_err(|e| SyncError::Storage(format!("{e}")))?;
            result.push((tid, peer_id));
        }
        Ok(result)
    }

    // ── Device limits ────────────────────────────────────────────

    /// Saves a device limit for a peer.
    pub fn save_device_limit(&self, peer_id: &PeerId, max_devices: usize) -> Result<(), SyncError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO device_limits (peer_id, max_devices) VALUES (?1, ?2)",
            params![peer_id.to_string(), max_devices as i64],
        )
        .map_err(|e| SyncError::Storage(format!("{e}")))?;
        Ok(())
    }

    /// Loads all device limits. Returns (peer_id, max_devices) tuples.
    pub fn load_device_limits(&self) -> Result<Vec<(PeerId, usize)>, SyncError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT peer_id, max_devices FROM device_limits")
            .map_err(|e| SyncError::Storage(format!("{e}")))?;
        let rows = stmt
            .query_map([], |row| {
                let pid: String = row.get(0)?;
                let max: i64 = row.get(1)?;
                Ok((pid, max))
            })
            .map_err(|e| SyncError::Storage(format!("{e}")))?;

        let mut result = Vec::new();
        for row in rows {
            let (pid_str, max) = row.map_err(|e| SyncError::Storage(format!("{e}")))?;
            let peer_id: PeerId = pid_str.parse().map_err(|e| SyncError::Storage(format!("{e}")))?;
            result.push((peer_id, max as usize));
        }
        Ok(result)
    }

    // ── Known peers ──────────────────────────────────────────────

    /// Saves a known peer.
    pub fn save_known_peer(&self, peer_id: &PeerId) -> Result<(), SyncError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO known_peers (peer_id) VALUES (?1)",
            params![peer_id.to_string()],
        )
        .map_err(|e| SyncError::Storage(format!("{e}")))?;
        Ok(())
    }

    /// Removes a known peer.
    pub fn remove_known_peer(&self, peer_id: &PeerId) -> Result<(), SyncError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM known_peers WHERE peer_id = ?1",
            params![peer_id.to_string()],
        )
        .map_err(|e| SyncError::Storage(format!("{e}")))?;
        Ok(())
    }

    /// Loads all known peers.
    pub fn load_known_peers(&self) -> Result<Vec<PeerId>, SyncError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT peer_id FROM known_peers")
            .map_err(|e| SyncError::Storage(format!("{e}")))?;
        let rows = stmt
            .query_map([], |row| {
                let pid: String = row.get(0)?;
                Ok(pid)
            })
            .map_err(|e| SyncError::Storage(format!("{e}")))?;

        let mut result = Vec::new();
        for row in rows {
            let pid_str = row.map_err(|e| SyncError::Storage(format!("{e}")))?;
            let peer_id: PeerId = pid_str.parse().map_err(|e| SyncError::Storage(format!("{e}")))?;
            result.push(peer_id);
        }
        Ok(result)
    }

    // ── Active devices ───────────────────────────────────────────

    /// Saves active devices for a peer.
    pub fn save_active_devices(&self, peer_id: &PeerId, device_ids: &[String]) -> Result<(), SyncError> {
        let conn = self.conn.lock().unwrap();
        // Clear existing then insert
        conn.execute(
            "DELETE FROM active_devices WHERE peer_id = ?1",
            params![peer_id.to_string()],
        )
        .map_err(|e| SyncError::Storage(format!("{e}")))?;

        for did in device_ids {
            conn.execute(
                "INSERT INTO active_devices (peer_id, device_id) VALUES (?1, ?2)",
                params![peer_id.to_string(), did],
            )
            .map_err(|e| SyncError::Storage(format!("{e}")))?;
        }
        Ok(())
    }

    /// Loads active devices for all peers.
    pub fn load_active_devices(&self) -> Result<Vec<(PeerId, String)>, SyncError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT peer_id, device_id FROM active_devices")
            .map_err(|e| SyncError::Storage(format!("{e}")))?;
        let rows = stmt
            .query_map([], |row| {
                let pid: String = row.get(0)?;
                let did: String = row.get(1)?;
                Ok((pid, did))
            })
            .map_err(|e| SyncError::Storage(format!("{e}")))?;

        let mut result = Vec::new();
        for row in rows {
            let (pid_str, did) = row.map_err(|e| SyncError::Storage(format!("{e}")))?;
            let peer_id: PeerId = pid_str.parse().map_err(|e| SyncError::Storage(format!("{e}")))?;
            result.push((peer_id, did));
        }
        Ok(result)
    }
}

fn parse_audit_action(s: &str) -> AuditAction {
    match s {
        "handshake" => AuditAction::Handshake,
        "sync_request" => AuditAction::SyncRequest,
        "event_send" => AuditAction::EventSend,
        "event_receive" => AuditAction::EventReceive,
        "device_register" => AuditAction::DeviceRegister,
        _ => AuditAction::Handshake, // fallback
    }
}

fn parse_audit_decision(s: &str) -> AuditDecision {
    match s {
        "Allowed" => AuditDecision::Allowed,
        "Denied" => AuditDecision::Denied,
        "Filtered" => AuditDecision::Filtered,
        _ => AuditDecision::Denied, // fallback
    }
}

fn parse_sync_role(s: &str) -> SyncRole {
    match s {
        "Viewer" => SyncRole::Viewer,
        "Editor" => SyncRole::Editor,
        "Admin" => SyncRole::Admin,
        "Owner" => SyncRole::Owner,
        _ => SyncRole::Viewer, // fallback
    }
}
