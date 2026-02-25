//! PrivStack Plugin Package (.ppk) format.
//!
//! A `.ppk` file is a zip archive containing:
//! - `manifest.toml` — plugin metadata, version, permissions, schemas
//! - `plugin.wasm`   — compiled Wasm Component Model module
//! - `icon.png`      — optional plugin icon (256x256 recommended)
//! - `README.md`     — optional plugin documentation
//! - `views/`        — optional declarative UI definitions (JSON)
//! - `signature.bin` — Ed25519 detached signature over the content hash
//!
//! Signing: the content hash covers all files except `signature.bin`.
//! First-party plugins are signed with the PrivStack key.
//! Third-party plugins are signed with the developer's key.

mod error;
mod manifest;
mod package;
mod signing;

pub use error::PpkError;
pub use manifest::{PpkManifest, PpkPermission, PpkEntitySchema, PpkIndexedField};
pub use package::{PpkPackage, PackageBuilder, PackageEntry};
pub use signing::{SigningKey, VerifyingKey, Signature, KeyPair};
