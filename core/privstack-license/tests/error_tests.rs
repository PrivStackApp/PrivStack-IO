use privstack_license::LicenseError;

#[test]
fn error_display_invalid_key_format() {
    let err = LicenseError::InvalidKeyFormat("bad format".into());
    assert!(format!("{err}").contains("invalid license key format"));
}

#[test]
fn error_display_invalid_signature() {
    let err = LicenseError::InvalidSignature;
    assert!(format!("{err}").contains("signature"));
}

#[test]
fn error_display_invalid_payload() {
    let err = LicenseError::InvalidPayload("missing field".into());
    let msg = format!("{err}");
    assert!(msg.contains("invalid license payload"));
    assert!(msg.contains("missing field"));
}

#[test]
fn error_display_expired() {
    let err = LicenseError::Expired("2025-01-01".into());
    assert!(format!("{err}").contains("expired"));
}

#[test]
fn error_display_not_activated() {
    let err = LicenseError::NotActivated;
    assert!(format!("{err}").contains("not activated"));
}

#[test]
fn error_display_activation_failed() {
    let err = LicenseError::ActivationFailed("server error".into());
    assert!(format!("{err}").contains("activation failed"));
}

#[test]
fn error_display_device_limit() {
    let err = LicenseError::DeviceLimitExceeded(5);
    let msg = format!("{err}");
    assert!(msg.contains("device limit"));
    assert!(msg.contains("5"));
}

#[test]
fn error_display_revoked() {
    let err = LicenseError::Revoked;
    assert!(format!("{err}").contains("revoked"));
}

#[test]
fn error_display_network() {
    let err = LicenseError::Network("timeout".into());
    assert!(format!("{err}").contains("network"));
}

#[test]
fn error_display_storage() {
    let err = LicenseError::Storage("disk full".into());
    assert!(format!("{err}").contains("storage"));
}

#[test]
fn error_from_serde_json() {
    let serde_err: Result<serde_json::Value, _> = serde_json::from_str("not json");
    let license_err: LicenseError = serde_err.unwrap_err().into();
    assert!(format!("{license_err}").contains("serialization"));
}

#[test]
fn error_is_debug() {
    let err = LicenseError::Revoked;
    let _ = format!("{err:?}");
}
