mod common;

use common::{make_perpetual_key, test_keypair};
use privstack_license::{DeviceFingerprint, LicenseKey};

#[test]
fn license_key_parse_and_validate() {
    let (sk, pk) = test_keypair();
    let key_str = make_perpetual_key(&sk);
    let key = LicenseKey::parse_with_key(&key_str, &pk);
    assert!(key.is_ok());
}

#[test]
fn invalid_key_rejected() {
    let (_, pk) = test_keypair();
    let key = LicenseKey::parse_with_key("invalid", &pk);
    assert!(key.is_err());
}

#[test]
fn device_fingerprint_stable() {
    let fp1 = DeviceFingerprint::generate();
    let fp2 = DeviceFingerprint::generate();
    assert_eq!(fp1.id(), fp2.id());
}
