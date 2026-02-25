//! Tests for sync error types.

use privstack_sync::{SyncError, SyncResult};

#[test]
fn network_error_display() {
    let err = SyncError::Network("connection reset".into());
    assert_eq!(err.to_string(), "network error: connection reset");
}

#[test]
fn protocol_error_display() {
    let err = SyncError::Protocol("invalid frame header".into());
    assert_eq!(err.to_string(), "protocol error: invalid frame header");
}

#[test]
fn storage_error_display() {
    let err = SyncError::Storage("disk full".into());
    assert_eq!(err.to_string(), "storage error: disk full");
}

#[test]
fn auth_error_display() {
    let err = SyncError::Auth("token expired".into());
    assert_eq!(err.to_string(), "authentication error: token expired");
}

#[test]
fn peer_not_found_display() {
    let err = SyncError::PeerNotFound("12D3KooW...".into());
    assert_eq!(err.to_string(), "peer not found: 12D3KooW...");
}

#[test]
fn connection_refused_display() {
    let err = SyncError::ConnectionRefused("192.168.1.1:9090".into());
    assert_eq!(err.to_string(), "connection refused: 192.168.1.1:9090");
}

#[test]
fn timeout_display() {
    let err = SyncError::Timeout;
    assert_eq!(err.to_string(), "operation timed out");
}

#[test]
fn channel_closed_display() {
    let err = SyncError::ChannelClosed;
    assert_eq!(err.to_string(), "channel closed");
}

#[test]
fn policy_denied_display() {
    let err = SyncError::PolicyDenied {
        reason: "read-only workspace".into(),
    };
    assert_eq!(err.to_string(), "policy denied: read-only workspace");
}

#[test]
fn serialization_error_from_serde() {
    let serde_err = serde_json::from_str::<serde_json::Value>("{{invalid}").unwrap_err();
    let err: SyncError = serde_err.into();
    let display = err.to_string();
    assert!(display.starts_with("serialization error:"), "got: {display}");
}

#[test]
fn sync_result_ok() {
    let result: SyncResult<i32> = Ok(42);
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn sync_result_err() {
    let result: SyncResult<i32> = Err(SyncError::Timeout);
    assert!(result.is_err());
}

#[test]
fn errors_are_debug_printable() {
    let variants: Vec<SyncError> = vec![
        SyncError::Network("test".into()),
        SyncError::Protocol("test".into()),
        SyncError::Storage("test".into()),
        SyncError::Auth("test".into()),
        SyncError::PeerNotFound("test".into()),
        SyncError::ConnectionRefused("test".into()),
        SyncError::Timeout,
        SyncError::ChannelClosed,
        SyncError::PolicyDenied { reason: "test".into() },
    ];
    for err in &variants {
        let debug = format!("{:?}", err);
        assert!(!debug.is_empty());
    }
}

#[test]
fn errors_implement_std_error() {
    let err = SyncError::Network("test".into());
    let std_err: &dyn std::error::Error = &err;
    assert!(std_err.source().is_none()); // Network variant has no source

    // Serialization variant should have a source
    let serde_err = serde_json::from_str::<serde_json::Value>("bad").unwrap_err();
    let err: SyncError = serde_err.into();
    let std_err: &dyn std::error::Error = &err;
    assert!(std_err.source().is_some());
}

#[test]
fn policy_denied_empty_reason() {
    let err = SyncError::PolicyDenied { reason: String::new() };
    assert_eq!(err.to_string(), "policy denied: ");
}

#[test]
fn network_error_with_unicode() {
    let err = SyncError::Network("连接失败 — réseau échoué".into());
    let display = err.to_string();
    assert!(display.contains("连接失败"));
    assert!(display.contains("réseau"));
}
