//! Wasmtime-based plugin host for PrivStack.
//!
//! Loads Wasm Component Model plugins, enforces capability-based permissions,
//! and routes host function calls to the PrivStack core engine.
//!
//! Each plugin runs in its own `wasmtime::Store` with memory isolation,
//! CPU fuel budgets, and scoped entity-type access.

pub mod bindings;
mod error;
mod host_impl;
mod manager;
mod permissions;
mod policy;
mod sandbox;
mod wit_types;

pub use error::PluginHostError;
pub use manager::PluginHostManager;
pub use permissions::{Permission, PermissionSet, PermissionTier};
pub use policy::{PolicyConfig, PolicyEngine, PolicyMode};
pub use sandbox::{PluginResourceMetrics, PluginSandbox, ResourceLimits};
pub use wit_types::*;
