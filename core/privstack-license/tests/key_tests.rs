mod common;

use common::{make_annual_key_at, make_monthly_key_at, make_perpetual_key, sign_key, test_keypair};
use privstack_license::{LicenseKey, LicensePlan, LicenseStatus, GRACE_PERIOD_SECS};

// ── LicensePlan ──────────────────────────────────────────────────

#[test]
fn max_devices() {
    assert_eq!(LicensePlan::Monthly.max_devices(), 3);
    assert_eq!(LicensePlan::Annual.max_devices(), 5);
    assert_eq!(LicensePlan::Perpetual.max_devices(), 5);
}

#[test]
fn has_priority_support() {
    assert!(!LicensePlan::Monthly.has_priority_support());
    assert!(LicensePlan::Annual.has_priority_support());
    assert!(LicensePlan::Perpetual.has_priority_support());
}

#[test]
fn duration_secs() {
    assert_eq!(LicensePlan::Monthly.duration_secs(), Some(30 * 24 * 60 * 60));
    assert_eq!(LicensePlan::Annual.duration_secs(), Some(365 * 24 * 60 * 60));
    assert_eq!(LicensePlan::Perpetual.duration_secs(), None);
}

#[test]
fn license_plan_serde() {
    let json = serde_json::to_string(&LicensePlan::Annual).unwrap();
    let parsed: LicensePlan = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, LicensePlan::Annual);
}

#[test]
fn license_plan_clone_copy() {
    let p = LicensePlan::Perpetual;
    let p2 = p;
    assert_eq!(p, p2);
}

// ── LicenseStatus ────────────────────────────────────────────────

#[test]
fn status_usability() {
    assert!(LicenseStatus::Active.is_usable());
    assert!(LicenseStatus::Grace { days_remaining: 7 }.is_usable());
    assert!(LicenseStatus::Grace { days_remaining: 0 }.is_usable());
    assert!(!LicenseStatus::ReadOnly.is_usable());
    assert!(!LicenseStatus::Expired.is_usable());
    assert!(!LicenseStatus::NotActivated.is_usable());
}

#[test]
fn status_viewability() {
    assert!(LicenseStatus::Active.is_viewable());
    assert!(LicenseStatus::Grace { days_remaining: 7 }.is_viewable());
    assert!(LicenseStatus::ReadOnly.is_viewable());
    assert!(!LicenseStatus::Expired.is_viewable());
    assert!(!LicenseStatus::NotActivated.is_viewable());
}

#[test]
fn status_serde() {
    let statuses = vec![
        LicenseStatus::Active,
        LicenseStatus::Grace { days_remaining: 14 },
        LicenseStatus::ReadOnly,
        LicenseStatus::Expired,
        LicenseStatus::NotActivated,
    ];
    for status in statuses {
        let json = serde_json::to_string(&status).unwrap();
        let parsed: LicenseStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, status);
    }
}

// ── LicenseKey parsing ───────────────────────────────────────────

#[test]
fn parse_perpetual_key() {
    let (sk, pk) = test_keypair();
    let key_str = make_perpetual_key(&sk);
    let parsed = LicenseKey::parse_with_key(&key_str, &pk).unwrap();
    assert_eq!(parsed.license_plan(), LicensePlan::Perpetual);
    assert!(parsed.expires_at_secs().is_none());
}

#[test]
fn parse_monthly_key() {
    let (sk, pk) = test_keypair();
    let now = chrono::Utc::now().timestamp();
    let key_str = make_monthly_key_at(&sk, now);
    let parsed = LicenseKey::parse_with_key(&key_str, &pk).unwrap();
    assert_eq!(parsed.license_plan(), LicensePlan::Monthly);
    assert!(parsed.expires_at_secs().is_some());
}

#[test]
fn parse_annual_key() {
    let (sk, pk) = test_keypair();
    let now = chrono::Utc::now().timestamp();
    let key_str = make_annual_key_at(&sk, now);
    let parsed = LicenseKey::parse_with_key(&key_str, &pk).unwrap();
    assert_eq!(parsed.license_plan(), LicensePlan::Annual);
}

#[test]
fn parse_with_whitespace() {
    let (sk, pk) = test_keypair();
    let key_str = make_perpetual_key(&sk);
    let padded = format!("  {}  ", key_str);
    let parsed = LicenseKey::parse_with_key(&padded, &pk);
    assert!(parsed.is_ok());
}

// ── Invalid keys ─────────────────────────────────────────────────

#[test]
fn parse_invalid_no_dot() {
    let (_, pk) = test_keypair();
    let result = LicenseKey::parse_with_key("nodothere", &pk);
    assert!(result.is_err());
}

#[test]
fn parse_invalid_three_dots() {
    let (_, pk) = test_keypair();
    let result = LicenseKey::parse_with_key("a.b.c", &pk);
    assert!(result.is_err());
}

#[test]
fn parse_invalid_tampered_payload() {
    let (sk, pk) = test_keypair();
    let key_str = make_perpetual_key(&sk);
    // Tamper with the payload part (swap first char)
    let parts: Vec<&str> = key_str.split('.').collect();
    let tampered = format!("X{}.{}", &parts[0][1..], parts[1]);
    let result = LicenseKey::parse_with_key(&tampered, &pk);
    assert!(result.is_err());
}

#[test]
fn parse_invalid_tampered_signature() {
    let (sk, pk) = test_keypair();
    let key_str = make_perpetual_key(&sk);
    let parts: Vec<&str> = key_str.split('.').collect();
    let tampered = format!("{}.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", parts[0]);
    let result = LicenseKey::parse_with_key(&tampered, &pk);
    assert!(result.is_err());
}

#[test]
fn parse_invalid_bad_base64() {
    let (_, pk) = test_keypair();
    let result = LicenseKey::parse_with_key("!!!.!!!", &pk);
    assert!(result.is_err());
}

#[test]
fn parse_invalid_json() {
    let (sk, pk) = test_keypair();
    // Sign valid base64 that contains non-JSON
    let key_str = sign_key(&sk, "not json at all");
    let result = LicenseKey::parse_with_key(&key_str, &pk);
    assert!(result.is_err());
}

#[test]
fn parse_missing_fields() {
    let (sk, pk) = test_keypair();
    // Valid JSON but missing required fields
    let key_str = sign_key(&sk, r#"{"sub":1}"#);
    let result = LicenseKey::parse_with_key(&key_str, &pk);
    assert!(result.is_err());
}

// ── Status computation ───────────────────────────────────────────

#[test]
fn status_perpetual_is_active() {
    let (sk, pk) = test_keypair();
    let key_str = make_perpetual_key(&sk);
    let parsed = LicenseKey::parse_with_key(&key_str, &pk).unwrap();
    assert_eq!(parsed.status(), LicenseStatus::Active);
}

#[test]
fn status_fresh_monthly_is_active() {
    let (sk, pk) = test_keypair();
    let now = chrono::Utc::now().timestamp();
    let key_str = make_monthly_key_at(&sk, now);
    let parsed = LicenseKey::parse_with_key(&key_str, &pk).unwrap();
    assert_eq!(parsed.status(), LicenseStatus::Active);
}

#[test]
fn status_expired_monthly_in_grace() {
    let (sk, pk) = test_keypair();
    // Issued 31 days ago → expired 1 day ago → in grace
    let iat = chrono::Utc::now().timestamp() - 31 * 24 * 60 * 60;
    let key_str = make_monthly_key_at(&sk, iat);
    let parsed = LicenseKey::parse_with_key(&key_str, &pk).unwrap();
    match parsed.status() {
        LicenseStatus::Grace { days_remaining } => {
            assert!(days_remaining <= 29);
        }
        other => panic!("expected Grace, got {:?}", other),
    }
}

#[test]
fn status_old_monthly_is_readonly() {
    let (sk, pk) = test_keypair();
    // Issued 61 days ago → expired 31 days ago → past grace
    let iat = chrono::Utc::now().timestamp() - 61 * 24 * 60 * 60;
    let key_str = make_monthly_key_at(&sk, iat);
    let parsed = LicenseKey::parse_with_key(&key_str, &pk).unwrap();
    assert_eq!(parsed.status(), LicenseStatus::ReadOnly);
}

// ── Accessors ────────────────────────────────────────────────────

#[test]
fn key_payload_accessors() {
    let (sk, pk) = test_keypair();
    let key_str = make_perpetual_key(&sk);
    let parsed = LicenseKey::parse_with_key(&key_str, &pk).unwrap();

    assert_eq!(parsed.payload().sub, 1);
    assert_eq!(parsed.payload().email, "test@example.com");
    assert_eq!(parsed.payload().plan, LicensePlan::Perpetual);
    assert!(parsed.issued_at_secs() > 0);
}

#[test]
fn key_raw_preserved() {
    let (sk, pk) = test_keypair();
    let key_str = make_perpetual_key(&sk);
    let parsed = LicenseKey::parse_with_key(&key_str, &pk).unwrap();
    assert_eq!(parsed.raw(), key_str);
}

// ── Serde ────────────────────────────────────────────────────────

#[test]
fn key_serialization_roundtrip() {
    let (sk, pk) = test_keypair();
    let key_str = make_perpetual_key(&sk);
    let parsed = LicenseKey::parse_with_key(&key_str, &pk).unwrap();
    let json = serde_json::to_string(&parsed).unwrap();
    let restored: LicenseKey = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.raw(), restored.raw());
    assert_eq!(parsed.license_plan(), restored.license_plan());
}

// ── is_usable / is_viewable on status ────────────────────────────

#[test]
fn grace_is_usable() {
    let status = LicenseStatus::Grace { days_remaining: 5 };
    assert!(status.is_usable());
    assert!(status.is_viewable());
}

#[test]
fn readonly_is_not_usable_but_viewable() {
    let status = LicenseStatus::ReadOnly;
    assert!(!status.is_usable());
    assert!(status.is_viewable());
}

#[test]
fn grace_period_constant() {
    assert_eq!(GRACE_PERIOD_SECS, 30 * 24 * 60 * 60);
}
