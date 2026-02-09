//! ACL event handler — applies ACL-as-CRDT events to the enterprise policy.

use crate::error::SyncError;
use crate::policy::{EnterpriseSyncPolicy, SyncRole, TeamId};
use async_trait::async_trait;
use privstack_types::{EntityId, Event, EventPayload, PeerId};
use std::sync::Arc;

/// Trait for handling ACL events that flow through the sync pipeline.
#[async_trait]
pub trait AclEventHandler: Send + Sync {
    /// Attempts to handle an event as an ACL event.
    /// Returns `Ok(true)` if handled (callers should skip normal applicator),
    /// `Ok(false)` if not an ACL event, or `Err` on failure.
    async fn handle_acl_event(&self, event: &Event) -> Result<bool, SyncError>;
}

/// Applies ACL events to an `EnterpriseSyncPolicy`.
pub struct AclApplicator {
    policy: Arc<EnterpriseSyncPolicy>,
}

impl AclApplicator {
    pub fn new(policy: Arc<EnterpriseSyncPolicy>) -> Self {
        Self { policy }
    }
}

#[async_trait]
impl AclEventHandler for AclApplicator {
    async fn handle_acl_event(&self, event: &Event) -> Result<bool, SyncError> {
        match &event.payload {
            EventPayload::AclGrantPeer {
                entity_id,
                peer_id,
                role,
            } => {
                let eid: EntityId = entity_id.parse().map_err(|e| SyncError::Protocol(format!("{e}")))?;
                let pid: PeerId = peer_id.parse().map_err(|e| SyncError::Protocol(format!("{e}")))?;
                let r = parse_role(role)?;
                self.policy.grant_peer_role(eid, pid, r).await;
                Ok(true)
            }
            EventPayload::AclRevokePeer {
                entity_id,
                peer_id,
            } => {
                let eid: EntityId = entity_id.parse().map_err(|e| SyncError::Protocol(format!("{e}")))?;
                let pid: PeerId = peer_id.parse().map_err(|e| SyncError::Protocol(format!("{e}")))?;
                self.policy.revoke_peer_role(eid, pid).await;
                Ok(true)
            }
            EventPayload::AclGrantTeam {
                entity_id,
                team_id,
                role,
            } => {
                let eid: EntityId = entity_id.parse().map_err(|e| SyncError::Protocol(format!("{e}")))?;
                let tid = parse_team_id(team_id)?;
                let r = parse_role(role)?;
                self.policy.grant_team_role(eid, tid, r).await;
                Ok(true)
            }
            EventPayload::AclRevokeTeam {
                entity_id,
                team_id,
            } => {
                let eid: EntityId = entity_id.parse().map_err(|e| SyncError::Protocol(format!("{e}")))?;
                let tid = parse_team_id(team_id)?;
                self.policy.revoke_team_role(eid, tid).await;
                Ok(true)
            }
            EventPayload::AclSetDefault { entity_id, role } => {
                let eid: EntityId = entity_id.parse().map_err(|e| SyncError::Protocol(format!("{e}")))?;
                let r = match role {
                    Some(s) if !s.is_empty() => Some(parse_role(s)?),
                    _ => None,
                };
                self.policy.set_default_role(eid, r).await;
                Ok(true)
            }
            EventPayload::TeamAddPeer { team_id, peer_id } => {
                let tid = parse_team_id(team_id)?;
                let pid: PeerId = peer_id.parse().map_err(|e| SyncError::Protocol(format!("{e}")))?;
                self.policy.add_team_member(tid, pid).await;
                Ok(true)
            }
            EventPayload::TeamRemovePeer { team_id, peer_id } => {
                let tid = parse_team_id(team_id)?;
                let pid: PeerId = peer_id.parse().map_err(|e| SyncError::Protocol(format!("{e}")))?;
                self.policy.remove_team_member(tid, pid).await;
                Ok(true)
            }
            // Not an ACL event
            _ => Ok(false),
        }
    }
}

/// Returns true if the given payload is an ACL-related event.
pub fn is_acl_event(payload: &EventPayload) -> bool {
    matches!(
        payload,
        EventPayload::AclGrantPeer { .. }
            | EventPayload::AclRevokePeer { .. }
            | EventPayload::AclGrantTeam { .. }
            | EventPayload::AclRevokeTeam { .. }
            | EventPayload::AclSetDefault { .. }
            | EventPayload::TeamAddPeer { .. }
            | EventPayload::TeamRemovePeer { .. }
    )
}

fn parse_role(s: &str) -> Result<SyncRole, SyncError> {
    match s {
        "Viewer" => Ok(SyncRole::Viewer),
        "Editor" => Ok(SyncRole::Editor),
        "Admin" => Ok(SyncRole::Admin),
        "Owner" => Ok(SyncRole::Owner),
        _ => Err(SyncError::Protocol(format!("unknown role: {s}"))),
    }
}

fn parse_team_id(s: &str) -> Result<TeamId, SyncError> {
    let uuid = uuid::Uuid::parse_str(s)
        .map_err(|e| SyncError::Protocol(format!("invalid team_id: {e}")))?;
    Ok(TeamId(uuid))
}
