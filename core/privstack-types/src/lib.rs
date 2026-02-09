//! Core type definitions for PrivStack.
//!
//! This crate defines the fundamental, plugin-agnostic types used throughout
//! the core engine:
//! - Entity and Peer identifiers (UUID v7)
//! - Hybrid Logical Clock timestamps
//! - Generic sync events (entity-level operations)
//!
//! All domain-specific types (documents, blocks, rich text, task models, etc.)
//! belong in their respective plugins, not here.

mod event;
mod ids;
mod timestamp;

pub use event::{Event, EventId, EventPayload};
pub use ids::{EntityId, PeerId};
pub use timestamp::HybridTimestamp;

/// Result type alias using the crate's error type.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in type operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("invalid UUID: {0}")]
    InvalidUuid(#[from] uuid::Error),

    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(String),
}
