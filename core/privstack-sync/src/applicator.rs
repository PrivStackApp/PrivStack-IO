//! Event applicator - applies sync events to the entity store.
//!
//! Handles entity-level operations: create, update, delete, and full snapshots.
//! Plugin-specific domain logic (e.g., block-level CRDT merge for a rich-text
//! editor) is delegated to the plugin's `PluginDomainHandler`.

use privstack_model::{Entity, EntitySchema, MergeStrategy, PluginDomainHandler};
use privstack_storage::EntityStore;
use privstack_types::{EntityId, Event, EventPayload, PeerId};
use tracing::{debug, warn};

/// Result type for applicator operations.
pub type ApplicatorResult<T> = Result<T, ApplicatorError>;

/// Errors that can occur during event application.
#[derive(Debug, thiserror::Error)]
pub enum ApplicatorError {
    #[error("Storage error: {0}")]
    Storage(#[from] privstack_storage::StorageError),

    #[error("Entity not found: {0}")]
    EntityNotFound(String),

    #[error("Schema not found for entity type: {0}")]
    SchemaNotFound(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),
}

/// Applies sync events to the entity store using schema-driven merge.
pub struct EventApplicator {
    /// Local peer ID for creating events.
    #[allow(dead_code)]
    local_peer_id: PeerId,
}

impl EventApplicator {
    /// Creates a new event applicator.
    pub fn new(local_peer_id: PeerId) -> Self {
        Self { local_peer_id }
    }

    /// Applies a single event to the entity store.
    /// Returns true if the store was modified.
    pub fn apply_event(
        &self,
        event: &Event,
        store: &EntityStore,
        schema: Option<&EntitySchema>,
        handler: Option<&dyn PluginDomainHandler>,
    ) -> ApplicatorResult<bool> {
        debug!("Applying event {:?} to entity {}", event.payload, event.entity_id);

        match &event.payload {
            EventPayload::EntityCreated { entity_type, json_data } => {
                self.apply_entity_created(event, entity_type, json_data, store, schema)
            }
            EventPayload::EntityUpdated { entity_type, json_data } => {
                self.apply_entity_updated(event, entity_type, json_data, store, schema, handler)
            }
            EventPayload::EntityDeleted { entity_type } => {
                self.apply_entity_deleted(event, entity_type, store)
            }
            EventPayload::FullSnapshot { entity_type, json_data } => {
                self.apply_full_snapshot(event, entity_type, json_data, store, schema, handler)
            }
            // ACL events are handled by AclApplicator, not the entity applicator
            _ => {
                debug!("Skipping non-entity event payload: {:?}", event.payload);
                Ok(false)
            }
        }
    }

    fn apply_entity_created(
        &self,
        event: &Event,
        entity_type: &str,
        json_data: &str,
        store: &EntityStore,
        schema: Option<&EntitySchema>,
    ) -> ApplicatorResult<bool> {
        let data: serde_json::Value = serde_json::from_str(json_data)?;
        let entity = Entity {
            id: event.entity_id.to_string(),
            entity_type: entity_type.to_string(),
            data,
            created_at: event.timestamp.wall_time() as i64,
            modified_at: event.timestamp.wall_time() as i64,
            created_by: event.peer_id.to_string(),
        };

        if let Some(s) = schema {
            store.save_entity(&entity, s)?;
        } else {
            store.save_entity_raw(&entity)?;
        }

        debug!("Created entity {} (type={})", event.entity_id, entity_type);
        Ok(true)
    }

    fn apply_entity_updated(
        &self,
        event: &Event,
        entity_type: &str,
        json_data: &str,
        store: &EntityStore,
        schema: Option<&EntitySchema>,
        handler: Option<&dyn PluginDomainHandler>,
    ) -> ApplicatorResult<bool> {
        let remote_data: serde_json::Value = serde_json::from_str(json_data)?;
        let remote_entity = Entity {
            id: event.entity_id.to_string(),
            entity_type: entity_type.to_string(),
            data: remote_data,
            created_at: event.timestamp.wall_time() as i64,
            modified_at: event.timestamp.wall_time() as i64,
            created_by: event.peer_id.to_string(),
        };

        // Check if entity exists locally for merge
        let existing = store.get_entity(&event.entity_id.to_string())?;
        let merged = match existing {
            Some(local) => self.merge_entities(&local, &remote_entity, schema, handler),
            None => remote_entity,
        };

        if let Some(s) = schema {
            store.save_entity(&merged, s)?;
        } else {
            store.save_entity_raw(&merged)?;
        }

        debug!("Updated entity {} (type={})", event.entity_id, entity_type);
        Ok(true)
    }

    fn apply_entity_deleted(
        &self,
        event: &Event,
        entity_type: &str,
        store: &EntityStore,
    ) -> ApplicatorResult<bool> {
        store.delete_entity(&event.entity_id.to_string())?;
        debug!("Deleted entity {} (type={})", event.entity_id, entity_type);
        Ok(true)
    }

    fn apply_full_snapshot(
        &self,
        event: &Event,
        entity_type: &str,
        json_data: &str,
        store: &EntityStore,
        schema: Option<&EntitySchema>,
        handler: Option<&dyn PluginDomainHandler>,
    ) -> ApplicatorResult<bool> {
        // FullSnapshot is treated the same as EntityUpdated — merge with local
        self.apply_entity_updated(event, entity_type, json_data, store, schema, handler)
    }

    /// Merges a local and remote entity based on the schema's merge strategy.
    pub fn merge_entities(
        &self,
        local: &Entity,
        remote: &Entity,
        schema: Option<&EntitySchema>,
        handler: Option<&dyn PluginDomainHandler>,
    ) -> Entity {
        let strategy = schema.map(|s| s.merge_strategy).unwrap_or(MergeStrategy::LwwDocument);

        match strategy {
            MergeStrategy::LwwDocument => {
                // Last-writer-wins on whole document
                if remote.modified_at >= local.modified_at {
                    remote.clone()
                } else {
                    local.clone()
                }
            }
            MergeStrategy::LwwPerField => {
                // Last-writer-wins per top-level field
                if remote.modified_at >= local.modified_at {
                    // Remote is newer overall — use it but preserve any local-only fields
                    let mut merged_data = local.data.clone();
                    if let (Some(local_obj), Some(remote_obj)) =
                        (merged_data.as_object_mut(), remote.data.as_object())
                    {
                        for (key, value) in remote_obj {
                            local_obj.insert(key.clone(), value.clone());
                        }
                    }
                    Entity {
                        data: merged_data,
                        modified_at: remote.modified_at,
                        ..local.clone()
                    }
                } else {
                    local.clone()
                }
            }
            MergeStrategy::Custom => {
                // Delegate to plugin handler
                if let Some(h) = handler {
                    h.merge(local, remote)
                } else {
                    warn!("Custom merge strategy but no handler — falling back to LWW");
                    if remote.modified_at >= local.modified_at {
                        remote.clone()
                    } else {
                        local.clone()
                    }
                }
            }
        }
    }
}

/// Creates a sync event for an entity operation.
pub fn create_event(
    entity_id: EntityId,
    peer_id: PeerId,
    entity_type: &str,
    json_data: &str,
) -> Event {
    Event::full_snapshot(entity_id, peer_id, entity_type, json_data)
}
