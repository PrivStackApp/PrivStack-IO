use std::io::Cursor;

use privstack_ppk::*;

fn test_manifest() -> PpkManifest {
    PpkManifest {
        id: "privstack.test".into(),
        name: "Test".into(),
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
    }
}

// ── Manifest Validation ────────────────────────────────────────

#[test]
fn validate_valid_manifest() {
    let m = PpkManifest {
        id: "privstack.test".into(),
        name: "Test".into(),
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
    assert!(m.validate().is_ok());
}

#[test]
fn validate_empty_id() {
    let m = PpkManifest {
        id: "".into(),
        name: "Test".into(),
        description: "".into(),
        version: "1.0.0".into(),
        author: "".into(),
        icon: None,
        navigation_order: 100,
        category: "utility".into(),
        can_disable: true,
        is_experimental: false,
        min_app_version: None,
        permissions: vec![],
        schemas: vec![],
    };
    assert!(m.validate().is_err());
}

#[test]
fn validate_no_dot_in_id() {
    let m = PpkManifest {
        id: "invalid".into(),
        name: "Test".into(),
        description: "".into(),
        version: "1.0.0".into(),
        author: "".into(),
        icon: None,
        navigation_order: 100,
        category: "utility".into(),
        can_disable: true,
        is_experimental: false,
        min_app_version: None,
        permissions: vec![],
        schemas: vec![],
    };
    assert!(m.validate().is_err());
}

#[test]
fn is_first_party() {
    let m = PpkManifest {
        id: "privstack.rss".into(),
        name: "RSS".into(),
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
    assert!(m.is_first_party());

    let m2 = PpkManifest {
        id: "community.weather".into(),
        ..m
    };
    assert!(!m2.is_first_party());
}

// ── Package Builder & Roundtrip ──────────────────────────────────

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
fn pack_unpack_minimal() {
    let bytes = PackageBuilder::new(test_manifest())
        .build()
        .unwrap();
    let pkg = PpkPackage::open(Cursor::new(&bytes)).unwrap();
    assert_eq!(pkg.manifest.id, "privstack.test");
    assert!(pkg.wasm.is_none());
}

// ── Signing & Verification ──────────────────────────────────────

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
    let manifest = test_manifest();
    let real_keypair = KeyPair::generate();
    let wrong_keypair = KeyPair::generate();

    let ppk_bytes = PackageBuilder::new(manifest)
        .wasm(b"tampered wasm".to_vec())
        .sign(&real_keypair.signing_key)
        .build()
        .expect("pack+sign should succeed");

    let pkg = PpkPackage::open(Cursor::new(&ppk_bytes)).expect("unpack should succeed");
    let result = pkg.verify(&wrong_keypair.verifying_key);
    assert!(result.is_err());
}

#[test]
fn unsigned_package_verify_fails() {
    let ppk_bytes = PackageBuilder::new(test_manifest())
        .wasm(b"content".to_vec())
        .build()
        .expect("pack should succeed");

    let pkg = PpkPackage::open(Cursor::new(&ppk_bytes)).expect("unpack should succeed");
    let keypair = KeyPair::generate();
    let result = pkg.verify(&keypair.verifying_key);
    assert!(result.is_err());
}

#[test]
fn content_hash_deterministic() {
    let manifest = test_manifest();
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
fn content_hash_excludes_signature() {
    let manifest = test_manifest();
    let unsigned = PackageBuilder::new(manifest.clone())
        .wasm(b"wasm".to_vec())
        .build()
        .unwrap();

    let kp = KeyPair::generate();
    let signed = PackageBuilder::new(manifest)
        .wasm(b"wasm".to_vec())
        .sign(&kp.signing_key)
        .build()
        .unwrap();

    let p1 = PpkPackage::open(Cursor::new(&unsigned)).unwrap();
    let p2 = PpkPackage::open(Cursor::new(&signed)).unwrap();
    assert_eq!(p1.content_hash(), p2.content_hash());
    assert!(p2.signature.is_some());
}

// ── Views ────────────────────────────────────────────────────────

#[test]
fn package_with_views() {
    let ppk_bytes = PackageBuilder::new(test_manifest())
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

// ── Manifest TOML ────────────────────────────────────────────────

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

// ── Key Export/Import ────────────────────────────────────────────

#[test]
fn keypair_export_import() {
    let keypair = KeyPair::generate();

    let secret_bytes = keypair.signing_key.to_bytes();
    let public_bytes = keypair.verifying_key.to_bytes();

    let restored_signing = SigningKey::from_bytes(&secret_bytes);
    let restored_verifying = VerifyingKey::from_bytes(&public_bytes).unwrap();

    let message = b"test message";
    let sig = keypair.signing_key.sign(message);
    assert!(restored_verifying.verify(message, &sig).is_ok());

    let sig2 = restored_signing.sign(message);
    assert!(keypair.verifying_key.verify(message, &sig2).is_ok());
}

// ── Permission Serde ─────────────────────────────────────────────

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

// ── Signing primitives ──────────────────────────────────────────

#[test]
fn sign_verify_roundtrip() {
    let kp = KeyPair::generate();
    let msg = b"hello world";
    let sig = kp.signing_key.sign(msg);
    assert!(kp.verifying_key.verify(msg, &sig).is_ok());
}

#[test]
fn wrong_message_fails() {
    let kp = KeyPair::generate();
    let sig = kp.signing_key.sign(b"correct");
    assert!(kp.verifying_key.verify(b"wrong", &sig).is_err());
}

#[test]
fn wrong_key_fails() {
    let kp1 = KeyPair::generate();
    let kp2 = KeyPair::generate();
    let sig = kp1.signing_key.sign(b"message");
    assert!(kp2.verifying_key.verify(b"message", &sig).is_err());
}

#[test]
fn key_bytes_roundtrip() {
    let kp = KeyPair::generate();
    let secret = kp.signing_key.to_bytes();
    let public = kp.verifying_key.to_bytes();

    let sk = SigningKey::from_bytes(&secret);
    let vk = VerifyingKey::from_bytes(&public).unwrap();

    let sig = sk.sign(b"test");
    assert!(vk.verify(b"test", &sig).is_ok());
}

#[test]
fn signature_bytes_roundtrip() {
    let kp = KeyPair::generate();
    let sig = kp.signing_key.sign(b"data");
    let bytes = sig.to_bytes();
    let restored = Signature::from_bytes(&bytes);
    assert!(kp.verifying_key.verify(b"data", &restored).is_ok());
}

#[test]
fn verifying_key_from_signing_key() {
    let kp = KeyPair::generate();
    let derived = kp.signing_key.verifying_key();
    let sig = kp.signing_key.sign(b"check");
    assert!(derived.verify(b"check", &sig).is_ok());
}
