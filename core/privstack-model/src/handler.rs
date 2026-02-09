use crate::Entity;

/// Optional trait for plugins that need custom validation, post-load
/// processing, or merge logic beyond the standard LWW strategies.
///
/// Most plugins do NOT need to implement this — the generic CRUD engine
/// handles everything automatically based on `EntitySchema`.
///
/// Only implement this if you need:
/// - Input validation (e.g., password strength checks)
/// - Post-load data enrichment (e.g., computing derived fields)
/// - Custom CRDT merge (e.g., budget reconciliation)
pub trait PluginDomainHandler: Send + Sync {
    /// Validate an entity before it is persisted.
    /// Return `Err(message)` to reject the write.
    fn validate(&self, entity: &Entity) -> Result<(), String> {
        let _ = entity;
        Ok(())
    }

    /// Called after loading an entity from storage, before returning to the caller.
    /// Use this for computing derived/transient fields.
    fn on_after_load(&self, entity: &mut Entity) {
        let _ = entity;
    }

    /// Custom merge for `MergeStrategy::Custom`.
    /// Default implementation is last-writer-wins by `modified_at`.
    fn merge(&self, local: &Entity, remote: &Entity) -> Entity {
        if remote.modified_at >= local.modified_at {
            remote.clone()
        } else {
            local.clone()
        }
    }
}
