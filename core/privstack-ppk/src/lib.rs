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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn roundtrip_pack_unpack() {
        let manifest = PpkManifest {
            id: "privstack.test".into(),
            name: "Test Plugin".into(),
            description: "A test plugin".into(),
            version: "1.0.0".into(),
            author: "PrivStack".into(),
            icon: Some("TestIcon".into()),
            navigation_order: 500,
            category: "utility".into(),
            can_disable: true,
            is_experimental: false,
            min_app_version: None,
            permissions: vec![PpkPermission::EntityCrud, PpkPermission::ViewState],
            schemas: vec![],
        };

        let wasm_bytes = b"fake wasm module content";
        let readme = b"# Test Plugin\n\nA test.";

        let ppk_bytes = PackageBuilder::new(manifest.clone())
            .wasm(wasm_bytes.to_vec())
            .readme(readme.to_vec())
            .build()
            .expect("pack should succeed");

        assert!(!ppk_bytes.is_empty());

        let pkg = PpkPackage::open(Cursor::new(&ppk_bytes)).expect("unpack should succeed");
        assert_eq!(pkg.manifest.id, "privstack.test");
        assert_eq!(pkg.manifest.name, "Test Plugin");
        assert_eq!(pkg.wasm.as_deref(), Some(wasm_bytes.as_slice()));
        assert_eq!(pkg.readme.as_deref(), Some(readme.as_slice()));
        assert!(pkg.icon.is_none());
        assert!(pkg.signature.is_none());
    }

    #[test]
    fn sign_and_verify() {
        let manifest = PpkManifest {
            id: "privstack.signed".into(),
            name: "Signed Plugin".into(),
            description: "A signed plugin".into(),
            version: "1.0.0".into(),
            author: "PrivStack".into(),
            icon: None,
            navigation_order: 100,
            category: "productivity".into(),
            can_disable: true,
            is_experimental: false,
            min_app_version: None,
            permissions: vec![PpkPermission::EntityCrud],
            schemas: vec![],
        };

        let wasm_bytes = b"signed wasm content";
        let keypair = KeyPair::generate();

        let ppk_bytes = PackageBuilder::new(manifest)
            .wasm(wasm_bytes.to_vec())
            .sign(&keypair.signing_key)
            .build()
            .expect("pack+sign should succeed");

        let pkg = PpkPackage::open(Cursor::new(&ppk_bytes)).expect("unpack should succeed");
        assert!(pkg.signature.is_some());

        let verified = pkg.verify(&keypair.verifying_key);
        assert!(verified.is_ok());
    }

    #[test]
    fn verify_fails_with_wrong_key() {
        let manifest = PpkManifest {
            id: "privstack.tampered".into(),
            name: "Tampered".into(),
            description: "".into(),
            version: "1.0.0".into(),
            author: "PrivStack".into(),
            icon: None,
            navigation_order: 100,
            category: "utility".into(),
            can_disable: true,
            is_experimental: false,
            min_app_version: None,
            permissions: vec![],
            schemas: vec![],
        };

        let wasm_bytes = b"tampered wasm";
        let real_keypair = KeyPair::generate();
        let wrong_keypair = KeyPair::generate();

        let ppk_bytes = PackageBuilder::new(manifest)
            .wasm(wasm_bytes.to_vec())
            .sign(&real_keypair.signing_key)
            .build()
            .expect("pack+sign should succeed");

        let pkg = PpkPackage::open(Cursor::new(&ppk_bytes)).expect("unpack should succeed");
        let result = pkg.verify(&wrong_keypair.verifying_key);
        assert!(result.is_err());
    }

    #[test]
    fn unsigned_package_verify_fails() {
        let manifest = PpkManifest {
            id: "privstack.unsigned".into(),
            name: "Unsigned".into(),
            description: "".into(),
            version: "1.0.0".into(),
            author: "PrivStack".into(),
            icon: None,
            navigation_order: 100,
            category: "utility".into(),
            can_disable: true,
            is_experimental: false,
            min_app_version: None,
            permissions: vec![],
            schemas: vec![],
        };

        let ppk_bytes = PackageBuilder::new(manifest)
            .wasm(b"content".to_vec())
            .build()
            .expect("pack should succeed");

        let pkg = PpkPackage::open(Cursor::new(&ppk_bytes)).expect("unpack should succeed");
        let keypair = KeyPair::generate();
        let result = pkg.verify(&keypair.verifying_key);
        assert!(result.is_err());
    }

    #[test]
    fn manifest_toml_roundtrip() {
        let manifest = PpkManifest {
            id: "privstack.rss".into(),
            name: "RSS Reader".into(),
            description: "RSS/Atom feed reader".into(),
            version: "1.0.0".into(),
            author: "PrivStack".into(),
            icon: Some("Rss".into()),
            navigation_order: 350,
            category: "utility".into(),
            can_disable: true,
            is_experimental: false,
            min_app_version: Some("0.1.0".into()),
            permissions: vec![
                PpkPermission::EntityCrud,
                PpkPermission::EntityQuery,
                PpkPermission::ViewState,
                PpkPermission::CommandPalette,
            ],
            schemas: vec![
                PpkEntitySchema {
                    entity_type: "feed".into(),
                    indexed_fields: vec![
                        PpkIndexedField {
                            field_path: "/title".into(),
                            field_type: "text".into(),
                            searchable: true,
                        },
                        PpkIndexedField {
                            field_path: "/url".into(),
                            field_type: "text".into(),
                            searchable: false,
                        },
                    ],
                    merge_strategy: "lww_per_field".into(),
                },
            ],
        };

        let toml_str = toml::to_string_pretty(&manifest).expect("serialize");
        let parsed: PpkManifest = toml::from_str(&toml_str).expect("deserialize");

        assert_eq!(manifest.id, parsed.id);
        assert_eq!(manifest.name, parsed.name);
        assert_eq!(manifest.permissions.len(), parsed.permissions.len());
        assert_eq!(manifest.schemas.len(), parsed.schemas.len());
        assert_eq!(manifest.schemas[0].entity_type, parsed.schemas[0].entity_type);
        assert_eq!(manifest.schemas[0].indexed_fields.len(), parsed.schemas[0].indexed_fields.len());
    }

    #[test]
    fn content_hash_deterministic() {
        let manifest = PpkManifest {
            id: "privstack.hash".into(),
            name: "Hash Test".into(),
            description: "".into(),
            version: "1.0.0".into(),
            author: "PrivStack".into(),
            icon: None,
            navigation_order: 100,
            category: "utility".into(),
            can_disable: true,
            is_experimental: false,
            min_app_version: None,
            permissions: vec![],
            schemas: vec![],
        };

        let wasm = b"deterministic content";

        let pkg1 = PackageBuilder::new(manifest.clone())
            .wasm(wasm.to_vec())
            .build()
            .expect("build 1");
        let pkg2 = PackageBuilder::new(manifest)
            .wasm(wasm.to_vec())
            .build()
            .expect("build 2");

        let p1 = PpkPackage::open(Cursor::new(&pkg1)).unwrap();
        let p2 = PpkPackage::open(Cursor::new(&pkg2)).unwrap();

        assert_eq!(p1.content_hash(), p2.content_hash());
    }

    #[test]
    fn package_with_views() {
        let manifest = PpkManifest {
            id: "privstack.views".into(),
            name: "Views Test".into(),
            description: "".into(),
            version: "1.0.0".into(),
            author: "PrivStack".into(),
            icon: None,
            navigation_order: 100,
            category: "utility".into(),
            can_disable: true,
            is_experimental: false,
            min_app_version: None,
            permissions: vec![],
            schemas: vec![],
        };

        let ppk_bytes = PackageBuilder::new(manifest)
            .wasm(b"wasm".to_vec())
            .add_view("main.json", b"{\"type\":\"list\"}".to_vec())
            .add_view("detail.json", b"{\"type\":\"detail\"}".to_vec())
            .build()
            .expect("build");

        let pkg = PpkPackage::open(Cursor::new(&ppk_bytes)).unwrap();
        assert_eq!(pkg.views.len(), 2);
        assert!(pkg.views.contains_key("main.json"));
        assert!(pkg.views.contains_key("detail.json"));
    }

    #[test]
    fn keypair_export_import() {
        let keypair = KeyPair::generate();

        let secret_bytes = keypair.signing_key.to_bytes();
        let public_bytes = keypair.verifying_key.to_bytes();

        let restored_signing = SigningKey::from_bytes(&secret_bytes);
        let restored_verifying = VerifyingKey::from_bytes(&public_bytes).unwrap();

        // Sign with original, verify with restored
        let message = b"test message";
        let sig = keypair.signing_key.sign(message);
        assert!(restored_verifying.verify(message, &sig).is_ok());

        // Sign with restored, verify with original
        let sig2 = restored_signing.sign(message);
        assert!(keypair.verifying_key.verify(message, &sig2).is_ok());
    }

    #[test]
    fn permission_serde() {
        let perms = vec![
            PpkPermission::EntityCrud,
            PpkPermission::EntityQuery,
            PpkPermission::ViewState,
            PpkPermission::CommandPalette,
            PpkPermission::VaultAccess,
            PpkPermission::CrossPluginLink,
            PpkPermission::DialogDisplay,
            PpkPermission::TimerAccess,
            PpkPermission::NetworkAccess,
        ];

        for perm in &perms {
            let json = serde_json::to_string(perm).unwrap();
            let parsed: PpkPermission = serde_json::from_str(&json).unwrap();
            assert_eq!(*perm, parsed);
        }
    }
}
