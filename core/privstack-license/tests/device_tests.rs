use privstack_license::{DeviceFingerprint, DeviceInfo};

#[test]
fn device_info_collection() {
    let info = DeviceInfo::collect();
    assert!(!info.os_name.is_empty());
    assert!(!info.arch.is_empty());
    assert!(!info.hostname.is_empty());
}

#[test]
fn device_info_serde() {
    let info = DeviceInfo::collect();
    let json = serde_json::to_string(&info).unwrap();
    let parsed: DeviceInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.os_name, info.os_name);
    assert_eq!(parsed.arch, info.arch);
}

#[test]
fn fingerprint_generation() {
    let fp = DeviceFingerprint::generate();
    assert!(!fp.id().is_empty());
    assert!(fp.matches_current());
}

#[test]
fn fingerprint_stability() {
    let fp1 = DeviceFingerprint::generate();
    let fp2 = DeviceFingerprint::generate();
    assert_eq!(fp1.id(), fp2.id());
}

#[test]
fn fingerprint_matches_self() {
    let fp = DeviceFingerprint::generate();
    assert!(fp.matches_current());
}

#[test]
fn fingerprint_serialization_roundtrip() {
    let fp = DeviceFingerprint::generate();
    let json = serde_json::to_string(&fp).unwrap();
    let parsed: DeviceFingerprint = serde_json::from_str(&json).unwrap();
    assert_eq!(fp.id(), parsed.id());
    assert_eq!(fp, parsed);
}

#[test]
fn fingerprint_equality() {
    let fp1 = DeviceFingerprint::generate();
    let fp2 = DeviceFingerprint::generate();
    // Same device → same id → equal
    assert_eq!(fp1.id(), fp2.id());
}

#[test]
fn fingerprint_clone() {
    let fp = DeviceFingerprint::generate();
    let cloned = fp.clone();
    assert_eq!(fp.id(), cloned.id());
}

#[test]
fn device_info_clone() {
    let info = DeviceInfo::collect();
    let cloned = info.clone();
    assert_eq!(cloned.os_name, info.os_name);
}
