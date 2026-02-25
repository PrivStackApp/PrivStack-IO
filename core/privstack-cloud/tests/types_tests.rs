use chrono::{Datelike, Duration, Utc};
use privstack_cloud::*;

// --- StsCredentials ---

fn make_creds(expires_in_secs: i64) -> StsCredentials {
    StsCredentials {
        access_key_id: "AKIA_TEST".into(),
        secret_access_key: "secret".into(),
        session_token: "token".into(),
        expires_at: Utc::now() + Duration::seconds(expires_in_secs),
        bucket: "test-bucket".into(),
        region: "us-east-2".into(),
        prefix: None,
        endpoint: None,
    }
}

#[test]
fn sts_is_expired_when_past() {
    let creds = make_creds(-10);
    assert!(creds.is_expired());
}

#[test]
fn sts_not_expired_when_future() {
    let creds = make_creds(3600);
    assert!(!creds.is_expired());
}

#[test]
fn sts_expires_within_secs_true() {
    let creds = make_creds(100);
    assert!(creds.expires_within_secs(200));
}

#[test]
fn sts_expires_within_secs_false() {
    let creds = make_creds(3600);
    assert!(!creds.expires_within_secs(300));
}

#[test]
fn sts_expires_within_secs_exact_boundary() {
    // Just within the margin
    let creds = make_creds(299);
    assert!(creds.expires_within_secs(300));
}

// --- Serialization roundtrips ---

#[test]
fn sts_credentials_roundtrip() {
    let creds = make_creds(3600);
    let json = serde_json::to_string(&creds).unwrap();
    let de: StsCredentials = serde_json::from_str(&json).unwrap();
    assert_eq!(de.access_key_id, "AKIA_TEST");
    assert_eq!(de.bucket, "test-bucket");
}

#[test]
fn cloud_workspace_roundtrip() {
    let ws = CloudWorkspace {
        id: 1,
        user_id: 42,
        workspace_id: "ws-uuid".into(),
        workspace_name: "My WS".into(),
        s3_prefix: "users/42/workspaces/ws-uuid".into(),
        storage_used_bytes: 1024,
        storage_quota_bytes: 1_000_000,
        created_at: Utc::now(),
    };
    let json = serde_json::to_string(&ws).unwrap();
    let de: CloudWorkspace = serde_json::from_str(&json).unwrap();
    assert_eq!(de.workspace_id, "ws-uuid");
    assert_eq!(de.storage_used_bytes, 1024);
}

#[test]
fn sync_cursor_roundtrip() {
    let cursor = SyncCursor {
        entity_id: "e-1".into(),
        cursor_position: 42,
        last_batch_key: Some("batch.enc".into()),
    };
    let json = serde_json::to_string(&cursor).unwrap();
    let de: SyncCursor = serde_json::from_str(&json).unwrap();
    assert_eq!(de.cursor_position, 42);
    assert_eq!(de.last_batch_key, Some("batch.enc".into()));
}

#[test]
fn batch_meta_roundtrip() {
    let meta = BatchMeta {
        s3_key: "key.enc".into(),
        cursor_start: 0,
        cursor_end: 10,
        size_bytes: 512,
        event_count: 5,
        is_snapshot: false,
    };
    let json = serde_json::to_string(&meta).unwrap();
    let de: BatchMeta = serde_json::from_str(&json).unwrap();
    assert_eq!(de.s3_key, "key.enc");
    assert!(!de.is_snapshot);
}

#[test]
fn share_info_roundtrip() {
    let info = ShareInfo {
        share_id: 1,
        entity_id: "e-1".into(),
        entity_type: "note".into(),
        entity_name: Some("My Note".into()),
        recipient_email: "bob@example.com".into(),
        permission: SharePermission::Read,
        status: ShareStatus::Pending,
        created_at: Utc::now(),
        accepted_at: None,
    };
    let json = serde_json::to_string(&info).unwrap();
    let de: ShareInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(de.entity_id, "e-1");
    assert_eq!(de.permission, SharePermission::Read);
}

#[test]
fn quota_info_roundtrip() {
    let qi = QuotaInfo {
        storage_used_bytes: 500,
        storage_quota_bytes: 1000,
        usage_percent: 50.0,
    };
    let json = serde_json::to_string(&qi).unwrap();
    let de: QuotaInfo = serde_json::from_str(&json).unwrap();
    assert!((de.usage_percent - 50.0).abs() < f64::EPSILON);
}

#[test]
fn shared_entity_roundtrip() {
    let se = SharedEntity {
        entity_id: "e-1".into(),
        entity_type: "note".into(),
        entity_name: None,
        owner_user_id: 5,
        workspace_id: "ws-1".into(),
        permission: SharePermission::Write,
    };
    let json = serde_json::to_string(&se).unwrap();
    let de: SharedEntity = serde_json::from_str(&json).unwrap();
    assert_eq!(de.permission, SharePermission::Write);
    assert!(de.entity_name.is_none());
}

#[test]
fn share_permission_serde_lowercase() {
    let json = serde_json::to_string(&SharePermission::Read).unwrap();
    assert_eq!(json, "\"read\"");
    let json = serde_json::to_string(&SharePermission::Write).unwrap();
    assert_eq!(json, "\"write\"");
}

#[test]
fn share_status_serde_lowercase() {
    let json = serde_json::to_string(&ShareStatus::Pending).unwrap();
    assert_eq!(json, "\"pending\"");
    let json = serde_json::to_string(&ShareStatus::Accepted).unwrap();
    assert_eq!(json, "\"accepted\"");
    let json = serde_json::to_string(&ShareStatus::Revoked).unwrap();
    assert_eq!(json, "\"revoked\"");
}

#[test]
fn share_permission_equality() {
    assert_eq!(SharePermission::Read, SharePermission::Read);
    assert_ne!(SharePermission::Read, SharePermission::Write);
}

#[test]
fn share_status_equality() {
    assert_eq!(ShareStatus::Pending, ShareStatus::Pending);
    assert_ne!(ShareStatus::Pending, ShareStatus::Accepted);
}

#[test]
fn blob_meta_roundtrip() {
    let bm = BlobMeta {
        blob_id: "b-1".into(),
        entity_id: Some("e-1".into()),
        s3_key: "blob.enc".into(),
        size_bytes: 256,
        content_hash: Some("abc123".into()),
    };
    let json = serde_json::to_string(&bm).unwrap();
    let de: BlobMeta = serde_json::from_str(&json).unwrap();
    assert_eq!(de.blob_id, "b-1");
}

#[test]
fn blob_meta_optional_fields() {
    let bm = BlobMeta {
        blob_id: "b-1".into(),
        entity_id: None,
        s3_key: "blob.enc".into(),
        size_bytes: 0,
        content_hash: None,
    };
    let json = serde_json::to_string(&bm).unwrap();
    let de: BlobMeta = serde_json::from_str(&json).unwrap();
    assert!(de.entity_id.is_none());
    assert!(de.content_hash.is_none());
}

#[test]
fn auth_tokens_roundtrip() {
    let tokens = AuthTokens {
        access_token: "at".into(),
        refresh_token: "rt".into(),
        user_id: 1,
        email: "test@example.com".into(),
    };
    let json = serde_json::to_string(&tokens).unwrap();
    let de: AuthTokens = serde_json::from_str(&json).unwrap();
    assert_eq!(de.email, "test@example.com");
}

#[test]
fn device_info_roundtrip() {
    let di = DeviceInfo {
        device_id: "dev-1".into(),
        device_name: Some("MacBook".into()),
        platform: Some("macos".into()),
        last_seen_at: Some(Utc::now()),
    };
    let json = serde_json::to_string(&di).unwrap();
    let de: DeviceInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(de.device_id, "dev-1");
}

#[test]
fn device_info_all_optional_null() {
    let di = DeviceInfo {
        device_id: "dev-1".into(),
        device_name: None,
        platform: None,
        last_seen_at: None,
    };
    let json = serde_json::to_string(&di).unwrap();
    let de: DeviceInfo = serde_json::from_str(&json).unwrap();
    assert!(de.device_name.is_none());
    assert!(de.platform.is_none());
    assert!(de.last_seen_at.is_none());
}

#[test]
fn cloud_sync_status_roundtrip() {
    let status = CloudSyncStatus {
        is_syncing: true,
        is_authenticated: true,
        active_workspace: Some("ws-1".into()),
        pending_upload_count: 5,
        last_sync_at: Some(Utc::now()),
        connected_devices: 2,
        is_rate_limited: false,
        rate_limit_remaining_secs: 0,
        synced_entity_count: 3,
        total_entity_count: 5,
    };
    let json = serde_json::to_string(&status).unwrap();
    let de: CloudSyncStatus = serde_json::from_str(&json).unwrap();
    assert!(de.is_syncing);
    assert_eq!(de.pending_upload_count, 5);
}

#[test]
fn advance_cursor_request_roundtrip() {
    let req = AdvanceCursorRequest {
        workspace_id: "ws-1".into(),
        device_id: "dev-1".into(),
        entity_id: "e-1".into(),
        cursor_position: 10,
        batch_key: "batch.enc".into(),
        size_bytes: 1024,
        event_count: 5,
    };
    let json = serde_json::to_string(&req).unwrap();
    let de: AdvanceCursorRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(de.cursor_position, 10);
}

#[test]
fn create_share_request_roundtrip() {
    let req = CreateShareRequest {
        entity_id: "e-1".into(),
        entity_type: "note".into(),
        entity_name: Some("My Note".into()),
        workspace_id: "ws-1".into(),
        recipient_email: "bob@example.com".into(),
        permission: SharePermission::Write,
    };
    let json = serde_json::to_string(&req).unwrap();
    let de: CreateShareRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(de.permission, SharePermission::Write);
}

#[test]
fn register_blob_request_roundtrip() {
    let req = RegisterBlobRequest {
        workspace_id: "ws-1".into(),
        blob_id: "b-1".into(),
        entity_id: Some("e-1".into()),
        s3_key: "blob.enc".into(),
        size_bytes: 256,
        content_hash: Some("abc".into()),
    };
    let json = serde_json::to_string(&req).unwrap();
    let de: RegisterBlobRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(de.blob_id, "b-1");
}

#[test]
fn pending_changes_roundtrip() {
    let pc = PendingChanges {
        pending: vec![PendingEntity {
            entity_id: "e-1".into(),
            latest_cursor: 10,
            device_cursor: 5,
        }],
    };
    let json = serde_json::to_string(&pc).unwrap();
    let de: PendingChanges = serde_json::from_str(&json).unwrap();
    assert_eq!(de.pending.len(), 1);
    assert_eq!(de.pending[0].latest_cursor, 10);
}

// --- RateLimitConfig ---

#[test]
fn rate_limit_config_defaults() {
    let config = RateLimitConfig::default();
    assert_eq!(config.window_seconds, 60);
    assert_eq!(config.max_requests_per_window, 600);
    assert_eq!(config.recommended_poll_interval_secs, 30);
    assert_eq!(config.flush_batch_size, 25);
    assert_eq!(config.inter_entity_delay_ms, 120);
}

#[test]
fn rate_limit_config_roundtrip() {
    let config = RateLimitConfig {
        window_seconds: 120,
        max_requests_per_window: 300,
        recommended_poll_interval_secs: 60,
        flush_batch_size: 10,
        inter_entity_delay_ms: 250,
    };
    let json = serde_json::to_string(&config).unwrap();
    let de: RateLimitConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(de.window_seconds, 120);
    assert_eq!(de.flush_batch_size, 10);
}

#[test]
fn rate_limit_config_deserialize_from_server() {
    // Simulate server JSON response
    let json = r#"{"window_seconds":60,"max_requests_per_window":600,"recommended_poll_interval_secs":30,"flush_batch_size":25,"inter_entity_delay_ms":120}"#;
    let config: RateLimitConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.max_requests_per_window, 600);
}

// --- SyncProgress ---

#[test]
fn sync_progress_default() {
    let progress = SyncProgress::default();
    assert_eq!(progress.synced_count, 0);
    assert_eq!(progress.total_count, 0);
}

// --- CloudSyncStatus rate-limit fields ---

#[test]
fn cloud_sync_status_rate_limit_defaults() {
    // When deserializing old JSON without rate-limit fields, defaults should kick in
    let json = r#"{"is_syncing":false,"is_authenticated":true,"active_workspace":"ws","pending_upload_count":0,"last_sync_at":null,"connected_devices":1}"#;
    let status: CloudSyncStatus = serde_json::from_str(json).unwrap();
    assert!(!status.is_rate_limited);
    assert_eq!(status.rate_limit_remaining_secs, 0);
    assert_eq!(status.synced_entity_count, 0);
    assert_eq!(status.total_entity_count, 0);
}

// --- StsCredentials prefix/endpoint ---

#[test]
fn sts_credentials_deserialize_without_optional_fields() {
    // API responses before prefix/endpoint were added
    let json = r#"{"access_key_id":"AK","secret_access_key":"SK","session_token":"ST","expires_at":"2026-01-01T00:00:00Z","bucket":"b","region":"us-east-2"}"#;
    let creds: StsCredentials = serde_json::from_str(json).unwrap();
    assert!(creds.prefix.is_none());
    assert!(creds.endpoint.is_none());
}

#[test]
fn sts_credentials_deserialize_with_optional_fields() {
    let json = r#"{"access_key_id":"AK","secret_access_key":"SK","session_token":"ST","expires_at":"2026-01-01T00:00:00Z","bucket":"b","region":"us-east-2","prefix":"users/1/ws/abc","endpoint":"http://localhost:9000"}"#;
    let creds: StsCredentials = serde_json::from_str(json).unwrap();
    assert_eq!(creds.prefix.as_deref(), Some("users/1/ws/abc"));
    assert_eq!(creds.endpoint.as_deref(), Some("http://localhost:9000"));
}

// --- StsCredentials expiration alias ---

#[test]
fn sts_credentials_expiration_alias() {
    // API returns "expiration" but we alias it to "expires_at"
    let json = r#"{"access_key_id":"AK","secret_access_key":"SK","session_token":"ST","expiration":"2026-06-01T00:00:00Z","bucket":"b","region":"us-east-2"}"#;
    let creds: StsCredentials = serde_json::from_str(json).unwrap();
    assert_eq!(creds.expires_at.year(), 2026);
}

// --- BatchMeta is_snapshot deserialization ---

#[test]
fn batch_meta_is_snapshot_from_int() {
    let json = r#"{"s3_key":"k","cursor_start":0,"cursor_end":10,"size_bytes":512,"event_count":5,"is_snapshot":1}"#;
    let meta: BatchMeta = serde_json::from_str(json).unwrap();
    assert!(meta.is_snapshot);

    let json0 = r#"{"s3_key":"k","cursor_start":0,"cursor_end":10,"size_bytes":512,"event_count":5,"is_snapshot":0}"#;
    let meta0: BatchMeta = serde_json::from_str(json0).unwrap();
    assert!(!meta0.is_snapshot);
}

// --- QuotaInfo usage_percent from string ---

#[test]
fn quota_info_usage_percent_from_string() {
    let json = r#"{"storage_used_bytes":500,"storage_quota_bytes":1000,"usage_percent":"50.00"}"#;
    let qi: QuotaInfo = serde_json::from_str(json).unwrap();
    assert!((qi.usage_percent - 50.0).abs() < f64::EPSILON);
}

#[test]
fn quota_info_usage_percent_from_number() {
    let json = r#"{"storage_used_bytes":500,"storage_quota_bytes":1000,"usage_percent":50.0}"#;
    let qi: QuotaInfo = serde_json::from_str(json).unwrap();
    assert!((qi.usage_percent - 50.0).abs() < f64::EPSILON);
}
