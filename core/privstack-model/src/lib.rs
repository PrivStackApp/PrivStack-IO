//! Core entity model for PrivStack.
//!
//! Defines the universal types that all PrivStack subsystems depend on:
//! - [`Entity`] — the generic data container (id, type, JSON payload, timestamps)
//! - [`EntitySchema`] — declares an entity type's indexed fields and merge strategy
//! - [`MergeStrategy`] — how conflicts are resolved during sync (LWW, per-field, custom)
//! - [`PluginDomainHandler`] — optional trait for custom validation/merge logic
//!
//! These types are consumed by storage, sync, FFI, and (indirectly via JSON)
//! the C# plugin SDK. They form the contract between plugins and the core engine.

mod entity;
mod handler;
mod schema;

pub use entity::Entity;
pub use handler::PluginDomainHandler;
pub use schema::{EntitySchema, FieldType, IndexedField, MergeStrategy};
