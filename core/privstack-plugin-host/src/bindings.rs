//! Wasmtime Component Model bindings generated from the WIT definitions.
//!
//! This file is excluded from coverage reports as it contains auto-generated
//! code from wasmtime::component::bindgen! macro.
//!
//! Uses `wasmtime::component::bindgen!` to produce:
//! - Host import traits that we implement (sdk, settings, logger, etc.)
//! - Guest export callable interfaces (plugin, linkable-item-provider, etc.)

use wasmtime::component::bindgen;

bindgen!({
    path: "wit",
    world: "plugin-world",
    // Fully synchronous — network::fetch_url blocks on the host side.
    async: false,
    // Trap on missing optional exports instead of panicking.
    trappable_imports: true,
});
