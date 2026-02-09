//! Event types for entity-level sync.
//!
//! Events represent changes to entities that are synced between peers.
//! Each event is immutable and contains all information needed to apply
//! the change on any replica.
//!
//! The core only understands entity-level operations (create, update, delete,
//! snapshot). Plugins that need finer-grained operations (e.g., block-level
//! CRDT for a rich-text editor) handle those internally via their own
//! Rust dylib and `PluginDomainHandler`.

use crate::{EntityId, HybridTimestamp, PeerId};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Unique identifier for an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(Uuid);

impl EventId {
    /// Creates a new event ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for EventId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// The payload of an event, containing the actual operation.
///
/// All operations are entity-level. The core has no knowledge of what
/// the JSON data contains — that is entirely plugin-defined.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", content = "data")]
pub enum EventPayload {
    /// An entity was created.
    EntityCreated {
        /// The plugin-defined entity type (e.g., "note", "task").
        entity_type: String,
        /// Full JSON representation of the entity.
        json_data: String,
    },

    /// An entity was updated.
    EntityUpdated {
        /// The plugin-defined entity type.
        entity_type: String,
        /// Full JSON representation of the updated entity.
        json_data: String,
    },

    /// An entity was deleted.
    EntityDeleted {
        /// The plugin-defined entity type.
        entity_type: String,
    },

    /// Full entity snapshot for sync.
    /// Carries the complete serialized state of an entity so peers can
    /// store it directly using the registered merge strategy.
    FullSnapshot {
        /// The plugin-defined entity type.
        entity_type: String,
        /// Full JSON representation of the entity.
        json_data: String,
    },

    // ── ACL propagation events ──────────────────────────────────

    /// Grant a peer a role on an entity.
    AclGrantPeer {
        entity_id: String,
        peer_id: String,
        role: String,
    },

    /// Revoke a peer's role on an entity.
    AclRevokePeer {
        entity_id: String,
        peer_id: String,
    },

    /// Grant a team a role on an entity.
    AclGrantTeam {
        entity_id: String,
        team_id: String,
        role: String,
    },

    /// Revoke a team's role on an entity.
    AclRevokeTeam {
        entity_id: String,
        team_id: String,
    },

    /// Set (or clear) the default role for an entity.
    AclSetDefault {
        entity_id: String,
        /// Role name, or empty/absent to clear.
        role: Option<String>,
    },

    /// Add a peer to a team.
    TeamAddPeer {
        team_id: String,
        peer_id: String,
    },

    /// Remove a peer from a team.
    TeamRemovePeer {
        team_id: String,
        peer_id: String,
    },
}

/// An event representing a change to an entity.
///
/// Events are the unit of replication in the sync system.
/// They are immutable and totally ordered by their timestamp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Unique identifier for this event.
    pub id: EventId,

    /// The entity this event applies to.
    pub entity_id: EntityId,

    /// The peer that created this event.
    pub peer_id: PeerId,

    /// When this event was created.
    pub timestamp: HybridTimestamp,

    /// The operation to perform.
    pub payload: EventPayload,

    /// Dependencies: events that must be applied before this one.
    /// Typically the previous event from the same peer.
    #[serde(default)]
    pub dependencies: Vec<EventId>,
}

impl Event {
    /// Creates a new event.
    #[must_use]
    pub fn new(
        entity_id: EntityId,
        peer_id: PeerId,
        timestamp: HybridTimestamp,
        payload: EventPayload,
    ) -> Self {
        Self {
            id: EventId::new(),
            entity_id,
            peer_id,
            timestamp,
            payload,
            dependencies: Vec::new(),
        }
    }

    /// Creates an entity-created event.
    #[must_use]
    pub fn entity_created(
        entity_id: EntityId,
        peer_id: PeerId,
        entity_type: impl Into<String>,
        json_data: impl Into<String>,
    ) -> Self {
        Self::new(
            entity_id,
            peer_id,
            HybridTimestamp::now(),
            EventPayload::EntityCreated {
                entity_type: entity_type.into(),
                json_data: json_data.into(),
            },
        )
    }

    /// Creates an entity-updated event.
    #[must_use]
    pub fn entity_updated(
        entity_id: EntityId,
        peer_id: PeerId,
        entity_type: impl Into<String>,
        json_data: impl Into<String>,
    ) -> Self {
        Self::new(
            entity_id,
            peer_id,
            HybridTimestamp::now(),
            EventPayload::EntityUpdated {
                entity_type: entity_type.into(),
                json_data: json_data.into(),
            },
        )
    }

    /// Creates an entity-deleted event.
    #[must_use]
    pub fn entity_deleted(
        entity_id: EntityId,
        peer_id: PeerId,
        entity_type: impl Into<String>,
    ) -> Self {
        Self::new(
            entity_id,
            peer_id,
            HybridTimestamp::now(),
            EventPayload::EntityDeleted {
                entity_type: entity_type.into(),
            },
        )
    }

    /// Creates a full snapshot event for sync.
    #[must_use]
    pub fn full_snapshot(
        entity_id: EntityId,
        peer_id: PeerId,
        entity_type: impl Into<String>,
        json_data: impl Into<String>,
    ) -> Self {
        Self::new(
            entity_id,
            peer_id,
            HybridTimestamp::now(),
            EventPayload::FullSnapshot {
                entity_type: entity_type.into(),
                json_data: json_data.into(),
            },
        )
    }

    /// Adds a dependency to this event.
    pub fn with_dependency(mut self, dep: EventId) -> Self {
        self.dependencies.push(dep);
        self
    }
}
