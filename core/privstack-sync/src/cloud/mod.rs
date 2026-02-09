//! Cloud storage transports for sync.
//!
//! Provides file-based sync using cloud storage providers like
//! Google Drive and iCloud as the transport layer.

pub mod google_drive;
pub mod icloud;
pub mod storage;

pub use google_drive::{GoogleDriveConfig, GoogleDriveStorage};
pub use icloud::{ICloudConfig, ICloudStorage};
pub use storage::{CloudFile, CloudStorage, CloudStorageConfig};
