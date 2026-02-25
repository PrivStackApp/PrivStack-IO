use privstack_ffi::*;
#[cfg(feature = "wasm-plugins")]
use privstack_plugin_host::{PolicyConfig, PolicyEngine, PluginHostManager};
use privstack_license::{ActivationStore, LicenseError, LicenseStatus, LicensePlan};
use privstack_sync::SyncEvent;
use privstack_types::PeerId;
use privstack_model::EntitySchema;
use serial_test::serial;
use std::ffi::{c_char, CStr, CString};
use std::ptr;
use uuid::Uuid;

/// Returns the temp path used for the test activation store.
fn test_activation_path() -> std::path::PathBuf {
    std::env::temp_dir()
        .join("privstack-ffi-tests")
        .join("activation.json")
}

/// Replaces the activation store in the global handle with one pointing
/// to a temp directory, isolating tests from the real activation file.
/// Writes a dummy activation JSON so that `ActivationStore::load()` hits
/// a signature-verification error → `Err(...)` → fail-open for mutations.
fn setup_test_activation_store() {
    let act_path = test_activation_path();
    let _ = std::fs::create_dir_all(act_path.parent().unwrap());
    let _ = std::fs::write(
        &act_path,
        r#"{"license_key":"test.dummy","license_plan":"perpetual","email":"test@test.com","sub":1,"activated_at":"2025-01-01T00:00:00Z","device_fingerprint":{"id":"test","generated_at":"2025-01-01T00:00:00Z"},"activation_token":"test"}"#,
    );
    let mut handle = HANDLE.lock().unwrap();
    if let Some(h) = handle.as_mut() {
        h.activation_store = ActivationStore::new(act_path);
    }
}

/// Removes the dummy activation file so `load()` returns `Ok(None)`.
/// Use in tests that verify "not activated" behavior.
fn test_clear_activation() {
    let _ = std::fs::remove_file(test_activation_path());
}

/// Initializes the runtime with an in-memory policy (no filesystem I/O).
#[cfg(feature = "wasm-plugins")]
fn test_init() -> PrivStackError {
    let r = init_with_plugin_host_builder(":memory:", |es, ev| {
        PluginHostManager::with_policy(es, ev, PolicyEngine::with_config(PolicyConfig::default()))
    });
    if r == PrivStackError::Ok {
        setup_test_activation_store();
    }
    r
}

/// Initializes the runtime without plugin host (no wasm-plugins feature).
#[cfg(not(feature = "wasm-plugins"))]
fn test_init() -> PrivStackError {
    let r = init_core(":memory:");
    if r == PrivStackError::Ok {
        setup_test_activation_store();
    }
    r
}

#[test]
fn version_returns_valid_string() {
    let version = privstack_version();
    assert!(!version.is_null());
    let version_str = unsafe { CStr::from_ptr(version) }.to_str().unwrap();
    assert_eq!(version_str, env!("CARGO_PKG_VERSION"));
}

#[test]
fn init_with_null_returns_error() {
    let result = unsafe { privstack_init(ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
#[serial]
fn init_and_shutdown() {
    let result = test_init();
    assert_eq!(result, PrivStackError::Ok);
    privstack_shutdown();
}

#[test]
#[serial]
fn vault_init_unlock_lock_via_auth() {
    let result = test_init();
    assert_eq!(result, PrivStackError::Ok);

    assert!(!privstack_auth_is_initialized());
    assert!(!privstack_auth_is_unlocked());

    let password = CString::new("testpassword123").unwrap();
    let result = unsafe { privstack_auth_initialize(password.as_ptr()) };
    assert_eq!(result, PrivStackError::Ok);

    assert!(privstack_auth_is_initialized());
    assert!(privstack_auth_is_unlocked());

    let result = privstack_auth_lock();
    assert_eq!(result, PrivStackError::Ok);
    assert!(!privstack_auth_is_unlocked());

    let result = unsafe { privstack_auth_unlock(password.as_ptr()) };
    assert_eq!(result, PrivStackError::Ok);
    assert!(privstack_auth_is_unlocked());

    privstack_shutdown();
}

#[test]
#[serial]
fn vault_blob_store_read_delete() {
    test_init();

    let vault_id = CString::new("test_vault").unwrap();
    let password = CString::new("password123").unwrap();
    unsafe {
        privstack_vault_create(vault_id.as_ptr());
        privstack_vault_initialize(vault_id.as_ptr(), password.as_ptr());
    }

    let blob_id = CString::new("blob1").unwrap();
    let data = b"secret data";
    let result = unsafe {
        privstack_vault_blob_store(
            vault_id.as_ptr(),
            blob_id.as_ptr(),
            data.as_ptr(),
            data.len(),
        )
    };
    assert_eq!(result, PrivStackError::Ok);

    let mut out_data: *mut u8 = ptr::null_mut();
    let mut out_len: usize = 0;
    let result = unsafe {
        privstack_vault_blob_read(
            vault_id.as_ptr(),
            blob_id.as_ptr(),
            &mut out_data,
            &mut out_len,
        )
    };
    assert_eq!(result, PrivStackError::Ok);
    assert_eq!(out_len, data.len());
    let read_data = unsafe { std::slice::from_raw_parts(out_data, out_len) };
    assert_eq!(read_data, data);
    unsafe { privstack_free_bytes(out_data, out_len) };

    let result =
        unsafe { privstack_vault_blob_delete(vault_id.as_ptr(), blob_id.as_ptr()) };
    assert_eq!(result, PrivStackError::Ok);

    privstack_shutdown();
}

// ── Null pointer checks ─────────────────────────────────────

#[test]
fn auth_initialize_null() {
    let result = unsafe { privstack_auth_initialize(ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn auth_unlock_null() {
    let result = unsafe { privstack_auth_unlock(ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn auth_change_password_null() {
    let result = unsafe { privstack_auth_change_password(ptr::null(), ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn vault_create_null() {
    let result = unsafe { privstack_vault_create(ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn vault_initialize_null() {
    let result = unsafe { privstack_vault_initialize(ptr::null(), ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn vault_unlock_null() {
    let result = unsafe { privstack_vault_unlock(ptr::null(), ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn vault_lock_null() {
    let result = unsafe { privstack_vault_lock(ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn vault_is_initialized_null() {
    let result = unsafe { privstack_vault_is_initialized(ptr::null()) };
    assert!(!result);
}

#[test]
fn vault_is_unlocked_null() {
    let result = unsafe { privstack_vault_is_unlocked(ptr::null()) };
    assert!(!result);
}

#[test]
fn vault_change_password_null() {
    let result = unsafe { privstack_vault_change_password(ptr::null(), ptr::null(), ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn vault_blob_store_null() {
    let result = unsafe { privstack_vault_blob_store(ptr::null(), ptr::null(), ptr::null(), 0) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn vault_blob_read_null() {
    let result = unsafe { privstack_vault_blob_read(ptr::null(), ptr::null(), ptr::null_mut(), ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn vault_blob_delete_null() {
    let result = unsafe { privstack_vault_blob_delete(ptr::null(), ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn vault_blob_list_null() {
    let result = unsafe { privstack_vault_blob_list(ptr::null(), ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn blob_store_null() {
    let result = unsafe { privstack_blob_store(ptr::null(), ptr::null(), ptr::null(), 0, ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn blob_read_null() {
    let result = unsafe { privstack_blob_read(ptr::null(), ptr::null(), ptr::null_mut(), ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn blob_delete_null() {
    let result = unsafe { privstack_blob_delete(ptr::null(), ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn blob_list_null() {
    let result = unsafe { privstack_blob_list(ptr::null(), ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn pairing_generate_code_null() {
    let result = unsafe { privstack_pairing_generate_code(ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn pairing_join_code_null() {
    let result = unsafe { privstack_pairing_join_code(ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn pairing_get_code_null() {
    let result = unsafe { privstack_pairing_get_code(ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn pairing_list_peers_null() {
    let result = unsafe { privstack_pairing_list_peers(ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn pairing_trust_peer_null() {
    let result = unsafe { privstack_pairing_trust_peer(ptr::null(), ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn pairing_remove_peer_null() {
    let result = unsafe { privstack_pairing_remove_peer(ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn pairing_save_state_null() {
    let result = unsafe { privstack_pairing_save_state(ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn pairing_load_state_null() {
    let result = unsafe { privstack_pairing_load_state(ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn pairing_get_device_name_null() {
    let result = unsafe { privstack_pairing_get_device_name(ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn pairing_set_device_name_null() {
    let result = unsafe { privstack_pairing_set_device_name(ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn cloud_init_google_drive_null() {
    let result = unsafe { privstack_cloud_init_google_drive(ptr::null(), ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn cloud_authenticate_null() {
    let result = unsafe { privstack_cloud_authenticate(CloudProvider::GoogleDrive, ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn cloud_complete_auth_null() {
    let result = unsafe { privstack_cloud_complete_auth(CloudProvider::GoogleDrive, ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn cloud_list_files_null() {
    let result = unsafe { privstack_cloud_list_files(CloudProvider::GoogleDrive, ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn cloud_upload_null() {
    let result = unsafe { privstack_cloud_upload(CloudProvider::GoogleDrive, ptr::null(), ptr::null(), 0, ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn cloud_download_null() {
    let result = unsafe { privstack_cloud_download(CloudProvider::GoogleDrive, ptr::null(), ptr::null_mut(), ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn cloud_delete_null() {
    let result = unsafe { privstack_cloud_delete(CloudProvider::GoogleDrive, ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn license_parse_null() {
    let result = unsafe { privstack_license_parse(ptr::null(), ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn license_get_plan_null() {
    let result = unsafe { privstack_license_get_plan(ptr::null(), ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn device_info_null() {
    let result = unsafe { privstack_device_info(ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn device_fingerprint_null() {
    let result = unsafe { privstack_device_fingerprint(ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn license_activate_null() {
    let result = unsafe { privstack_license_activate(ptr::null(), ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn license_check_null() {
    let result = unsafe { privstack_license_check(ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn license_status_null() {
    let result = unsafe { privstack_license_status(ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn license_activated_plan_null() {
    let result = unsafe { privstack_license_activated_plan(ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
fn register_entity_type_null() {
    let result = unsafe { privstack_register_entity_type(ptr::null()) };
    assert_eq!(result, -1);
}

// ── Not-initialized checks ──────────────────────────────────

#[test]
#[serial]
fn auth_not_initialized_returns_false() {
    privstack_shutdown(); // ensure clean state
    assert!(!privstack_auth_is_initialized());
    assert!(!privstack_auth_is_unlocked());
}

#[test]
#[serial]
fn vault_lock_all_not_initialized() {
    privstack_shutdown();
    let result = privstack_vault_lock_all();
    assert_eq!(result, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn auth_lock_not_initialized() {
    privstack_shutdown();
    let result = privstack_auth_lock();
    assert_eq!(result, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn sync_start_not_initialized() {
    privstack_shutdown();
    let result = privstack_sync_start();
    assert_eq!(result, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn sync_stop_not_initialized() {
    privstack_shutdown();
    let result = privstack_sync_stop();
    assert_eq!(result, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn cloud_is_authenticated_not_initialized() {
    privstack_shutdown();
    assert!(!privstack_cloud_is_authenticated(CloudProvider::GoogleDrive));
    assert!(!privstack_cloud_is_authenticated(CloudProvider::ICloud));
}

#[test]
#[serial]
fn license_is_valid_not_initialized() {
    privstack_shutdown();
    assert!(!privstack_license_is_valid());
}

#[test]
#[serial]
fn license_deactivate_not_initialized() {
    privstack_shutdown();
    let result = privstack_license_deactivate();
    assert_eq!(result, PrivStackError::NotInitialized);
}

// ── License utility functions ───────────────────────────────

#[test]
fn license_max_devices() {
    assert_eq!(privstack_license_max_devices(FfiLicensePlan::Trial), 1);
    assert_eq!(privstack_license_max_devices(FfiLicensePlan::Monthly), 3);
    assert_eq!(privstack_license_max_devices(FfiLicensePlan::Annual), 5);
    assert_eq!(privstack_license_max_devices(FfiLicensePlan::Perpetual), 5);
}

#[test]
fn license_has_priority_support() {
    assert!(!privstack_license_has_priority_support(FfiLicensePlan::Trial));
    assert!(!privstack_license_has_priority_support(FfiLicensePlan::Monthly));
    assert!(privstack_license_has_priority_support(FfiLicensePlan::Annual));
    assert!(privstack_license_has_priority_support(FfiLicensePlan::Perpetual));
}

// ── Free functions ──────────────────────────────────────────

#[test]
fn free_null_string() {
    unsafe { privstack_free_string(ptr::null_mut()) };
}

#[test]
fn free_null_bytes() {
    unsafe { privstack_free_bytes(ptr::null_mut(), 0) };
}

#[test]
fn free_valid_string() {
    let s = CString::new("test").unwrap();
    unsafe { privstack_free_string(s.into_raw()) };
}

// ── SyncEventDto conversion ─────────────────────────────────

#[test]
fn sync_event_dto_peer_discovered() {
    let event = SyncEvent::PeerDiscovered {
        peer_id: PeerId::new(),
        device_name: Some("Laptop".to_string()),
    };
    let dto: SyncEventDto = event.into();
    assert_eq!(dto.event_type, "peer_discovered");
    assert!(dto.peer_id.is_some());
    assert_eq!(dto.device_name.as_deref(), Some("Laptop"));
}

#[test]
fn sync_event_dto_sync_started() {
    let dto: SyncEventDto = SyncEvent::SyncStarted { peer_id: PeerId::new() }.into();
    assert_eq!(dto.event_type, "sync_started");
}

#[test]
fn sync_event_dto_sync_completed() {
    let dto: SyncEventDto = SyncEvent::SyncCompleted {
        peer_id: PeerId::new(),
        events_sent: 10,
        events_received: 5,
    }.into();
    assert_eq!(dto.event_type, "sync_completed");
    assert_eq!(dto.events_sent, Some(10));
    assert_eq!(dto.events_received, Some(5));
}

#[test]
fn sync_event_dto_sync_failed() {
    let dto: SyncEventDto = SyncEvent::SyncFailed {
        peer_id: PeerId::new(),
        error: "timeout".to_string(),
    }.into();
    assert_eq!(dto.event_type, "sync_failed");
    assert_eq!(dto.error.as_deref(), Some("timeout"));
}

#[test]
fn sync_event_dto_entity_updated() {
    let dto: SyncEventDto = SyncEvent::EntityUpdated {
        entity_id: privstack_types::EntityId::new(),
    }.into();
    assert_eq!(dto.event_type, "entity_updated");
    assert!(dto.entity_id.is_some());
}

// ── Execute / Search null pointer ───────────────────────────

#[test]
fn execute_null() {
    let result = unsafe { privstack_execute(ptr::null()) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("null_pointer"));
    unsafe { privstack_free_string(result) };
}

#[test]
fn search_null() {
    let result = unsafe { privstack_search(ptr::null()) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("null_pointer"));
    unsafe { privstack_free_string(result) };
}

// ── Full lifecycle: pairing ─────────────────────────────────

#[test]
#[serial]
fn pairing_generate_and_get_code() {
    test_init();

    let mut out_code: *mut c_char = ptr::null_mut();
    let result = unsafe { privstack_pairing_generate_code(&mut out_code) };
    assert_eq!(result, PrivStackError::Ok);
    assert!(!out_code.is_null());

    let code_json = unsafe { CStr::from_ptr(out_code) }.to_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(code_json).expect("should be valid JSON");
    assert!(parsed.get("code").is_some(), "JSON should contain 'code' field");
    assert!(parsed.get("hash").is_some(), "JSON should contain 'hash' field");
    unsafe { privstack_free_string(out_code) };

    // Get the code back
    let mut out_code2: *mut c_char = ptr::null_mut();
    let result = unsafe { privstack_pairing_get_code(&mut out_code2) };
    assert_eq!(result, PrivStackError::Ok);
    assert!(!out_code2.is_null());

    let get_json = unsafe { CStr::from_ptr(out_code2) }.to_str().unwrap();
    let parsed2: serde_json::Value = serde_json::from_str(get_json).expect("should be valid JSON");
    assert!(parsed2.get("code").is_some());
    unsafe { privstack_free_string(out_code2) };

    privstack_shutdown();
}

#[test]
#[serial]
fn pairing_list_peers_empty() {
    test_init();

    let mut out_json: *mut c_char = ptr::null_mut();
    let result = unsafe { privstack_pairing_list_peers(&mut out_json) };
    assert_eq!(result, PrivStackError::Ok);
    assert!(!out_json.is_null());

    let json = unsafe { CStr::from_ptr(out_json) }.to_str().unwrap();
    assert!(json.contains("[]") || json.contains("{}"));
    unsafe { privstack_free_string(out_json) };

    privstack_shutdown();
}

#[test]
#[serial]
fn pairing_save_load_state() {
    test_init();

    // Save state
    let mut out_json: *mut c_char = ptr::null_mut();
    let result = unsafe { privstack_pairing_save_state(&mut out_json) };
    assert_eq!(result, PrivStackError::Ok);
    assert!(!out_json.is_null());

    let state_json = unsafe { CStr::from_ptr(out_json) }.to_str().unwrap().to_string();
    unsafe { privstack_free_string(out_json) };

    // Load it back
    let c_json = CString::new(state_json).unwrap();
    let result = unsafe { privstack_pairing_load_state(c_json.as_ptr()) };
    assert_eq!(result, PrivStackError::Ok);

    privstack_shutdown();
}

#[test]
#[serial]
fn device_name_get_set() {
    test_init();

    let name = CString::new("TestDevice").unwrap();
    let result = unsafe { privstack_pairing_set_device_name(name.as_ptr()) };
    assert_eq!(result, PrivStackError::Ok);

    let mut out_name: *mut c_char = ptr::null_mut();
    let result = unsafe { privstack_pairing_get_device_name(&mut out_name) };
    assert_eq!(result, PrivStackError::Ok);

    let got = unsafe { CStr::from_ptr(out_name) }.to_str().unwrap();
    assert_eq!(got, "TestDevice");
    unsafe { privstack_free_string(out_name) };

    privstack_shutdown();
}

// ── Full lifecycle: cloud init ──────────────────────────────

#[test]
#[serial]
fn cloud_init_google_drive_lifecycle() {
    test_init();

    let client_id = CString::new("test_id").unwrap();
    let client_secret = CString::new("test_secret").unwrap();
    let result = unsafe { privstack_cloud_init_google_drive(client_id.as_ptr(), client_secret.as_ptr()) };
    assert_eq!(result, PrivStackError::Ok);

    assert!(!privstack_cloud_is_authenticated(CloudProvider::GoogleDrive));

    privstack_shutdown();
}

#[test]
#[serial]
fn cloud_init_icloud_default() {
    test_init();

    let result = unsafe { privstack_cloud_init_icloud(ptr::null()) };
    assert_eq!(result, PrivStackError::Ok);

    privstack_shutdown();
}

#[test]
#[serial]
fn cloud_init_icloud_custom_bundle() {
    test_init();

    let bundle = CString::new("com.test.custom").unwrap();
    let result = unsafe { privstack_cloud_init_icloud(bundle.as_ptr()) };
    assert_eq!(result, PrivStackError::Ok);

    privstack_shutdown();
}

// ── Full lifecycle: device info ─────────────────────────────

#[test]
fn device_info_returns_json() {
    let mut out_json: *mut c_char = ptr::null_mut();
    let result = unsafe { privstack_device_info(&mut out_json) };
    assert_eq!(result, PrivStackError::Ok);
    assert!(!out_json.is_null());

    let json = unsafe { CStr::from_ptr(out_json) }.to_str().unwrap();
    assert!(json.contains("os_name"));
    assert!(json.contains("fingerprint"));
    unsafe { privstack_free_string(out_json) };
}

#[test]
fn device_fingerprint_returns_string() {
    let mut out_fp: *mut c_char = ptr::null_mut();
    let result = unsafe { privstack_device_fingerprint(&mut out_fp) };
    assert_eq!(result, PrivStackError::Ok);
    assert!(!out_fp.is_null());

    let fp = unsafe { CStr::from_ptr(out_fp) }.to_str().unwrap();
    assert!(!fp.is_empty());
    unsafe { privstack_free_string(out_fp) };
}

// ── Full lifecycle: unencrypted blobs ───────────────────────

#[test]
#[serial]
fn blob_store_read_delete_lifecycle() {
    test_init();

    let ns = CString::new("test_ns").unwrap();
    let bid = CString::new("blob1").unwrap();
    let data = b"blob content";

    let result = unsafe {
        privstack_blob_store(ns.as_ptr(), bid.as_ptr(), data.as_ptr(), data.len(), ptr::null())
    };
    assert_eq!(result, PrivStackError::Ok);

    // Read it back
    let mut out_data: *mut u8 = ptr::null_mut();
    let mut out_len: usize = 0;
    let result = unsafe {
        privstack_blob_read(ns.as_ptr(), bid.as_ptr(), &mut out_data, &mut out_len)
    };
    assert_eq!(result, PrivStackError::Ok);
    assert_eq!(out_len, data.len());
    let read = unsafe { std::slice::from_raw_parts(out_data, out_len) };
    assert_eq!(read, data);
    unsafe { privstack_free_bytes(out_data, out_len) };

    // List blobs
    let mut out_json: *mut c_char = ptr::null_mut();
    let result = unsafe { privstack_blob_list(ns.as_ptr(), &mut out_json) };
    assert_eq!(result, PrivStackError::Ok);
    assert!(!out_json.is_null());
    unsafe { privstack_free_string(out_json) };

    // Delete
    let result = unsafe { privstack_blob_delete(ns.as_ptr(), bid.as_ptr()) };
    assert_eq!(result, PrivStackError::Ok);

    privstack_shutdown();
}

#[test]
#[serial]
fn blob_store_with_metadata() {
    test_init();

    let ns = CString::new("ns").unwrap();
    let bid = CString::new("b1").unwrap();
    let data = b"data";
    let meta = CString::new(r#"{"key":"value"}"#).unwrap();

    let result = unsafe {
        privstack_blob_store(ns.as_ptr(), bid.as_ptr(), data.as_ptr(), data.len(), meta.as_ptr())
    };
    assert_eq!(result, PrivStackError::Ok);

    privstack_shutdown();
}

// ── Full lifecycle: vault blob list ─────────────────────────

#[test]
#[serial]
fn vault_blob_list_lifecycle() {
    test_init();

    let vid = CString::new("list_vault").unwrap();
    let pwd = CString::new("password123").unwrap();
    unsafe {
        privstack_vault_create(vid.as_ptr());
        privstack_vault_initialize(vid.as_ptr(), pwd.as_ptr());
    }

    let bid = CString::new("b1").unwrap();
    let data = b"test";
    unsafe {
        privstack_vault_blob_store(vid.as_ptr(), bid.as_ptr(), data.as_ptr(), data.len());
    }

    let mut out_json: *mut c_char = ptr::null_mut();
    let result = unsafe { privstack_vault_blob_list(vid.as_ptr(), &mut out_json) };
    assert_eq!(result, PrivStackError::Ok);
    assert!(!out_json.is_null());

    let json = unsafe { CStr::from_ptr(out_json) }.to_str().unwrap();
    assert!(json.contains("b1"));
    unsafe { privstack_free_string(out_json) };

    privstack_shutdown();
}

// ── Execute endpoint ────────────────────────────────────────

#[test]
#[serial]
fn execute_invalid_json() {
    test_init();

    let bad_json = CString::new("not json").unwrap();
    let result = unsafe { privstack_execute(bad_json.as_ptr()) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("json_parse_error"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn execute_unknown_entity_type() {
    test_init();

    let req = CString::new(r#"{"plugin_id":"test","action":"read","entity_type":"nonexistent","entity_id":"123"}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("unknown_entity"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn search_invalid_json() {
    test_init();

    let bad = CString::new("not json").unwrap();
    let result = unsafe { privstack_search(bad.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("json_parse_error"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn search_valid_query() {
    test_init();

    let query = CString::new(r#"{"query":"test","limit":10}"#).unwrap();
    let result = unsafe { privstack_search(query.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("success"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn register_entity_type_valid() {
    test_init();

    let schema_json = CString::new(r#"{"entity_type":"test_note","indexed_fields":[{"field_path":"/title","field_type":"text","searchable":true}],"merge_strategy":"lww_document"}"#).unwrap();
    let result = unsafe { privstack_register_entity_type(schema_json.as_ptr()) };
    assert_eq!(result, 0);

    privstack_shutdown();
}

#[test]
#[serial]
fn register_entity_type_invalid_json() {
    test_init();

    let bad = CString::new("not json").unwrap();
    let result = unsafe { privstack_register_entity_type(bad.as_ptr()) };
    assert_eq!(result, -3);

    privstack_shutdown();
}

// ── Sync status checks ──────────────────────────────────────

#[test]
#[serial]
fn sync_is_running_after_init() {
    test_init();

    assert!(!privstack_sync_is_running());

    privstack_shutdown();
}

// ── PrivStackError enum ─────────────────────────────────────

#[test]
fn error_enum_values() {
    assert_eq!(PrivStackError::Ok as i32, 0);
    assert_eq!(PrivStackError::NullPointer as i32, 1);
    assert_eq!(PrivStackError::InvalidUtf8 as i32, 2);
    assert_eq!(PrivStackError::JsonError as i32, 3);
    assert_eq!(PrivStackError::StorageError as i32, 4);
    assert_eq!(PrivStackError::NotFound as i32, 5);
    assert_eq!(PrivStackError::NotInitialized as i32, 6);
    assert_eq!(PrivStackError::Unknown as i32, 99);
}

#[test]
fn error_enum_debug() {
    let err = PrivStackError::NullPointer;
    assert_eq!(format!("{:?}", err), "NullPointer");
}

#[test]
fn error_enum_clone_eq() {
    let err = PrivStackError::StorageError;
    let cloned = err;
    assert_eq!(err, cloned);
}

// ── CloudProvider enum ──────────────────────────────────────

#[test]
fn cloud_provider_values() {
    assert_eq!(CloudProvider::GoogleDrive as i32, 0);
    assert_eq!(CloudProvider::ICloud as i32, 1);
}

// ── FfiLicensePlan / FfiLicenseStatus ───────────────────────

#[test]
fn ffi_license_plan_from() {
    assert_eq!(FfiLicensePlan::from(LicensePlan::Trial), FfiLicensePlan::Trial);
    assert_eq!(FfiLicensePlan::from(LicensePlan::Monthly), FfiLicensePlan::Monthly);
    assert_eq!(FfiLicensePlan::from(LicensePlan::Annual), FfiLicensePlan::Annual);
    assert_eq!(FfiLicensePlan::from(LicensePlan::Perpetual), FfiLicensePlan::Perpetual);
}

#[test]
fn ffi_license_status_from() {
    assert_eq!(FfiLicenseStatus::from(LicenseStatus::Active), FfiLicenseStatus::Active);
    assert_eq!(FfiLicenseStatus::from(LicenseStatus::Expired), FfiLicenseStatus::Expired);
    assert_eq!(FfiLicenseStatus::from(LicenseStatus::Grace { days_remaining: 14 }), FfiLicenseStatus::Grace);
    assert_eq!(FfiLicenseStatus::from(LicenseStatus::ReadOnly), FfiLicenseStatus::ReadOnly);
    assert_eq!(FfiLicenseStatus::from(LicenseStatus::NotActivated), FfiLicenseStatus::NotActivated);
}

// ── license_error_to_ffi mapping ────────────────────────────

#[test]
fn license_error_mapping() {
    assert_eq!(license_error_to_ffi(LicenseError::InvalidKeyFormat("bad".into())), PrivStackError::LicenseInvalidFormat);
    assert_eq!(license_error_to_ffi(LicenseError::InvalidSignature), PrivStackError::LicenseInvalidSignature);
    assert_eq!(license_error_to_ffi(LicenseError::InvalidPayload("bad".into())), PrivStackError::LicenseInvalidFormat);
    assert_eq!(license_error_to_ffi(LicenseError::NotActivated), PrivStackError::LicenseNotActivated);
    assert_eq!(license_error_to_ffi(LicenseError::Revoked), PrivStackError::LicenseExpired);
}

// ── Vault lock/unlock lifecycle via FFI ─────────────────────

#[test]
#[serial]
fn vault_full_lifecycle() {
    test_init();

    let vid = CString::new("lifecycle_vault").unwrap();
    let pwd = CString::new("password123").unwrap();

    // Create
    let r = unsafe { privstack_vault_create(vid.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);

    // Not yet initialized
    assert!(!unsafe { privstack_vault_is_initialized(vid.as_ptr()) });

    // Initialize
    let r = unsafe { privstack_vault_initialize(vid.as_ptr(), pwd.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);
    assert!(unsafe { privstack_vault_is_initialized(vid.as_ptr()) });
    assert!(unsafe { privstack_vault_is_unlocked(vid.as_ptr()) });

    // Lock
    let r = unsafe { privstack_vault_lock(vid.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);
    assert!(!unsafe { privstack_vault_is_unlocked(vid.as_ptr()) });

    // Unlock
    let r = unsafe { privstack_vault_unlock(vid.as_ptr(), pwd.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);
    assert!(unsafe { privstack_vault_is_unlocked(vid.as_ptr()) });

    // Change password
    let new_pwd = CString::new("newpassword1").unwrap();
    let r = unsafe { privstack_vault_change_password(vid.as_ptr(), pwd.as_ptr(), new_pwd.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);

    // Lock all
    let r = privstack_vault_lock_all();
    assert_eq!(r, PrivStackError::Ok);
    assert!(!unsafe { privstack_vault_is_unlocked(vid.as_ptr()) });

    privstack_shutdown();
}

// ── Vault ID with dots (e.g. "privstack.files") ───────────

#[test]
#[serial]
fn vault_dotted_id_lifecycle() {
    test_init();

    let vid = CString::new("privstack.files").unwrap();
    let pwd = CString::new("password123").unwrap();

    // Create with dotted ID
    let r = unsafe { privstack_vault_create(vid.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);

    // Not yet initialized
    assert!(!unsafe { privstack_vault_is_initialized(vid.as_ptr()) });

    // Initialize
    let r = unsafe { privstack_vault_initialize(vid.as_ptr(), pwd.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);
    assert!(unsafe { privstack_vault_is_initialized(vid.as_ptr()) });
    assert!(unsafe { privstack_vault_is_unlocked(vid.as_ptr()) });

    // Store and read a blob
    let bid = CString::new("test_blob").unwrap();
    let data = b"hello vault with dots";
    let r = unsafe {
        privstack_vault_blob_store(vid.as_ptr(), bid.as_ptr(), data.as_ptr(), data.len())
    };
    assert_eq!(r, PrivStackError::Ok);

    let mut out_data: *mut u8 = ptr::null_mut();
    let mut out_len: usize = 0;
    let r = unsafe {
        privstack_vault_blob_read(vid.as_ptr(), bid.as_ptr(), &mut out_data, &mut out_len)
    };
    assert_eq!(r, PrivStackError::Ok);
    assert_eq!(out_len, data.len());

    let read_data = unsafe { std::slice::from_raw_parts(out_data, out_len) };
    assert_eq!(read_data, data);
    unsafe { privstack_free_bytes(out_data, out_len) };

    // Lock and unlock
    let r = unsafe { privstack_vault_lock(vid.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);
    let r = unsafe { privstack_vault_unlock(vid.as_ptr(), pwd.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);

    privstack_shutdown();
}

// ── Locked vault blob operations ────────────────────────────

#[test]
#[serial]
fn vault_blob_locked_returns_error() {
    test_init();

    let vid = CString::new("locked_vault").unwrap();
    let pwd = CString::new("password123").unwrap();
    unsafe {
        privstack_vault_create(vid.as_ptr());
        privstack_vault_initialize(vid.as_ptr(), pwd.as_ptr());
        privstack_vault_lock(vid.as_ptr());
    }

    let bid = CString::new("b1").unwrap();
    let data = b"test";
    let result = unsafe {
        privstack_vault_blob_store(vid.as_ptr(), bid.as_ptr(), data.as_ptr(), data.len())
    };
    assert_eq!(result, PrivStackError::VaultLocked);

    let mut out_data: *mut u8 = ptr::null_mut();
    let mut out_len: usize = 0;
    let result = unsafe {
        privstack_vault_blob_read(vid.as_ptr(), bid.as_ptr(), &mut out_data, &mut out_len)
    };
    assert_eq!(result, PrivStackError::VaultLocked);

    privstack_shutdown();
}

// ── Blob not found ──────────────────────────────────────────

#[test]
#[serial]
fn vault_blob_read_not_found() {
    test_init();

    let vid = CString::new("nf_vault").unwrap();
    let pwd = CString::new("password123").unwrap();
    unsafe {
        privstack_vault_create(vid.as_ptr());
        privstack_vault_initialize(vid.as_ptr(), pwd.as_ptr());
    }

    let bid = CString::new("nonexistent").unwrap();
    let mut out_data: *mut u8 = ptr::null_mut();
    let mut out_len: usize = 0;
    let result = unsafe {
        privstack_vault_blob_read(vid.as_ptr(), bid.as_ptr(), &mut out_data, &mut out_len)
    };
    assert_eq!(result, PrivStackError::NotFound);

    let result = unsafe { privstack_vault_blob_delete(vid.as_ptr(), bid.as_ptr()) };
    assert_eq!(result, PrivStackError::NotFound);

    privstack_shutdown();
}

// ── Cloud provider name ─────────────────────────────────────

#[test]
#[serial]
fn cloud_provider_name_google() {
    test_init();

    let cid = CString::new("id").unwrap();
    let csec = CString::new("sec").unwrap();
    unsafe { privstack_cloud_init_google_drive(cid.as_ptr(), csec.as_ptr()) };

    let name = privstack_cloud_provider_name(CloudProvider::GoogleDrive);
    assert!(!name.is_null());
    let name_str = unsafe { CStr::from_ptr(name) }.to_str().unwrap();
    assert_eq!(name_str, "Google Drive");

    privstack_shutdown();
}

#[test]
#[serial]
fn cloud_provider_name_icloud() {
    test_init();

    unsafe { privstack_cloud_init_icloud(ptr::null()) };

    let name = privstack_cloud_provider_name(CloudProvider::ICloud);
    assert!(!name.is_null());
    let name_str = unsafe { CStr::from_ptr(name) }.to_str().unwrap();
    assert_eq!(name_str, "iCloud Drive");

    privstack_shutdown();
}

// ── Sync poll event when no events ──────────────────────────

#[test]
#[serial]
fn sync_poll_event_not_running() {
    test_init();

    let mut out_json: *mut c_char = ptr::null_mut();
    let result = unsafe { privstack_sync_poll_event(&mut out_json) };
    assert_eq!(result, PrivStackError::SyncNotRunning);

    privstack_shutdown();
}

// ── Sync status ─────────────────────────────────────────────

#[test]
#[serial]
fn sync_status_when_not_running() {
    test_init();

    let mut out_json: *mut c_char = ptr::null_mut();
    let result = unsafe { privstack_sync_status(&mut out_json) };
    assert_eq!(result, PrivStackError::Ok);
    assert!(!out_json.is_null());

    let json = unsafe { CStr::from_ptr(out_json) }.to_str().unwrap();
    assert!(json.contains("\"running\":false"));
    assert!(json.contains("local_peer_id"));
    assert!(json.contains("discovered_peers"));
    unsafe { privstack_free_string(out_json) };

    privstack_shutdown();
}

#[test]
fn sync_status_null() {
    let result = unsafe { privstack_sync_status(ptr::null_mut()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

// ── Sync trigger ────────────────────────────────────────────

#[test]
#[serial]
fn sync_trigger_not_running() {
    test_init();

    let result = privstack_sync_trigger();
    assert_eq!(result, PrivStackError::SyncNotRunning);

    privstack_shutdown();
}

// ── Sync publish event ──────────────────────────────────────

#[test]
fn sync_publish_event_null() {
    let result = unsafe { privstack_sync_publish_event(ptr::null()) };
    assert_eq!(result, PrivStackError::NullPointer);
}

#[test]
#[serial]
fn sync_publish_event_invalid_json() {
    test_init();

    let bad = CString::new("not json").unwrap();
    let result = unsafe { privstack_sync_publish_event(bad.as_ptr()) };
    assert_eq!(result, PrivStackError::JsonError);

    privstack_shutdown();
}

#[test]
#[serial]
fn sync_publish_event_no_orchestrator() {
    test_init();

    // The exact Event serialization format is complex; invalid JSON gives JsonError,
    // and even valid JSON without an orchestrator gives SyncNotRunning.
    // Test the invalid JSON path here (SyncNotRunning tested via sync_trigger).
    let event_json = CString::new(r#"{"invalid":"event"}"#).unwrap();
    let result = unsafe { privstack_sync_publish_event(event_json.as_ptr()) };
    assert_eq!(result, PrivStackError::JsonError);

    privstack_shutdown();
}

// ── Execute CRUD lifecycle ──────────────────────────────────

#[test]
#[serial]
fn execute_create_read_update_delete() {
    test_init();

    // Register entity type
    let schema = CString::new(r#"{"entity_type":"test_item","indexed_fields":[{"field_path":"/title","field_type":"text","searchable":true}],"merge_strategy":"lww_document"}"#).unwrap();
    let r = unsafe { privstack_register_entity_type(schema.as_ptr()) };
    assert_eq!(r, 0);

    // Create
    let create_req = CString::new(r#"{"plugin_id":"test","action":"create","entity_type":"test_item","payload":"{\"title\":\"Hello World\"}"}"#).unwrap();
    let result = unsafe { privstack_execute(create_req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap().to_string();
    assert!(json.contains("\"success\":true"), "Create failed: {json}");
    assert!(json.contains("Hello World"));

    // Extract entity ID
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let entity_id = parsed["data"]["id"].as_str().unwrap().to_string();
    unsafe { privstack_free_string(result) };

    // Read
    let read_req = CString::new(format!(
        r#"{{"plugin_id":"test","action":"read","entity_type":"test_item","entity_id":"{}"}}"#,
        entity_id
    )).unwrap();
    let result = unsafe { privstack_execute(read_req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("\"success\":true"), "Read failed: {json}");
    assert!(json.contains("Hello World"));
    unsafe { privstack_free_string(result) };

    // Update
    let update_req = CString::new(format!(
        r#"{{"plugin_id":"test","action":"update","entity_type":"test_item","entity_id":"{}","payload":"{{\"title\":\"Updated Title\"}}"}}"#,
        entity_id
    )).unwrap();
    let result = unsafe { privstack_execute(update_req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("\"success\":true"), "Update failed: {json}");
    assert!(json.contains("Updated Title"));
    unsafe { privstack_free_string(result) };

    // Read List
    let list_req = CString::new(r#"{"plugin_id":"test","action":"read_list","entity_type":"test_item"}"#).unwrap();
    let result = unsafe { privstack_execute(list_req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("\"success\":true"), "List failed: {json}");
    assert!(json.contains("Updated Title"));
    unsafe { privstack_free_string(result) };

    // Trash
    let trash_req = CString::new(format!(
        r#"{{"plugin_id":"test","action":"trash","entity_type":"test_item","entity_id":"{}"}}"#,
        entity_id
    )).unwrap();
    let result = unsafe { privstack_execute(trash_req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("\"success\":true"), "Trash failed: {json}");
    unsafe { privstack_free_string(result) };

    // Restore
    let restore_req = CString::new(format!(
        r#"{{"plugin_id":"test","action":"restore","entity_type":"test_item","entity_id":"{}"}}"#,
        entity_id
    )).unwrap();
    let result = unsafe { privstack_execute(restore_req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("\"success\":true"), "Restore failed: {json}");
    unsafe { privstack_free_string(result) };

    // Delete
    let del_req = CString::new(format!(
        r#"{{"plugin_id":"test","action":"delete","entity_type":"test_item","entity_id":"{}"}}"#,
        entity_id
    )).unwrap();
    let result = unsafe { privstack_execute(del_req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("\"success\":true"), "Delete failed: {json}");
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn execute_read_missing_id() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"err_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    let req = CString::new(r#"{"plugin_id":"test","action":"read","entity_type":"err_item"}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("missing_id"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn execute_read_not_found() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"nf_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    let req = CString::new(r#"{"plugin_id":"test","action":"read","entity_type":"nf_item","entity_id":"nonexistent"}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("not_found"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn execute_create_missing_payload() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"mp_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    let req = CString::new(r#"{"plugin_id":"test","action":"create","entity_type":"mp_item"}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("missing_payload"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn execute_unknown_action() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"ua_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    let req = CString::new(r#"{"plugin_id":"test","action":"purge","entity_type":"ua_item"}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("unknown_action"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn execute_delete_missing_id() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"dm_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    let req = CString::new(r#"{"plugin_id":"test","action":"delete","entity_type":"dm_item"}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("missing_id"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn execute_trash_missing_id() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"tm_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    let req = CString::new(r#"{"plugin_id":"test","action":"trash","entity_type":"tm_item"}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("missing_id"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn execute_restore_missing_id() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"rm_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    let req = CString::new(r#"{"plugin_id":"test","action":"restore","entity_type":"rm_item"}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("missing_id"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn execute_query() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"q_item","indexed_fields":[{"field_path":"/title","field_type":"text","searchable":true}],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    // Create item
    let create = CString::new(r#"{"plugin_id":"test","action":"create","entity_type":"q_item","payload":"{\"title\":\"Queryable\"}"}"#).unwrap();
    let r = unsafe { privstack_execute(create.as_ptr()) };
    unsafe { privstack_free_string(r) };

    // Query
    let query = CString::new(r#"{"plugin_id":"test","action":"query","entity_type":"q_item","payload":"[]"}"#).unwrap();
    let result = unsafe { privstack_execute(query.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("\"success\":true"), "Query failed: {json}");
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn execute_link_unlink_get_links() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"link_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    // Create two items
    let create1 = CString::new(r#"{"plugin_id":"test","action":"create","entity_type":"link_item","entity_id":"item1","payload":"{\"title\":\"A\"}"}"#).unwrap();
    let r = unsafe { privstack_execute(create1.as_ptr()) };
    unsafe { privstack_free_string(r) };

    let create2 = CString::new(r#"{"plugin_id":"test","action":"create","entity_type":"link_item","entity_id":"item2","payload":"{\"title\":\"B\"}"}"#).unwrap();
    let r = unsafe { privstack_execute(create2.as_ptr()) };
    unsafe { privstack_free_string(r) };

    // Link
    let link = CString::new(r#"{"plugin_id":"test","action":"link","entity_type":"link_item","entity_id":"item1","parameters":{"target_type":"link_item","target_id":"item2"}}"#).unwrap();
    let result = unsafe { privstack_execute(link.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("\"success\":true"), "Link failed: {json}");
    unsafe { privstack_free_string(result) };

    // Get links
    let get_links = CString::new(r#"{"plugin_id":"test","action":"get_links","entity_type":"link_item","entity_id":"item1"}"#).unwrap();
    let result = unsafe { privstack_execute(get_links.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("\"success\":true"), "Get links failed: {json}");
    assert!(json.contains("item2"));
    unsafe { privstack_free_string(result) };

    // Unlink
    let unlink = CString::new(r#"{"plugin_id":"test","action":"unlink","entity_type":"link_item","entity_id":"item1","parameters":{"target_type":"link_item","target_id":"item2"}}"#).unwrap();
    let result = unsafe { privstack_execute(unlink.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("\"success\":true"), "Unlink failed: {json}");
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn execute_link_missing_params() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"lp_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    let req = CString::new(r#"{"plugin_id":"test","action":"link","entity_type":"lp_item","entity_id":"x"}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("missing_params"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn execute_link_missing_id() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"li_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    let req = CString::new(r#"{"plugin_id":"test","action":"link","entity_type":"li_item","parameters":{"target_type":"li_item","target_id":"y"}}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("missing_id"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn execute_unlink_missing_id() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"ui_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    let req = CString::new(r#"{"plugin_id":"test","action":"unlink","entity_type":"ui_item","parameters":{"target_type":"x","target_id":"y"}}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("missing_id"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn execute_get_links_missing_id() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"gl_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    let req = CString::new(r#"{"plugin_id":"test","action":"get_links","entity_type":"gl_item"}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("missing_id"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn execute_read_list_with_params() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"rl_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    // Create items
    for i in 0..5 {
        let req = CString::new(format!(
            r#"{{"plugin_id":"test","action":"create","entity_type":"rl_item","payload":"{{\"n\":{i}}}"}}"#
        )).unwrap();
        let r = unsafe { privstack_execute(req.as_ptr()) };
        unsafe { privstack_free_string(r) };
    }

    // List with limit and offset
    let req = CString::new(r#"{"plugin_id":"test","action":"read_list","entity_type":"rl_item","parameters":{"limit":"2","offset":"1"}}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("\"success\":true"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn execute_create_invalid_payload_json() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"bad_json_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    let req = CString::new(r#"{"plugin_id":"test","action":"create","entity_type":"bad_json_item","payload":"not valid json"}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("json_error"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

// ── Search with entity types filter ─────────────────────────

#[test]
#[serial]
fn search_with_entity_types() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"search_item","indexed_fields":[{"field_path":"/title","field_type":"text","searchable":true}],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    // Create item with searchable content
    let create = CString::new(r#"{"plugin_id":"test","action":"create","entity_type":"search_item","payload":"{\"title\":\"UniqueSearchableTerm\"}"}"#).unwrap();
    let r = unsafe { privstack_execute(create.as_ptr()) };
    unsafe { privstack_free_string(r) };

    // Search with entity type filter
    let query = CString::new(r#"{"query":"UniqueSearchableTerm","entity_types":["search_item"],"limit":5}"#).unwrap();
    let result = unsafe { privstack_search(query.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("\"success\":true"), "Search failed: {json}");
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

// ── Auth change password ────────────────────────────────────

#[test]
#[serial]
fn auth_change_password_lifecycle() {
    test_init();

    let pwd = CString::new("password123").unwrap();
    unsafe { privstack_auth_initialize(pwd.as_ptr()) };
    assert!(privstack_auth_is_unlocked());

    let new_pwd = CString::new("newpassword1").unwrap();
    let r = unsafe { privstack_auth_change_password(pwd.as_ptr(), new_pwd.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);

    // Lock and unlock with new password
    privstack_auth_lock();
    let r = unsafe { privstack_auth_unlock(new_pwd.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);
    assert!(privstack_auth_is_unlocked());

    privstack_shutdown();
}

#[test]
#[serial]
fn auth_change_password_too_short() {
    test_init();

    let pwd = CString::new("password123").unwrap();
    unsafe { privstack_auth_initialize(pwd.as_ptr()) };

    let short = CString::new("short").unwrap();
    let r = unsafe { privstack_auth_change_password(pwd.as_ptr(), short.as_ptr()) };
    assert_eq!(r, PrivStackError::PasswordTooShort);

    privstack_shutdown();
}

// ── iCloud through FFI ──────────────────────────────────────

#[test]
#[serial]
fn icloud_authenticate_via_ffi() {
    test_init();

    unsafe { privstack_cloud_init_icloud(ptr::null()) };

    let mut out_auth_url: *mut c_char = ptr::null_mut();
    let result = unsafe { privstack_cloud_authenticate(CloudProvider::ICloud, &mut out_auth_url) };
    // iCloud auth doesn't need URL — but may fail since container doesn't exist
    // Either way, we exercise the code path
    let _ = result;
    if !out_auth_url.is_null() {
        unsafe { privstack_free_string(out_auth_url) };
    }

    // complete_auth is a no-op for iCloud
    let code = CString::new("anything").unwrap();
    let result = unsafe { privstack_cloud_complete_auth(CloudProvider::ICloud, code.as_ptr()) };
    let _ = result;

    // is_authenticated
    let _ = privstack_cloud_is_authenticated(CloudProvider::ICloud);

    privstack_shutdown();
}

// ── Google Drive authenticate via FFI ────────────────────────

#[test]
#[serial]
fn google_drive_authenticate_via_ffi() {
    test_init();

    let cid = CString::new("test_client_id").unwrap();
    let csec = CString::new("test_client_secret").unwrap();
    unsafe { privstack_cloud_init_google_drive(cid.as_ptr(), csec.as_ptr()) };

    let mut out_auth_url: *mut c_char = ptr::null_mut();
    let result = unsafe { privstack_cloud_authenticate(CloudProvider::GoogleDrive, &mut out_auth_url) };
    assert_eq!(result, PrivStackError::Ok);
    assert!(!out_auth_url.is_null());

    let url = unsafe { CStr::from_ptr(out_auth_url) }.to_str().unwrap();
    assert!(url.contains("test_client_id"));
    assert!(url.contains("accounts.google.com"));
    unsafe { privstack_free_string(out_auth_url) };

    privstack_shutdown();
}

// ── Cloud not initialized ───────────────────────────────────

#[test]
#[serial]
fn cloud_operations_without_provider_init() {
    test_init();

    // Try operations without initializing provider
    let mut out_auth_url: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_cloud_authenticate(CloudProvider::GoogleDrive, &mut out_auth_url) };
    assert_eq!(r, PrivStackError::NotInitialized);

    let code = CString::new("code").unwrap();
    let r = unsafe { privstack_cloud_complete_auth(CloudProvider::GoogleDrive, code.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);

    let mut out_json: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_cloud_list_files(CloudProvider::GoogleDrive, &mut out_json) };
    assert_eq!(r, PrivStackError::NotInitialized);

    let name = CString::new("f.txt").unwrap();
    let data = b"x";
    let r = unsafe { privstack_cloud_upload(CloudProvider::GoogleDrive, name.as_ptr(), data.as_ptr(), data.len(), &mut out_json) };
    assert_eq!(r, PrivStackError::NotInitialized);

    let fid = CString::new("fid").unwrap();
    let mut out_data: *mut u8 = ptr::null_mut();
    let mut out_len: usize = 0;
    let r = unsafe { privstack_cloud_download(CloudProvider::GoogleDrive, fid.as_ptr(), &mut out_data, &mut out_len) };
    assert_eq!(r, PrivStackError::NotInitialized);

    let r = unsafe { privstack_cloud_delete(CloudProvider::GoogleDrive, fid.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);

    privstack_shutdown();
}

// ── License parse with valid key ────────────────────────────

#[test]
#[serial]
fn license_parse_invalid_key() {
    test_init();

    let key = CString::new("invalid-key-format").unwrap();
    let mut out_json: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_license_parse(key.as_ptr(), &mut out_json) };
    assert_ne!(r, PrivStackError::Ok);

    privstack_shutdown();
}

#[test]
#[serial]
fn license_get_plan_invalid_key() {
    test_init();

    let key = CString::new("bad-key").unwrap();
    let mut out_plan = FfiLicensePlan::Monthly;
    let r = unsafe { privstack_license_get_plan(key.as_ptr(), &mut out_plan) };
    assert_ne!(r, PrivStackError::Ok);

    privstack_shutdown();
}

// ── License not activated checks ────────────────────────────

#[test]
#[serial]
fn license_check_not_activated() {
    test_init();
    test_clear_activation();

    let mut out_json: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_license_check(&mut out_json) };
    assert_eq!(r, PrivStackError::LicenseNotActivated);

    privstack_shutdown();
}

#[test]
#[serial]
fn license_status_not_activated() {
    test_init();
    test_clear_activation();

    let mut out_status = FfiLicenseStatus::Active;
    let r = unsafe { privstack_license_status(&mut out_status) };
    assert_eq!(r, PrivStackError::Ok);
    assert_eq!(out_status, FfiLicenseStatus::NotActivated);

    privstack_shutdown();
}

#[test]
#[serial]
fn license_activated_plan_not_activated() {
    test_init();
    test_clear_activation();

    let mut out_plan = FfiLicensePlan::Perpetual;
    let r = unsafe { privstack_license_activated_plan(&mut out_plan) };
    assert_eq!(r, PrivStackError::LicenseNotActivated);

    privstack_shutdown();
}

#[test]
#[serial]
fn license_deactivate_when_none() {
    test_init();
    test_clear_activation();

    let r = privstack_license_deactivate();
    assert_eq!(r, PrivStackError::Ok);

    privstack_shutdown();
}

// ── Pairing: trust and remove peer ──────────────────────────

#[test]
#[serial]
fn pairing_trust_and_remove_peer() {
    test_init();

    let peer_id = CString::new("test-peer-id").unwrap();
    let device_name = CString::new("TestDevice").unwrap();

    // approve_peer only moves discovered peers to trusted; calling trust_peer
    // on an undiscovered peer is a no-op but exercises the FFI code path.
    let r = unsafe { privstack_pairing_trust_peer(peer_id.as_ptr(), device_name.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);

    // List peers (may be empty since peer wasn't discovered first)
    let mut out_json: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_pairing_list_peers(&mut out_json) };
    assert_eq!(r, PrivStackError::Ok);
    assert!(!out_json.is_null());
    unsafe { privstack_free_string(out_json) };

    // Remove (no-op for undiscovered peer)
    let r = unsafe { privstack_pairing_remove_peer(peer_id.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);

    privstack_shutdown();
}

// ── Pairing: join with invalid code ─────────────────────────

#[test]
#[serial]
fn pairing_join_invalid_code() {
    test_init();

    let code = CString::new("bad").unwrap();
    let r = unsafe { privstack_pairing_join_code(code.as_ptr()) };
    assert_eq!(r, PrivStackError::InvalidSyncCode);

    privstack_shutdown();
}

// ── Pairing: trust peer with null device name ───────────────

#[test]
#[serial]
fn pairing_trust_peer_null_device_name() {
    test_init();

    let peer_id = CString::new("peer-null-name").unwrap();
    let r = unsafe { privstack_pairing_trust_peer(peer_id.as_ptr(), ptr::null()) };
    assert_eq!(r, PrivStackError::Ok);

    privstack_shutdown();
}

// ── Pairing: get code when none set ─────────────────────────

#[test]
#[serial]
fn pairing_get_code_none_set() {
    test_init();

    let mut out_code: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_pairing_get_code(&mut out_code) };
    assert_eq!(r, PrivStackError::Ok);
    assert!(out_code.is_null()); // No code set

    privstack_shutdown();
}

// ── Pairing: load invalid state ─────────────────────────────

#[test]
#[serial]
fn pairing_load_invalid_state() {
    test_init();

    let bad = CString::new("not json").unwrap();
    let r = unsafe { privstack_pairing_load_state(bad.as_ptr()) };
    assert_eq!(r, PrivStackError::JsonError);

    privstack_shutdown();
}

// ── License activate with invalid key ───────────────────────

#[test]
#[serial]
fn license_activate_invalid_key() {
    test_init();

    let key = CString::new("not-a-valid-license").unwrap();
    let mut out_json: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_license_activate(key.as_ptr(), &mut out_json) };
    assert_ne!(r, PrivStackError::Ok);

    privstack_shutdown();
}

// ── Blob read not found ─────────────────────────────────────

#[test]
#[serial]
fn blob_read_not_found() {
    test_init();

    let ns = CString::new("ns").unwrap();
    let bid = CString::new("nonexistent").unwrap();
    let mut out_data: *mut u8 = ptr::null_mut();
    let mut out_len: usize = 0;
    let r = unsafe { privstack_blob_read(ns.as_ptr(), bid.as_ptr(), &mut out_data, &mut out_len) };
    assert_eq!(r, PrivStackError::NotFound);

    privstack_shutdown();
}

#[test]
#[serial]
fn blob_delete_not_found() {
    test_init();

    let ns = CString::new("ns").unwrap();
    let bid = CString::new("nonexistent").unwrap();
    let r = unsafe { privstack_blob_delete(ns.as_ptr(), bid.as_ptr()) };
    assert_eq!(r, PrivStackError::NotFound);

    privstack_shutdown();
}

// ── Sync start/stop lifecycle ────────────────────────────────

#[test]
#[serial]
fn sync_start_stop_lifecycle() {
    test_init();

    assert!(!privstack_sync_is_running());

    let r = privstack_sync_start();
    assert_eq!(r, PrivStackError::Ok);

    // Starting again should return AlreadyRunning
    let r = privstack_sync_start();
    assert_eq!(r, PrivStackError::SyncAlreadyRunning);

    // Poll event when running but empty queue
    let mut out_json: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_sync_poll_event(&mut out_json) };
    assert_eq!(r, PrivStackError::Ok);
    // out_json may be null (no events) or have content

    // Sync trigger should succeed now
    let r = privstack_sync_trigger();
    assert_eq!(r, PrivStackError::Ok);

    // Sync status when running
    let mut out_json: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_sync_status(&mut out_json) };
    assert_eq!(r, PrivStackError::Ok);
    assert!(!out_json.is_null());
    let json = unsafe { CStr::from_ptr(out_json) }.to_str().unwrap();
    assert!(json.contains("local_peer_id"));
    unsafe { privstack_free_string(out_json) };

    let r = privstack_sync_stop();
    assert_eq!(r, PrivStackError::Ok);

    assert!(!privstack_sync_is_running());

    privstack_shutdown();
}

// ── Execute: update existing entity ──────────────────────────

#[test]
#[serial]
fn execute_update_existing_entity() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"upd_item","indexed_fields":[{"field_path":"/title","field_type":"text","searchable":true}],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    // Create
    let create = CString::new(r#"{"plugin_id":"test","action":"create","entity_type":"upd_item","payload":"{\"title\":\"Original\"}"}"#).unwrap();
    let result = unsafe { privstack_execute(create.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap().to_string();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let entity_id = parsed["data"]["id"].as_str().unwrap().to_string();
    unsafe { privstack_free_string(result) };

    // Update with explicit entity_id
    let update = CString::new(format!(
        r#"{{"plugin_id":"test","action":"update","entity_type":"upd_item","entity_id":"{}","payload":"{{\"title\":\"Updated\"}}"}}"#,
        entity_id
    )).unwrap();
    let result = unsafe { privstack_execute(update.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("Updated"));
    assert!(json.contains(&entity_id));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

// ── Execute: update nonexistent entity (creates new) ─────────

#[test]
#[serial]
fn execute_update_missing_payload() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"ump_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    let req = CString::new(r#"{"plugin_id":"test","action":"update","entity_type":"ump_item","entity_id":"x"}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("missing_payload"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

// ── Execute not initialized ──────────────────────────────────

#[test]
#[serial]
fn execute_not_initialized() {
    privstack_shutdown();

    let req = CString::new(r#"{"plugin_id":"test","action":"read","entity_type":"x","entity_id":"1"}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("not_initialized"));
    unsafe { privstack_free_string(result) };
}

// ── Search not initialized ───────────────────────────────────

#[test]
#[serial]
fn search_not_initialized() {
    privstack_shutdown();

    let query = CString::new(r#"{"query":"test"}"#).unwrap();
    let result = unsafe { privstack_search(query.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("not_initialized"));
    unsafe { privstack_free_string(result) };
}

// ── License error mapping: additional variants ───────────────

#[test]
fn license_error_mapping_expired() {
    assert_eq!(
        license_error_to_ffi(LicenseError::Expired("expired".into())),
        PrivStackError::LicenseExpired
    );
}

#[test]
fn license_error_mapping_activation_failed() {
    assert_eq!(
        license_error_to_ffi(LicenseError::ActivationFailed("fail".into())),
        PrivStackError::LicenseActivationFailed
    );
}

#[test]
fn license_error_mapping_device_limit() {
    assert_eq!(
        license_error_to_ffi(LicenseError::DeviceLimitExceeded(3)),
        PrivStackError::LicenseActivationFailed
    );
}

#[test]
fn license_error_mapping_network() {
    assert_eq!(
        license_error_to_ffi(LicenseError::Network("timeout".into())),
        PrivStackError::SyncError
    );
}

#[test]
fn license_error_mapping_storage() {
    assert_eq!(
        license_error_to_ffi(LicenseError::Storage("io".into())),
        PrivStackError::StorageError
    );
}

#[test]
fn license_error_mapping_serialization() {
    assert_eq!(
        license_error_to_ffi(LicenseError::Serialization(serde_json::from_str::<serde_json::Value>("bad").unwrap_err())),
        PrivStackError::JsonError
    );
}

// ── Execute: read_list with include_trashed ──────────────────

#[test]
#[serial]
fn execute_read_list_include_trashed() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"trash_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    // Create and trash
    let create = CString::new(r#"{"plugin_id":"test","action":"create","entity_type":"trash_item","payload":"{\"title\":\"Trashable\"}"}"#).unwrap();
    let result = unsafe { privstack_execute(create.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap().to_string();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let eid = parsed["data"]["id"].as_str().unwrap().to_string();
    unsafe { privstack_free_string(result) };

    let trash = CString::new(format!(
        r#"{{"plugin_id":"test","action":"trash","entity_type":"trash_item","entity_id":"{}"}}"#, eid
    )).unwrap();
    let r = unsafe { privstack_execute(trash.as_ptr()) };
    unsafe { privstack_free_string(r) };

    // List without trashed
    let list = CString::new(r#"{"plugin_id":"test","action":"read_list","entity_type":"trash_item"}"#).unwrap();
    let result = unsafe { privstack_execute(list.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("\"success\":true"));
    unsafe { privstack_free_string(result) };

    // List with trashed
    let list = CString::new(r#"{"plugin_id":"test","action":"read_list","entity_type":"trash_item","parameters":{"include_trashed":"true"}}"#).unwrap();
    let result = unsafe { privstack_execute(list.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("\"success\":true"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

// ── Execute: unlink missing params ───────────────────────────

#[test]
#[serial]
fn execute_unlink_missing_params() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"ulp_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    let req = CString::new(r#"{"plugin_id":"test","action":"unlink","entity_type":"ulp_item","entity_id":"x"}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("missing_params"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

// ── Auth operations not initialized ──────────────────────────

#[test]
#[serial]
fn auth_initialize_not_initialized() {
    privstack_shutdown();
    let pwd = CString::new("password123").unwrap();
    let r = unsafe { privstack_auth_initialize(pwd.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn auth_unlock_not_initialized() {
    privstack_shutdown();
    let pwd = CString::new("password123").unwrap();
    let r = unsafe { privstack_auth_unlock(pwd.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn auth_change_password_not_initialized() {
    privstack_shutdown();
    let old = CString::new("oldpass12").unwrap();
    let new = CString::new("newpass12").unwrap();
    let r = unsafe { privstack_auth_change_password(old.as_ptr(), new.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

// ── Vault operations not initialized ─────────────────────────

#[test]
#[serial]
fn vault_create_not_initialized() {
    privstack_shutdown();
    let vid = CString::new("v").unwrap();
    let r = unsafe { privstack_vault_create(vid.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn vault_initialize_not_initialized() {
    privstack_shutdown();
    let vid = CString::new("v").unwrap();
    let pwd = CString::new("password123").unwrap();
    let r = unsafe { privstack_vault_initialize(vid.as_ptr(), pwd.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn vault_unlock_not_initialized() {
    privstack_shutdown();
    let vid = CString::new("v").unwrap();
    let pwd = CString::new("password123").unwrap();
    let r = unsafe { privstack_vault_unlock(vid.as_ptr(), pwd.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn vault_lock_not_initialized() {
    privstack_shutdown();
    let vid = CString::new("v").unwrap();
    let r = unsafe { privstack_vault_lock(vid.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

// ── Blob operations not initialized ──────────────────────────

#[test]
#[serial]
fn blob_store_not_initialized() {
    privstack_shutdown();
    let ns = CString::new("ns").unwrap();
    let bid = CString::new("b1").unwrap();
    let data = b"x";
    let r = unsafe { privstack_blob_store(ns.as_ptr(), bid.as_ptr(), data.as_ptr(), data.len(), ptr::null()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn blob_read_not_initialized() {
    privstack_shutdown();
    let ns = CString::new("ns").unwrap();
    let bid = CString::new("b1").unwrap();
    let mut out_data: *mut u8 = ptr::null_mut();
    let mut out_len: usize = 0;
    let r = unsafe { privstack_blob_read(ns.as_ptr(), bid.as_ptr(), &mut out_data, &mut out_len) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn blob_delete_not_initialized() {
    privstack_shutdown();
    let ns = CString::new("ns").unwrap();
    let bid = CString::new("b1").unwrap();
    let r = unsafe { privstack_blob_delete(ns.as_ptr(), bid.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn blob_list_not_initialized() {
    privstack_shutdown();
    let ns = CString::new("ns").unwrap();
    let mut out_json: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_blob_list(ns.as_ptr(), &mut out_json) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

// ── Vault blob operations not initialized ────────────────────

#[test]
#[serial]
fn vault_blob_store_not_initialized() {
    privstack_shutdown();
    let vid = CString::new("v").unwrap();
    let bid = CString::new("b1").unwrap();
    let data = b"x";
    let r = unsafe { privstack_vault_blob_store(vid.as_ptr(), bid.as_ptr(), data.as_ptr(), data.len()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn vault_blob_read_not_initialized() {
    privstack_shutdown();
    let vid = CString::new("v").unwrap();
    let bid = CString::new("b1").unwrap();
    let mut out_data: *mut u8 = ptr::null_mut();
    let mut out_len: usize = 0;
    let r = unsafe { privstack_vault_blob_read(vid.as_ptr(), bid.as_ptr(), &mut out_data, &mut out_len) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn vault_blob_delete_not_initialized() {
    privstack_shutdown();
    let vid = CString::new("v").unwrap();
    let bid = CString::new("b1").unwrap();
    let r = unsafe { privstack_vault_blob_delete(vid.as_ptr(), bid.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn vault_blob_list_not_initialized() {
    privstack_shutdown();
    let vid = CString::new("v").unwrap();
    let mut out_json: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_vault_blob_list(vid.as_ptr(), &mut out_json) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

// ── Sync publish event with valid event (no orchestrator) ────

#[test]
#[serial]
fn sync_publish_valid_event_no_orchestrator() {
    test_init();

    // Build a valid Event JSON
    let event = privstack_types::Event::new(
        privstack_types::EntityId::new(),
        PeerId::new(),
        privstack_types::HybridTimestamp::now(),
        privstack_types::EventPayload::EntityCreated {
            entity_type: "test".to_string(),
            json_data: serde_json::json!({"test": true}).to_string(),
        },
    );
    let event_json_str = serde_json::to_string(&event).unwrap();
    let event_json = CString::new(event_json_str).unwrap();
    let r = unsafe { privstack_sync_publish_event(event_json.as_ptr()) };
    assert_eq!(r, PrivStackError::SyncNotRunning);

    privstack_shutdown();
}

// ── Register entity type: not initialized ────────────────────

#[test]
#[serial]
fn register_entity_type_not_initialized() {
    privstack_shutdown();
    let schema = CString::new(r#"{"entity_type":"x","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    let r = unsafe { privstack_register_entity_type(schema.as_ptr()) };
    assert_eq!(r, -4);
}

// ── Pairing operations not initialized ───────────────────────

#[test]
#[serial]
fn pairing_generate_code_not_initialized() {
    privstack_shutdown();
    let mut out: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_pairing_generate_code(&mut out) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn pairing_join_code_not_initialized() {
    privstack_shutdown();
    let code = CString::new("ALPHA-BETA-GAMMA-DELTA").unwrap();
    let r = unsafe { privstack_pairing_join_code(code.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn pairing_list_peers_not_initialized() {
    privstack_shutdown();
    let mut out: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_pairing_list_peers(&mut out) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn pairing_save_state_not_initialized() {
    privstack_shutdown();
    let mut out: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_pairing_save_state(&mut out) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn pairing_load_state_not_initialized() {
    privstack_shutdown();
    let json = CString::new("{}").unwrap();
    let r = unsafe { privstack_pairing_load_state(json.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn pairing_get_device_name_not_initialized() {
    privstack_shutdown();
    let mut out: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_pairing_get_device_name(&mut out) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn pairing_set_device_name_not_initialized() {
    privstack_shutdown();
    let name = CString::new("test").unwrap();
    let r = unsafe { privstack_pairing_set_device_name(name.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn pairing_trust_peer_not_initialized() {
    privstack_shutdown();
    let pid = CString::new("peer1").unwrap();
    let name = CString::new("dev").unwrap();
    let r = unsafe { privstack_pairing_trust_peer(pid.as_ptr(), name.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn pairing_remove_peer_not_initialized() {
    privstack_shutdown();
    let pid = CString::new("peer1").unwrap();
    let r = unsafe { privstack_pairing_remove_peer(pid.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn pairing_get_code_not_initialized() {
    privstack_shutdown();
    let mut out: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_pairing_get_code(&mut out) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

// ══════════════════════════════════════════════════════════════
// Phase 7: EntityRegistry unit tests
// ══════════════════════════════════════════════════════════════

#[test]
fn entity_registry_new_is_empty() {
    let reg = EntityRegistry::new();
    assert!(!reg.has_schema("anything"));
    assert!(reg.get_schema("anything").is_none());
    assert!(reg.get_handler("anything").is_none());
}

#[test]
fn entity_registry_register_and_get_schema() {
    let mut reg = EntityRegistry::new();
    let schema = EntitySchema {
        entity_type: "note".to_string(),
        indexed_fields: vec![],
        merge_strategy: privstack_model::MergeStrategy::LwwDocument,
    };
    reg.register_schema(schema);

    assert!(reg.has_schema("note"));
    assert!(!reg.has_schema("task"));

    let fetched = reg.get_schema("note").unwrap();
    assert_eq!(fetched.entity_type, "note");
}

#[test]
fn entity_registry_overwrite_schema() {
    let mut reg = EntityRegistry::new();
    let schema1 = EntitySchema {
        entity_type: "note".to_string(),
        indexed_fields: vec![],
        merge_strategy: privstack_model::MergeStrategy::LwwDocument,
    };
    reg.register_schema(schema1);

    // Overwrite with new schema of same type
    let schema2 = EntitySchema {
        entity_type: "note".to_string(),
        indexed_fields: vec![privstack_model::IndexedField {
            field_path: "/title".to_string(),
            field_type: privstack_model::FieldType::Text,
            searchable: true,
            vector_dim: None,
            enum_options: None,
        }],
        merge_strategy: privstack_model::MergeStrategy::LwwDocument,
    };
    reg.register_schema(schema2);

    let fetched = reg.get_schema("note").unwrap();
    assert_eq!(fetched.indexed_fields.len(), 1);
}

#[test]
fn entity_registry_multiple_schemas() {
    let mut reg = EntityRegistry::new();
    for name in ["note", "task", "calendar_event"] {
        reg.register_schema(EntitySchema {
            entity_type: name.to_string(),
            indexed_fields: vec![],
            merge_strategy: privstack_model::MergeStrategy::LwwDocument,
        });
    }
    assert!(reg.has_schema("note"));
    assert!(reg.has_schema("task"));
    assert!(reg.has_schema("calendar_event"));
    assert!(!reg.has_schema("contact"));
}

#[test]
fn entity_registry_get_handler_none() {
    let reg = EntityRegistry::new();
    assert!(reg.get_handler("note").is_none());
}

// ══════════════════════════════════════════════════════════════
// Phase 7: PrivStackError exhaustive enum values
// ══════════════════════════════════════════════════════════════

#[test]
fn error_enum_all_values() {
    assert_eq!(PrivStackError::Ok as i32, 0);
    assert_eq!(PrivStackError::NullPointer as i32, 1);
    assert_eq!(PrivStackError::InvalidUtf8 as i32, 2);
    assert_eq!(PrivStackError::JsonError as i32, 3);
    assert_eq!(PrivStackError::StorageError as i32, 4);
    assert_eq!(PrivStackError::NotFound as i32, 5);
    assert_eq!(PrivStackError::NotInitialized as i32, 6);
    assert_eq!(PrivStackError::SyncNotRunning as i32, 7);
    assert_eq!(PrivStackError::SyncAlreadyRunning as i32, 8);
    assert_eq!(PrivStackError::SyncError as i32, 9);
    assert_eq!(PrivStackError::PeerNotFound as i32, 10);
    assert_eq!(PrivStackError::AuthError as i32, 11);
    assert_eq!(PrivStackError::CloudError as i32, 12);
    assert_eq!(PrivStackError::LicenseInvalidFormat as i32, 13);
    assert_eq!(PrivStackError::LicenseInvalidSignature as i32, 14);
    assert_eq!(PrivStackError::LicenseExpired as i32, 15);
    assert_eq!(PrivStackError::LicenseNotActivated as i32, 16);
    assert_eq!(PrivStackError::LicenseActivationFailed as i32, 17);
    assert_eq!(PrivStackError::InvalidSyncCode as i32, 18);
    assert_eq!(PrivStackError::PeerNotTrusted as i32, 19);
    assert_eq!(PrivStackError::PairingError as i32, 20);
    assert_eq!(PrivStackError::VaultLocked as i32, 21);
    assert_eq!(PrivStackError::VaultNotFound as i32, 22);
    assert_eq!(PrivStackError::PluginError as i32, 23);
    assert_eq!(PrivStackError::PluginNotFound as i32, 24);
    assert_eq!(PrivStackError::PluginPermissionDenied as i32, 25);
    assert_eq!(PrivStackError::Unknown as i32, 99);
}

#[test]
fn error_enum_copy_clone_eq() {
    let a = PrivStackError::CloudError;
    let b = a; // Copy
    let c = a.clone(); // Clone
    assert_eq!(a, b);
    assert_eq!(a, c);
    assert_ne!(a, PrivStackError::Ok);
}

#[test]
fn error_enum_debug_all_variants() {
    // Ensure Debug is derived for every variant
    let variants = [
        PrivStackError::Ok,
        PrivStackError::NullPointer,
        PrivStackError::InvalidUtf8,
        PrivStackError::JsonError,
        PrivStackError::StorageError,
        PrivStackError::NotFound,
        PrivStackError::NotInitialized,
        PrivStackError::SyncNotRunning,
        PrivStackError::SyncAlreadyRunning,
        PrivStackError::SyncError,
        PrivStackError::PeerNotFound,
        PrivStackError::AuthError,
        PrivStackError::CloudError,
        PrivStackError::LicenseInvalidFormat,
        PrivStackError::LicenseInvalidSignature,
        PrivStackError::LicenseExpired,
        PrivStackError::LicenseNotActivated,
        PrivStackError::LicenseActivationFailed,
        PrivStackError::InvalidSyncCode,
        PrivStackError::PeerNotTrusted,
        PrivStackError::PairingError,
        PrivStackError::VaultLocked,
        PrivStackError::VaultNotFound,
        PrivStackError::PluginError,
        PrivStackError::PluginNotFound,
        PrivStackError::PluginPermissionDenied,
        PrivStackError::Unknown,
    ];
    for v in &variants {
        let dbg = format!("{:?}", v);
        assert!(!dbg.is_empty());
    }
    // Verify count matches all declared variants (27 total)
    assert_eq!(variants.len(), 27);
}

// ══════════════════════════════════════════════════════════════
// Phase 7: CloudProvider enum
// ══════════════════════════════════════════════════════════════

#[test]
fn cloud_provider_copy_clone_eq_debug() {
    let a = CloudProvider::GoogleDrive;
    let b = a;
    let c = a.clone();
    assert_eq!(a, b);
    assert_eq!(a, c);
    assert_ne!(a, CloudProvider::ICloud);
    assert_eq!(format!("{:?}", a), "GoogleDrive");
    assert_eq!(format!("{:?}", CloudProvider::ICloud), "ICloud");
}

// ══════════════════════════════════════════════════════════════
// Phase 7: FfiLicensePlan / FfiLicenseStatus enum values
// ══════════════════════════════════════════════════════════════

#[test]
fn ffi_license_plan_all_values() {
    assert_eq!(FfiLicensePlan::Monthly as i32, 0);
    assert_eq!(FfiLicensePlan::Annual as i32, 1);
    assert_eq!(FfiLicensePlan::Perpetual as i32, 2);
    assert_eq!(FfiLicensePlan::Trial as i32, 3);
}

#[test]
fn ffi_license_plan_debug_clone_eq() {
    let a = FfiLicensePlan::Perpetual;
    let b = a;
    assert_eq!(a, b);
    assert_eq!(format!("{:?}", a), "Perpetual");
}

#[test]
fn ffi_license_status_all_values() {
    assert_eq!(FfiLicenseStatus::Active as i32, 0);
    assert_eq!(FfiLicenseStatus::Expired as i32, 1);
    assert_eq!(FfiLicenseStatus::Grace as i32, 2);
    assert_eq!(FfiLicenseStatus::ReadOnly as i32, 3);
    assert_eq!(FfiLicenseStatus::NotActivated as i32, 4);
}

#[test]
fn ffi_license_status_debug_clone_eq() {
    let a = FfiLicenseStatus::Expired;
    let b = a;
    assert_eq!(a, b);
    assert_ne!(a, FfiLicenseStatus::Active);
    assert_eq!(format!("{:?}", a), "Expired");
}

// ══════════════════════════════════════════════════════════════
// Phase 7: JSON serialization of all FFI DTOs
// ══════════════════════════════════════════════════════════════

#[test]
fn discovered_peer_info_json_serialization() {
    let info = DiscoveredPeerInfo {
        peer_id: "peer-abc".to_string(),
        device_name: Some("MyLaptop".to_string()),
        discovery_method: "mdns".to_string(),
        addresses: vec!["192.168.1.1:9000".to_string()],
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"peer_id\":\"peer-abc\""));
    assert!(json.contains("\"device_name\":\"MyLaptop\""));
    assert!(json.contains("\"discovery_method\":\"mdns\""));
    assert!(json.contains("192.168.1.1:9000"));

    // Without device name
    let info2 = DiscoveredPeerInfo {
        peer_id: "peer-xyz".to_string(),
        device_name: None,
        discovery_method: "dht".to_string(),
        addresses: vec![],
    };
    let json2 = serde_json::to_string(&info2).unwrap();
    assert!(json2.contains("\"device_name\":null"));
}

#[test]
fn sync_status_json_serialization() {
    let status = SyncStatus {
        running: true,
        local_peer_id: "local-id".to_string(),
        discovered_peers: vec![
            DiscoveredPeerInfo {
                peer_id: "p1".to_string(),
                device_name: Some("Phone".to_string()),
                discovery_method: "mdns".to_string(),
                addresses: vec!["10.0.0.1:8080".to_string()],
            },
        ],
    };
    let json = serde_json::to_string(&status).unwrap();
    assert!(json.contains("\"running\":true"));
    assert!(json.contains("\"local_peer_id\":\"local-id\""));
    assert!(json.contains("\"discovered_peers\":[{"));
    assert!(json.contains("Phone"));
}

#[test]
fn sync_status_empty_peers_json() {
    let status = SyncStatus {
        running: false,
        local_peer_id: "id".to_string(),
        discovered_peers: vec![],
    };
    let json = serde_json::to_string(&status).unwrap();
    assert!(json.contains("\"running\":false"));
    assert!(json.contains("\"discovered_peers\":[]"));
}

#[test]
fn sync_event_dto_json_serialization_all_variants() {
    // PeerDiscovered
    let dto = SyncEventDto::from(SyncEvent::PeerDiscovered {
        peer_id: PeerId::new(),
        device_name: Some("Dev".to_string()),
    });
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("\"event_type\":\"peer_discovered\""));
    assert!(json.contains("\"device_name\":\"Dev\""));

    // SyncStarted
    let dto = SyncEventDto::from(SyncEvent::SyncStarted { peer_id: PeerId::new() });
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("\"event_type\":\"sync_started\""));
    assert!(json.contains("\"device_name\":null"));

    // SyncCompleted
    let dto = SyncEventDto::from(SyncEvent::SyncCompleted {
        peer_id: PeerId::new(),
        events_sent: 42,
        events_received: 7,
    });
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("\"events_sent\":42"));
    assert!(json.contains("\"events_received\":7"));

    // SyncFailed
    let dto = SyncEventDto::from(SyncEvent::SyncFailed {
        peer_id: PeerId::new(),
        error: "connection lost".to_string(),
    });
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("\"error\":\"connection lost\""));

    // EntityUpdated
    let eid = privstack_types::EntityId::new();
    let dto = SyncEventDto::from(SyncEvent::EntityUpdated { entity_id: eid });
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("\"event_type\":\"entity_updated\""));
    assert!(json.contains("\"entity_id\":\""));
}

#[test]
fn sync_event_dto_peer_discovered_no_device_name() {
    let dto = SyncEventDto::from(SyncEvent::PeerDiscovered {
        peer_id: PeerId::new(),
        device_name: None,
    });
    assert_eq!(dto.event_type, "peer_discovered");
    assert!(dto.device_name.is_none());
    assert!(dto.entity_id.is_none());
    assert!(dto.events_sent.is_none());
    assert!(dto.error.is_none());
    assert!(dto.entity_type.is_none());
    assert!(dto.json_data.is_none());
}

#[test]
fn cloud_file_info_json_serialization() {
    let info = CloudFileInfo {
        id: "file-123".to_string(),
        name: "notes.txt".to_string(),
        path: "/docs/notes.txt".to_string(),
        size: 1024,
        modified_at_ms: 1700000000000,
        content_hash: Some("abc123".to_string()),
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"id\":\"file-123\""));
    assert!(json.contains("\"name\":\"notes.txt\""));
    assert!(json.contains("\"size\":1024"));
    assert!(json.contains("\"modified_at_ms\":1700000000000"));
    assert!(json.contains("\"content_hash\":\"abc123\""));

    // Without content hash
    let info2 = CloudFileInfo {
        id: "f2".to_string(),
        name: "data.bin".to_string(),
        path: "/data.bin".to_string(),
        size: 0,
        modified_at_ms: 0,
        content_hash: None,
    };
    let json2 = serde_json::to_string(&info2).unwrap();
    assert!(json2.contains("\"content_hash\":null"));
}

#[test]
fn license_info_json_serialization() {
    let info = LicenseInfo {
        raw: "payload.sig".to_string(),
        plan: "monthly".to_string(),
        email: "user@example.com".to_string(),
        sub: 42,
        status: "active".to_string(),
        issued_at_ms: 1700000000000,
        expires_at_ms: Some(1730000000000),
        grace_days_remaining: None,
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"raw\":\"payload.sig\""));
    assert!(json.contains("\"plan\":\"monthly\""));
    assert!(json.contains("\"email\":\"user@example.com\""));
    assert!(json.contains("\"sub\":42"));
    assert!(json.contains("\"issued_at_ms\":1700000000000"));
    assert!(json.contains("\"expires_at_ms\":1730000000000"));

    // With no expiry (perpetual)
    let info2 = LicenseInfo {
        raw: "payload2.sig2".to_string(),
        plan: "perpetual".to_string(),
        email: "admin@example.com".to_string(),
        sub: 1,
        status: "active".to_string(),
        issued_at_ms: 0,
        expires_at_ms: None,
        grace_days_remaining: None,
    };
    let json2 = serde_json::to_string(&info2).unwrap();
    assert!(json2.contains("\"expires_at_ms\":null"));
}

#[test]
fn activation_info_json_serialization() {
    let info = ActivationInfo {
        license_key: "payload.sig".to_string(),
        plan: "annual".to_string(),
        email: "user@example.com".to_string(),
        sub: 42,
        activated_at_ms: 1700000000000,
        expires_at_ms: Some(1730000000000),
        device_fingerprint: "fp-123".to_string(),
        status: "active".to_string(),
        is_valid: true,
        grace_days_remaining: None,
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"license_key\":\"payload.sig\""));
    assert!(json.contains("\"plan\":\"annual\""));
    assert!(json.contains("\"device_fingerprint\":\"fp-123\""));
    assert!(json.contains("\"status\":\"active\""));
    assert!(json.contains("\"is_valid\":true"));
}

#[test]
fn ffi_device_info_json_serialization() {
    let info = FfiDeviceInfo {
        os_name: "macOS".to_string(),
        os_version: "14.0".to_string(),
        hostname: "macbook-pro".to_string(),
        arch: "aarch64".to_string(),
        fingerprint: "fp-abc-123".to_string(),
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"os_name\":\"macOS\""));
    assert!(json.contains("\"os_version\":\"14.0\""));
    assert!(json.contains("\"hostname\":\"macbook-pro\""));
    assert!(json.contains("\"arch\":\"aarch64\""));
    assert!(json.contains("\"fingerprint\":\"fp-abc-123\""));
}

// ══════════════════════════════════════════════════════════════
// Phase 7: SdkResponse helper methods
// ══════════════════════════════════════════════════════════════

#[test]
fn sdk_response_ok_with_data() {
    let resp = SdkResponse::ok(serde_json::json!({"id": "123", "title": "Test"}));
    assert!(resp.success);
    assert!(resp.error_code.is_none());
    assert!(resp.error_message.is_none());
    assert!(resp.data.is_some());

    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"success\":true"));
    assert!(json.contains("\"id\":\"123\""));
    // error_code/error_message should be absent (skip_serializing_if)
    assert!(!json.contains("error_code"));
    assert!(!json.contains("error_message"));
}

#[test]
fn sdk_response_ok_empty() {
    let resp = SdkResponse::ok_empty();
    assert!(resp.success);
    assert!(resp.error_code.is_none());
    assert!(resp.error_message.is_none());
    assert!(resp.data.is_none());

    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"success\":true"));
    assert!(!json.contains("\"data\""));
}

#[test]
fn sdk_response_err() {
    let resp = SdkResponse::err("not_found", "Entity not found");
    assert!(!resp.success);
    assert_eq!(resp.error_code.as_deref(), Some("not_found"));
    assert_eq!(resp.error_message.as_deref(), Some("Entity not found"));
    assert!(resp.data.is_none());

    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"success\":false"));
    assert!(json.contains("\"error_code\":\"not_found\""));
    assert!(json.contains("\"error_message\":\"Entity not found\""));
}

// ══════════════════════════════════════════════════════════════
// Phase 7: to_c_string and nullable_cstr_to_str helpers
// ══════════════════════════════════════════════════════════════

#[test]
fn to_c_string_roundtrip() {
    let s = "hello world";
    let ptr = to_c_string(s);
    assert!(!ptr.is_null());
    let recovered = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
    assert_eq!(recovered, s);
    unsafe { privstack_free_string(ptr) };
}

#[test]
fn to_c_string_empty() {
    let ptr = to_c_string("");
    assert!(!ptr.is_null());
    let recovered = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
    assert_eq!(recovered, "");
    unsafe { privstack_free_string(ptr) };
}

#[test]
fn to_c_string_unicode() {
    let s = "cafe\u{0301} \u{1f600}";
    let ptr = to_c_string(s);
    assert!(!ptr.is_null());
    let recovered = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
    assert_eq!(recovered, s);
    unsafe { privstack_free_string(ptr) };
}

#[test]
fn nullable_cstr_to_str_null() {
    let result = unsafe { nullable_cstr_to_str(ptr::null()) };
    assert!(result.is_none());
}

#[test]
fn nullable_cstr_to_str_valid() {
    let s = CString::new("test string").unwrap();
    let result = unsafe { nullable_cstr_to_str(s.as_ptr()) };
    assert_eq!(result, Some("test string"));
}

#[test]
fn nullable_cstr_to_str_empty() {
    let s = CString::new("").unwrap();
    let result = unsafe { nullable_cstr_to_str(s.as_ptr()) };
    assert_eq!(result, Some(""));
}

// ══════════════════════════════════════════════════════════════
// Phase 7: SyncEventDto field completeness checks
// ══════════════════════════════════════════════════════════════

#[test]
fn sync_event_dto_sync_started_fields() {
    let pid = PeerId::new();
    let dto = SyncEventDto::from(SyncEvent::SyncStarted { peer_id: pid });
    assert_eq!(dto.event_type, "sync_started");
    assert!(dto.peer_id.is_some());
    assert!(dto.device_name.is_none());
    assert!(dto.entity_id.is_none());
    assert!(dto.events_sent.is_none());
    assert!(dto.events_received.is_none());
    assert!(dto.error.is_none());
    assert!(dto.entity_type.is_none());
    assert!(dto.json_data.is_none());
}

#[test]
fn sync_event_dto_sync_completed_fields() {
    let dto = SyncEventDto::from(SyncEvent::SyncCompleted {
        peer_id: PeerId::new(),
        events_sent: 0,
        events_received: 0,
    });
    assert_eq!(dto.event_type, "sync_completed");
    assert!(dto.peer_id.is_some());
    assert_eq!(dto.events_sent, Some(0));
    assert_eq!(dto.events_received, Some(0));
    assert!(dto.device_name.is_none());
    assert!(dto.entity_id.is_none());
    assert!(dto.error.is_none());
}

#[test]
fn sync_event_dto_sync_failed_fields() {
    let dto = SyncEventDto::from(SyncEvent::SyncFailed {
        peer_id: PeerId::new(),
        error: "".to_string(),
    });
    assert_eq!(dto.event_type, "sync_failed");
    assert!(dto.peer_id.is_some());
    assert_eq!(dto.error.as_deref(), Some(""));
    assert!(dto.device_name.is_none());
    assert!(dto.entity_id.is_none());
    assert!(dto.events_sent.is_none());
}

#[test]
fn sync_event_dto_entity_updated_fields() {
    let eid = privstack_types::EntityId::new();
    let eid_str = eid.to_string();
    let dto = SyncEventDto::from(SyncEvent::EntityUpdated { entity_id: eid });
    assert_eq!(dto.event_type, "entity_updated");
    assert!(dto.peer_id.is_none());
    assert_eq!(dto.entity_id.as_deref(), Some(eid_str.as_str()));
    assert!(dto.device_name.is_none());
    assert!(dto.events_sent.is_none());
    assert!(dto.error.is_none());
}

// ══════════════════════════════════════════════════════════════
// Phase 7: SdkRequest deserialization
// ══════════════════════════════════════════════════════════════

#[test]
fn sdk_request_deserialize_full() {
    let json = r#"{"plugin_id":"notes","action":"create","entity_type":"note","entity_id":"123","payload":"{\"title\":\"hi\"}","parameters":{"limit":"10"}}"#;
    let req: SdkRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.plugin_id, "notes");
    assert_eq!(req.action, "create");
    assert_eq!(req.entity_type, "note");
    assert_eq!(req.entity_id.as_deref(), Some("123"));
    assert_eq!(req.payload.as_deref(), Some("{\"title\":\"hi\"}"));
    assert!(req.parameters.is_some());
    assert_eq!(req.parameters.as_ref().unwrap().get("limit").unwrap(), "10");
}

#[test]
fn sdk_request_deserialize_minimal() {
    let json = r#"{"plugin_id":"test","action":"read_list","entity_type":"task"}"#;
    let req: SdkRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.action, "read_list");
    assert!(req.entity_id.is_none());
    assert!(req.payload.is_none());
    assert!(req.parameters.is_none());
}

#[test]
fn sdk_request_deserialize_invalid() {
    let result = serde_json::from_str::<SdkRequest>("not json");
    assert!(result.is_err());
}

#[test]
fn sdk_request_deserialize_missing_fields() {
    // Missing required fields
    let result = serde_json::from_str::<SdkRequest>(r#"{"action":"read"}"#);
    assert!(result.is_err());
}

// ══════════════════════════════════════════════════════════════
// Phase 7: SdkResponse JSON round-trip
// ══════════════════════════════════════════════════════════════

#[test]
fn sdk_response_ok_data_json_roundtrip() {
    let resp = SdkResponse::ok(serde_json::json!({
        "items": [{"id": "1"}, {"id": "2"}],
        "count": 2
    }));
    let json = serde_json::to_string(&resp).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["success"], true);
    assert_eq!(parsed["data"]["count"], 2);
    assert_eq!(parsed["data"]["items"].as_array().unwrap().len(), 2);
}

#[test]
fn sdk_response_err_json_roundtrip() {
    let resp = SdkResponse::err("storage_error", "Disk full");
    let json = serde_json::to_string(&resp).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["success"], false);
    assert_eq!(parsed["error_code"], "storage_error");
    assert_eq!(parsed["error_message"], "Disk full");
    assert!(parsed.get("data").is_none());
}

// ══════════════════════════════════════════════════════════════
// Phase 7: Free bytes with valid allocation
// ══════════════════════════════════════════════════════════════

#[test]
fn free_bytes_zero_len() {
    // Should be a no-op (len == 0 guard)
    unsafe { privstack_free_bytes(ptr::null_mut(), 0) };
}

#[test]
fn free_bytes_valid_allocation() {
    let data = vec![1u8, 2, 3, 4, 5];
    let len = data.len();
    let boxed = data.into_boxed_slice();
    let ptr = Box::into_raw(boxed) as *mut u8;
    unsafe { privstack_free_bytes(ptr, len) };
}

// ══════════════════════════════════════════════════════════════
// Phase 7: ActivationInfo with no expiry
// ══════════════════════════════════════════════════════════════

#[test]
fn activation_info_no_expiry() {
    let info = ActivationInfo {
        license_key: "payload.sig".to_string(),
        plan: "perpetual".to_string(),
        email: "test@example.com".to_string(),
        sub: 1,
        activated_at_ms: 0,
        expires_at_ms: None,
        device_fingerprint: "fp".to_string(),
        status: "active".to_string(),
        is_valid: true,
        grace_days_remaining: None,
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"expires_at_ms\":null"));
    assert!(json.contains("\"is_valid\":true"));
}

// ══════════════════════════════════════════════════════════════
// Coverage: Personal Sharing Functions
// ══════════════════════════════════════════════════════════════

#[test]
fn share_entity_null_entity_id() {
    let r = unsafe { privstack_share_entity_with_peer(ptr::null(), ptr::null()) };
    assert_eq!(r, PrivStackError::NullPointer);
}

#[test]
fn share_entity_null_peer_id() {
    let eid = CString::new("some-id").unwrap();
    let r = unsafe { privstack_share_entity_with_peer(eid.as_ptr(), ptr::null()) };
    assert_eq!(r, PrivStackError::NullPointer);
}

#[test]
fn unshare_entity_null() {
    let r = unsafe { privstack_unshare_entity_with_peer(ptr::null(), ptr::null()) };
    assert_eq!(r, PrivStackError::NullPointer);
}

#[test]
fn unshare_entity_null_peer_id() {
    let eid = CString::new("some-id").unwrap();
    let r = unsafe { privstack_unshare_entity_with_peer(eid.as_ptr(), ptr::null()) };
    assert_eq!(r, PrivStackError::NullPointer);
}

#[test]
fn list_shared_peers_null() {
    let r = unsafe { privstack_list_shared_peers(ptr::null(), ptr::null_mut()) };
    assert_eq!(r, PrivStackError::NullPointer);
}

#[test]
fn list_shared_peers_null_out() {
    let eid = CString::new(Uuid::new_v4().to_string()).unwrap();
    let r = unsafe { privstack_list_shared_peers(eid.as_ptr(), ptr::null_mut()) };
    assert_eq!(r, PrivStackError::NullPointer);
}

#[test]
#[serial]
fn share_entity_not_initialized() {
    privstack_shutdown();
    let eid = CString::new(Uuid::new_v4().to_string()).unwrap();
    let pid = CString::new(Uuid::new_v4().to_string()).unwrap();
    let r = unsafe { privstack_share_entity_with_peer(eid.as_ptr(), pid.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn unshare_entity_not_initialized() {
    privstack_shutdown();
    let eid = CString::new(Uuid::new_v4().to_string()).unwrap();
    let pid = CString::new(Uuid::new_v4().to_string()).unwrap();
    let r = unsafe { privstack_unshare_entity_with_peer(eid.as_ptr(), pid.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn list_shared_peers_not_initialized() {
    privstack_shutdown();
    let eid = CString::new(Uuid::new_v4().to_string()).unwrap();
    let mut out: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_list_shared_peers(eid.as_ptr(), &mut out) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
fn share_entity_invalid_uuid() {
    let eid = CString::new("not-a-uuid").unwrap();
    let pid = CString::new(Uuid::new_v4().to_string()).unwrap();
    let r = unsafe { privstack_share_entity_with_peer(eid.as_ptr(), pid.as_ptr()) };
    assert_eq!(r, PrivStackError::JsonError);
}

#[test]
fn share_entity_invalid_peer_uuid() {
    let eid = CString::new(Uuid::new_v4().to_string()).unwrap();
    let pid = CString::new("not-a-uuid").unwrap();
    let r = unsafe { privstack_share_entity_with_peer(eid.as_ptr(), pid.as_ptr()) };
    assert_eq!(r, PrivStackError::JsonError);
}

#[test]
fn unshare_entity_invalid_uuid() {
    let eid = CString::new("not-a-uuid").unwrap();
    let pid = CString::new(Uuid::new_v4().to_string()).unwrap();
    let r = unsafe { privstack_unshare_entity_with_peer(eid.as_ptr(), pid.as_ptr()) };
    assert_eq!(r, PrivStackError::JsonError);
}

#[test]
fn unshare_entity_invalid_peer_uuid() {
    let eid = CString::new(Uuid::new_v4().to_string()).unwrap();
    let pid = CString::new("not-a-uuid").unwrap();
    let r = unsafe { privstack_unshare_entity_with_peer(eid.as_ptr(), pid.as_ptr()) };
    assert_eq!(r, PrivStackError::JsonError);
}

#[test]
fn list_shared_peers_invalid_uuid() {
    let eid = CString::new("not-a-uuid").unwrap();
    let mut out: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_list_shared_peers(eid.as_ptr(), &mut out) };
    assert_eq!(r, PrivStackError::JsonError);
}

#[test]
#[serial]
fn share_unshare_list_lifecycle() {
    test_init();

    // No personal_policy set (sync not started), so share/unshare are no-ops but succeed
    let eid = CString::new(Uuid::new_v4().to_string()).unwrap();
    let pid = CString::new(Uuid::new_v4().to_string()).unwrap();

    let r = unsafe { privstack_share_entity_with_peer(eid.as_ptr(), pid.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);

    let r = unsafe { privstack_unshare_entity_with_peer(eid.as_ptr(), pid.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);

    let mut out: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_list_shared_peers(eid.as_ptr(), &mut out) };
    assert_eq!(r, PrivStackError::Ok);
    assert!(!out.is_null());
    let json = unsafe { CStr::from_ptr(out) }.to_str().unwrap();
    assert!(json.contains("[]"));
    unsafe { privstack_free_string(out) };

    privstack_shutdown();
}

// ══════════════════════════════════════════════════════════════
// Coverage: Plugin FFI Functions (requires wasm-plugins feature)
// ══════════════════════════════════════════════════════════════

#[cfg(feature = "wasm-plugins")]
mod plugin_tests {
use super::*;
use privstack_ffi::plugin_ffi::*;

#[test]
#[serial]
fn plugin_list_not_initialized() {
    privstack_shutdown();
    let result = privstack_plugin_list();
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert_eq!(json, "[]");
    unsafe { privstack_free_string(result) };
}

#[test]
#[serial]
fn plugin_list_empty() {
    test_init();

    let result = privstack_plugin_list();
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("[]"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_get_nav_items_not_initialized() {
    privstack_shutdown();
    let result = privstack_plugin_get_nav_items();
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert_eq!(json, "[]");
    unsafe { privstack_free_string(result) };
}

#[test]
#[serial]
fn plugin_get_nav_items_empty() {
    test_init();

    let result = privstack_plugin_get_nav_items();
    assert!(!result.is_null());
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_count_not_initialized() {
    privstack_shutdown();
    assert_eq!(privstack_plugin_count(), 0);
}

#[test]
#[serial]
fn plugin_count_zero() {
    test_init();
    assert_eq!(privstack_plugin_count(), 0);
    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_is_loaded_not_initialized() {
    privstack_shutdown();
    let id = CString::new("test-plugin").unwrap();
    assert!(!unsafe { privstack_plugin_is_loaded(id.as_ptr()) });
}

#[test]
fn plugin_is_loaded_null() {
    assert!(!unsafe { privstack_plugin_is_loaded(ptr::null()) });
}

#[test]
#[serial]
fn plugin_is_loaded_false() {
    test_init();

    let id = CString::new("nonexistent").unwrap();
    assert!(!unsafe { privstack_plugin_is_loaded(id.as_ptr()) });

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_get_commands_not_initialized() {
    privstack_shutdown();
    let id = CString::new("test").unwrap();
    let result = unsafe { privstack_plugin_get_commands(id.as_ptr()) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert_eq!(json, "[]");
    unsafe { privstack_free_string(result) };
}

#[test]
fn plugin_get_commands_null() {
    let result = unsafe { privstack_plugin_get_commands(ptr::null()) };
    assert!(!result.is_null());
    unsafe { privstack_free_string(result) };
}

#[test]
#[serial]
fn plugin_get_commands_not_found() {
    test_init();

    let id = CString::new("nonexistent").unwrap();
    let result = unsafe { privstack_plugin_get_commands(id.as_ptr()) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert_eq!(json, "[]");
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_get_link_providers_not_initialized() {
    privstack_shutdown();
    let result = privstack_plugin_get_link_providers();
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert_eq!(json, "[]");
    unsafe { privstack_free_string(result) };
}

#[test]
#[serial]
fn plugin_get_link_providers_empty() {
    test_init();

    let result = privstack_plugin_get_link_providers();
    assert!(!result.is_null());
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_search_items_not_initialized() {
    privstack_shutdown();
    let q = CString::new("test").unwrap();
    let result = unsafe { privstack_plugin_search_items(q.as_ptr(), 10) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert_eq!(json, "[]");
    unsafe { privstack_free_string(result) };
}

#[test]
fn plugin_search_items_null_query() {
    let result = unsafe { privstack_plugin_search_items(ptr::null(), 10) };
    assert!(!result.is_null());
    unsafe { privstack_free_string(result) };
}

#[test]
#[serial]
fn plugin_search_items_empty() {
    test_init();

    let q = CString::new("test").unwrap();
    let result = unsafe { privstack_plugin_search_items(q.as_ptr(), 5) };
    assert!(!result.is_null());
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_load_not_initialized() {
    privstack_shutdown();
    let meta = CString::new("{}").unwrap();
    let schemas = CString::new("[]").unwrap();
    let perms = CString::new("{}").unwrap();
    let r = unsafe { privstack_plugin_load(meta.as_ptr(), schemas.as_ptr(), perms.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn plugin_load_null_metadata() {
    test_init();
    let schemas = CString::new("[]").unwrap();
    let perms = CString::new("{}").unwrap();
    let r = unsafe { privstack_plugin_load(ptr::null(), schemas.as_ptr(), perms.as_ptr()) };
    assert_eq!(r, PrivStackError::NullPointer);
    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_load_null_schemas() {
    test_init();
    let meta = CString::new("{}").unwrap();
    let perms = CString::new("{}").unwrap();
    let r = unsafe { privstack_plugin_load(meta.as_ptr(), ptr::null(), perms.as_ptr()) };
    assert_eq!(r, PrivStackError::NullPointer);
    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_load_null_permissions() {
    test_init();
    let meta = CString::new("{}").unwrap();
    let schemas = CString::new("[]").unwrap();
    let r = unsafe { privstack_plugin_load(meta.as_ptr(), schemas.as_ptr(), ptr::null()) };
    assert_eq!(r, PrivStackError::NullPointer);
    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_load_invalid_metadata_json() {
    test_init();

    let meta = CString::new("not json").unwrap();
    let schemas = CString::new("[]").unwrap();
    let perms = CString::new("{}").unwrap();
    let r = unsafe { privstack_plugin_load(meta.as_ptr(), schemas.as_ptr(), perms.as_ptr()) };
    assert_eq!(r, PrivStackError::JsonError);

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_load_invalid_schemas_json() {
    test_init();

    let meta = CString::new(r#"{"id":"test","name":"Test","description":"d","version":"1.0","author":"a","icon":"i","navigation_order":100,"category":"utility","can_disable":true,"is_experimental":false}"#).unwrap();
    let schemas = CString::new("not json").unwrap();
    let perms = CString::new("{}").unwrap();
    let r = unsafe { privstack_plugin_load(meta.as_ptr(), schemas.as_ptr(), perms.as_ptr()) };
    assert_eq!(r, PrivStackError::JsonError);

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_load_invalid_permissions_json() {
    test_init();

    let meta = CString::new(r#"{"id":"test","name":"Test","description":"d","version":"1.0","author":"a","icon":"i","navigation_order":100,"category":"utility","can_disable":true,"is_experimental":false}"#).unwrap();
    let schemas = CString::new("[]").unwrap();
    let perms = CString::new("not json").unwrap();
    let r = unsafe { privstack_plugin_load(meta.as_ptr(), schemas.as_ptr(), perms.as_ptr()) };
    assert_eq!(r, PrivStackError::JsonError);

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_unload_not_initialized() {
    privstack_shutdown();
    let id = CString::new("test").unwrap();
    let r = unsafe { privstack_plugin_unload(id.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
fn plugin_unload_null() {
    let r = unsafe { privstack_plugin_unload(ptr::null()) };
    // Will get NullPointer or NotInitialized depending on HANDLE state
    assert!(r == PrivStackError::NullPointer || r == PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn plugin_unload_not_found() {
    test_init();

    let id = CString::new("nonexistent").unwrap();
    let r = unsafe { privstack_plugin_unload(id.as_ptr()) };
    assert_eq!(r, PrivStackError::PluginNotFound);

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_route_sdk_not_initialized() {
    privstack_shutdown();
    let id = CString::new("test").unwrap();
    let msg = CString::new("{}").unwrap();
    let result = unsafe { privstack_plugin_route_sdk(id.as_ptr(), msg.as_ptr()) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("not_initialized"));
    unsafe { privstack_free_string(result) };
}

#[test]
#[serial]
fn plugin_route_sdk_null_plugin_id() {
    test_init();
    let msg = CString::new("{}").unwrap();
    let result = unsafe { privstack_plugin_route_sdk(ptr::null(), msg.as_ptr()) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("null_plugin_id"));
    unsafe { privstack_free_string(result) };
    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_route_sdk_null_message() {
    test_init();
    let id = CString::new("test").unwrap();
    let result = unsafe { privstack_plugin_route_sdk(id.as_ptr(), ptr::null()) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("null_message"));
    unsafe { privstack_free_string(result) };
    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_route_sdk_invalid_json() {
    test_init();

    let id = CString::new("test").unwrap();
    let msg = CString::new("not json").unwrap();
    let result = unsafe { privstack_plugin_route_sdk(id.as_ptr(), msg.as_ptr()) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("invalid_json"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_send_command_not_initialized() {
    privstack_shutdown();
    let id = CString::new("test").unwrap();
    let cmd = CString::new("do_thing").unwrap();
    let args = CString::new("{}").unwrap();
    let result = unsafe { privstack_plugin_send_command(id.as_ptr(), cmd.as_ptr(), args.as_ptr()) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("not_initialized"));
    unsafe { privstack_free_string(result) };
}

#[test]
#[serial]
fn plugin_send_command_null_plugin_id() {
    test_init();
    let cmd = CString::new("do_thing").unwrap();
    let args = CString::new("{}").unwrap();
    let result = unsafe { privstack_plugin_send_command(ptr::null(), cmd.as_ptr(), args.as_ptr()) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("null_plugin_id"));
    unsafe { privstack_free_string(result) };
    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_send_command_null_command() {
    test_init();
    let id = CString::new("test").unwrap();
    let args = CString::new("{}").unwrap();
    let result = unsafe { privstack_plugin_send_command(id.as_ptr(), ptr::null(), args.as_ptr()) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("null_command"));
    unsafe { privstack_free_string(result) };
    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_send_command_null_args() {
    test_init();

    let id = CString::new("nonexistent").unwrap();
    let cmd = CString::new("do_thing").unwrap();
    let result = unsafe { privstack_plugin_send_command(id.as_ptr(), cmd.as_ptr(), ptr::null()) };
    assert!(!result.is_null());
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_fetch_url_not_initialized() {
    privstack_shutdown();
    let id = CString::new("test").unwrap();
    let url = CString::new("https://example.com").unwrap();
    let mut out_data: *mut u8 = ptr::null_mut();
    let mut out_len: usize = 0;
    let r = unsafe { privstack_plugin_fetch_url(id.as_ptr(), url.as_ptr(), &mut out_data, &mut out_len) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn plugin_fetch_url_null_plugin_id() {
    test_init();
    let url = CString::new("https://example.com").unwrap();
    let mut out_data: *mut u8 = ptr::null_mut();
    let mut out_len: usize = 0;
    let r = unsafe { privstack_plugin_fetch_url(ptr::null(), url.as_ptr(), &mut out_data, &mut out_len) };
    assert_eq!(r, PrivStackError::NullPointer);
    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_fetch_url_null_url() {
    test_init();
    let id = CString::new("test").unwrap();
    let mut out_data: *mut u8 = ptr::null_mut();
    let mut out_len: usize = 0;
    let r = unsafe { privstack_plugin_fetch_url(id.as_ptr(), ptr::null(), &mut out_data, &mut out_len) };
    assert_eq!(r, PrivStackError::NullPointer);
    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_get_view_state_not_initialized() {
    privstack_shutdown();
    let id = CString::new("test").unwrap();
    let result = unsafe { privstack_plugin_get_view_state(id.as_ptr()) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("Core not initialized"));
    unsafe { privstack_free_string(result) };
}

#[test]
fn plugin_get_view_state_null() {
    let result = unsafe { privstack_plugin_get_view_state(ptr::null()) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("null") || json.contains("error"));
    unsafe { privstack_free_string(result) };
}

#[test]
#[serial]
fn plugin_get_view_state_not_found() {
    test_init();

    let id = CString::new("nonexistent").unwrap();
    let result = unsafe { privstack_plugin_get_view_state(id.as_ptr()) };
    assert!(!result.is_null());
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_get_view_data_not_initialized() {
    privstack_shutdown();
    let id = CString::new("test").unwrap();
    let result = unsafe { privstack_plugin_get_view_data(id.as_ptr()) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert_eq!(json, "{}");
    unsafe { privstack_free_string(result) };
}

#[test]
fn plugin_get_view_data_null() {
    let result = unsafe { privstack_plugin_get_view_data(ptr::null()) };
    assert!(!result.is_null());
    unsafe { privstack_free_string(result) };
}

#[test]
#[serial]
fn plugin_get_view_data_not_found() {
    test_init();

    let id = CString::new("nonexistent").unwrap();
    let result = unsafe { privstack_plugin_get_view_data(id.as_ptr()) };
    assert!(!result.is_null());
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_activate_not_initialized() {
    privstack_shutdown();
    let id = CString::new("test").unwrap();
    let r = unsafe { privstack_plugin_activate(id.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
fn plugin_activate_null() {
    let r = unsafe { privstack_plugin_activate(ptr::null()) };
    assert!(r == PrivStackError::NullPointer || r == PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn plugin_activate_not_found() {
    test_init();

    let id = CString::new("nonexistent").unwrap();
    let r = unsafe { privstack_plugin_activate(id.as_ptr()) };
    assert_eq!(r, PrivStackError::PluginError);

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_navigated_to_not_initialized() {
    privstack_shutdown();
    let id = CString::new("test").unwrap();
    let r = unsafe { privstack_plugin_navigated_to(id.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
fn plugin_navigated_to_null() {
    let r = unsafe { privstack_plugin_navigated_to(ptr::null()) };
    assert!(r == PrivStackError::NullPointer || r == PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn plugin_navigated_to_not_found() {
    test_init();

    let id = CString::new("nonexistent").unwrap();
    let r = unsafe { privstack_plugin_navigated_to(id.as_ptr()) };
    assert_eq!(r, PrivStackError::PluginError);

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_navigated_from_not_initialized() {
    privstack_shutdown();
    let id = CString::new("test").unwrap();
    let r = unsafe { privstack_plugin_navigated_from(id.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
fn plugin_navigated_from_null() {
    let r = unsafe { privstack_plugin_navigated_from(ptr::null()) };
    assert!(r == PrivStackError::NullPointer || r == PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn plugin_navigated_from_not_found() {
    test_init();

    let id = CString::new("nonexistent").unwrap();
    let r = unsafe { privstack_plugin_navigated_from(id.as_ptr()) };
    assert_eq!(r, PrivStackError::PluginError);

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_update_permissions_not_initialized() {
    privstack_shutdown();
    let id = CString::new("test").unwrap();
    let perms = CString::new("{}").unwrap();
    let r = unsafe { privstack_plugin_update_permissions(id.as_ptr(), perms.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
fn plugin_update_permissions_null_id() {
    let perms = CString::new("{}").unwrap();
    let r = unsafe { privstack_plugin_update_permissions(ptr::null(), perms.as_ptr()) };
    assert!(r == PrivStackError::NullPointer || r == PrivStackError::NotInitialized);
}

#[test]
fn plugin_update_permissions_null_perms() {
    let id = CString::new("test").unwrap();
    let r = unsafe { privstack_plugin_update_permissions(id.as_ptr(), ptr::null()) };
    assert!(r == PrivStackError::NullPointer || r == PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn plugin_update_permissions_invalid_json() {
    test_init();

    let id = CString::new("test").unwrap();
    let perms = CString::new("not json").unwrap();
    let r = unsafe { privstack_plugin_update_permissions(id.as_ptr(), perms.as_ptr()) };
    assert_eq!(r, PrivStackError::JsonError);

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_update_permissions_not_found() {
    test_init();

    let id = CString::new("nonexistent").unwrap();
    let perms = CString::new(r#"{"granted":["sdk"],"denied":[],"pending_jit":[]}"#).unwrap();
    let r = unsafe { privstack_plugin_update_permissions(id.as_ptr(), perms.as_ptr()) };
    assert_eq!(r, PrivStackError::PluginNotFound);

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_install_ppk_not_initialized() {
    privstack_shutdown();
    let path = CString::new("/nonexistent.ppk").unwrap();
    let r = unsafe { privstack_plugin_install_ppk(path.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
fn plugin_install_ppk_null() {
    let r = unsafe { privstack_plugin_install_ppk(ptr::null()) };
    assert!(r == PrivStackError::NullPointer || r == PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn plugin_install_ppk_file_not_found() {
    test_init();

    let ppk_path = CString::new("/tmp/nonexistent_test.ppk").unwrap();
    let r = unsafe { privstack_plugin_install_ppk(ppk_path.as_ptr()) };
    assert_eq!(r, PrivStackError::NotFound);

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_load_wasm_not_initialized() {
    privstack_shutdown();
    let path = CString::new("/nonexistent.wasm").unwrap();
    let perms = CString::new("{}").unwrap();
    let mut out_id: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_plugin_load_wasm(path.as_ptr(), perms.as_ptr(), &mut out_id) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
fn plugin_load_wasm_null_path() {
    let perms = CString::new("{}").unwrap();
    let mut out_id: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_plugin_load_wasm(ptr::null(), perms.as_ptr(), &mut out_id) };
    assert!(r == PrivStackError::NullPointer || r == PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn plugin_load_wasm_file_not_found() {
    test_init();

    let wasm_path = CString::new("/tmp/nonexistent_test.wasm").unwrap();
    let perms = CString::new("{}").unwrap();
    let mut out_id: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_plugin_load_wasm(wasm_path.as_ptr(), perms.as_ptr(), &mut out_id) };
    assert_eq!(r, PrivStackError::PluginError);

    privstack_shutdown();
}

#[test]
#[serial]
fn plugin_load_wasm_null_permissions() {
    test_init();

    let wasm_path = CString::new("/tmp/nonexistent_test.wasm").unwrap();
    let mut out_id: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_plugin_load_wasm(wasm_path.as_ptr(), ptr::null(), &mut out_id) };
    // Null permissions should use default_first_party, then fail on file not found
    assert_eq!(r, PrivStackError::PluginError);

    privstack_shutdown();
}

#[test]
fn ppk_inspect_null() {
    let result = unsafe { privstack_ppk_inspect(ptr::null()) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert_eq!(json, "{}");
    unsafe { privstack_free_string(result) };
}

#[test]
fn ppk_inspect_file_not_found() {
    let path = CString::new("/tmp/nonexistent_test.ppk").unwrap();
    let result = unsafe { privstack_ppk_inspect(path.as_ptr()) };
    assert!(!result.is_null());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert_eq!(json, "{}");
    unsafe { privstack_free_string(result) };
}

#[test]
fn ppk_content_hash_null() {
    let result = unsafe { privstack_ppk_content_hash(ptr::null()) };
    assert!(!result.is_null());
    let s = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert_eq!(s, "");
    unsafe { privstack_free_string(result) };
}

#[test]
fn ppk_content_hash_file_not_found() {
    let path = CString::new("/tmp/nonexistent_test.ppk").unwrap();
    let result = unsafe { privstack_ppk_content_hash(path.as_ptr()) };
    assert!(!result.is_null());
    let s = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert_eq!(s, "");
    unsafe { privstack_free_string(result) };
}

// ══════════════════════════════════════════════════════════════
// Coverage: Cloud iCloud operations not initialized
// ══════════════════════════════════════════════════════════════

#[test]
#[serial]
fn cloud_icloud_operations_without_provider_init() {
    test_init();

    let mut out_auth_url: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_cloud_authenticate(CloudProvider::ICloud, &mut out_auth_url) };
    assert_eq!(r, PrivStackError::NotInitialized);

    let code = CString::new("code").unwrap();
    let r = unsafe { privstack_cloud_complete_auth(CloudProvider::ICloud, code.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);

    let mut out_json: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_cloud_list_files(CloudProvider::ICloud, &mut out_json) };
    assert_eq!(r, PrivStackError::NotInitialized);

    let name = CString::new("f.txt").unwrap();
    let data = b"x";
    let r = unsafe { privstack_cloud_upload(CloudProvider::ICloud, name.as_ptr(), data.as_ptr(), data.len(), &mut out_json) };
    assert_eq!(r, PrivStackError::NotInitialized);

    let fid = CString::new("fid").unwrap();
    let mut out_data: *mut u8 = ptr::null_mut();
    let mut out_len: usize = 0;
    let r = unsafe { privstack_cloud_download(CloudProvider::ICloud, fid.as_ptr(), &mut out_data, &mut out_len) };
    assert_eq!(r, PrivStackError::NotInitialized);

    let r = unsafe { privstack_cloud_delete(CloudProvider::ICloud, fid.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);

    privstack_shutdown();
}

// ══════════════════════════════════════════════════════════════
// Coverage: Sync status/trigger/poll not initialized
// ══════════════════════════════════════════════════════════════

#[test]
#[serial]
fn sync_is_running_not_initialized() {
    privstack_shutdown();
    assert!(!privstack_sync_is_running());
}

#[test]
#[serial]
fn sync_status_not_initialized() {
    privstack_shutdown();
    let mut out: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_sync_status(&mut out) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn sync_trigger_not_initialized() {
    privstack_shutdown();
    let r = privstack_sync_trigger();
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn sync_poll_event_not_initialized() {
    privstack_shutdown();
    let mut out: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_sync_poll_event(&mut out) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
fn sync_poll_event_null() {
    let r = unsafe { privstack_sync_poll_event(ptr::null_mut()) };
    assert_eq!(r, PrivStackError::NullPointer);
}

#[test]
#[serial]
fn sync_publish_event_not_initialized() {
    privstack_shutdown();
    let event = privstack_types::Event::new(
        privstack_types::EntityId::new(),
        PeerId::new(),
        privstack_types::HybridTimestamp::now(),
        privstack_types::EventPayload::EntityCreated {
            entity_type: "test".to_string(),
            json_data: "{}".to_string(),
        },
    );
    let json = CString::new(serde_json::to_string(&event).unwrap()).unwrap();
    let r = unsafe { privstack_sync_publish_event(json.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

// ══════════════════════════════════════════════════════════════
// Coverage: Cloud not-initialized paths
// ══════════════════════════════════════════════════════════════

#[test]
#[serial]
fn cloud_init_google_drive_not_initialized() {
    privstack_shutdown();
    let cid = CString::new("id").unwrap();
    let csec = CString::new("sec").unwrap();
    let r = unsafe { privstack_cloud_init_google_drive(cid.as_ptr(), csec.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn cloud_init_icloud_not_initialized() {
    privstack_shutdown();
    let r = unsafe { privstack_cloud_init_icloud(ptr::null()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

// ══════════════════════════════════════════════════════════════
// Coverage: License checks not initialized
// ══════════════════════════════════════════════════════════════

#[test]
#[serial]
fn license_check_not_initialized() {
    privstack_shutdown();
    let mut out: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_license_check(&mut out) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn license_status_not_initialized() {
    privstack_shutdown();
    let mut out = FfiLicenseStatus::Active;
    let r = unsafe { privstack_license_status(&mut out) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn license_activated_plan_not_initialized() {
    privstack_shutdown();
    let mut out = FfiLicensePlan::Monthly;
    let r = unsafe { privstack_license_activated_plan(&mut out) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

#[test]
#[serial]
fn license_activate_not_initialized() {
    privstack_shutdown();
    let key = CString::new("PS-KEY-1234").unwrap();
    let mut out: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_license_activate(key.as_ptr(), &mut out) };
    // Will fail on parse before reaching NotInitialized, or hit NotInitialized
    assert_ne!(r, PrivStackError::Ok);
}

// ══════════════════════════════════════════════════════════════
// Coverage: Vault is_initialized/is_unlocked not initialized
// ══════════════════════════════════════════════════════════════

#[test]
#[serial]
fn vault_is_initialized_not_initialized() {
    privstack_shutdown();
    let vid = CString::new("v").unwrap();
    assert!(!unsafe { privstack_vault_is_initialized(vid.as_ptr()) });
}

#[test]
#[serial]
fn vault_is_unlocked_not_initialized() {
    privstack_shutdown();
    let vid = CString::new("v").unwrap();
    assert!(!unsafe { privstack_vault_is_unlocked(vid.as_ptr()) });
}

#[test]
#[serial]
fn vault_change_password_not_initialized() {
    privstack_shutdown();
    let vid = CString::new("v").unwrap();
    let old = CString::new("old123456").unwrap();
    let new = CString::new("new123456").unwrap();
    let r = unsafe { privstack_vault_change_password(vid.as_ptr(), old.as_ptr(), new.as_ptr()) };
    assert_eq!(r, PrivStackError::NotInitialized);
}

// ══════════════════════════════════════════════════════════════
// Coverage: Plugin load + unload lifecycle
// ══════════════════════════════════════════════════════════════

#[test]
#[serial]
fn plugin_load_and_unload_lifecycle() {
    test_init();

    let meta = CString::new(r#"{"id":"test-plugin","name":"Test Plugin","description":"A test","version":"1.0.0","author":"Test","icon":"plug","navigation_order":100,"category":"utility","can_disable":true,"is_experimental":false}"#).unwrap();
    let schemas = CString::new("[]").unwrap();
    let perms = CString::new(r#"{"granted":["sdk","settings","logger","navigation","state-notify"],"denied":[],"pending_jit":[]}"#).unwrap();

    let r = unsafe { privstack_plugin_load(meta.as_ptr(), schemas.as_ptr(), perms.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);

    assert_eq!(privstack_plugin_count(), 1);

    let id = CString::new("test-plugin").unwrap();
    assert!(unsafe { privstack_plugin_is_loaded(id.as_ptr()) });

    // List plugins
    let result = privstack_plugin_list();
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("test-plugin"));
    unsafe { privstack_free_string(result) };

    // Nav items
    let result = privstack_plugin_get_nav_items();
    assert!(!result.is_null());
    unsafe { privstack_free_string(result) };

    // Link providers
    let result = privstack_plugin_get_link_providers();
    assert!(!result.is_null());
    unsafe { privstack_free_string(result) };

    // Get commands
    let result = unsafe { privstack_plugin_get_commands(id.as_ptr()) };
    assert!(!result.is_null());
    unsafe { privstack_free_string(result) };

    // Loading same plugin again should fail
    let r = unsafe { privstack_plugin_load(meta.as_ptr(), schemas.as_ptr(), perms.as_ptr()) };
    assert_eq!(r, PrivStackError::PluginError);

    // Unload
    let r = unsafe { privstack_plugin_unload(id.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);
    assert!(!unsafe { privstack_plugin_is_loaded(id.as_ptr()) });
    assert_eq!(privstack_plugin_count(), 0);

    privstack_shutdown();
}

} // mod plugin_tests (cfg wasm-plugins)

// ══════════════════════════════════════════════════════════════
// Coverage: Trusted peers via pairing_get_trusted_peers
// ══════════════════════════════════════════════════════════════

#[test]
fn pairing_get_trusted_peers_null() {
    let r = unsafe { privstack_pairing_get_trusted_peers(ptr::null_mut()) };
    assert_eq!(r, PrivStackError::NullPointer);
}

#[test]
#[serial]
fn pairing_get_trusted_peers_lifecycle() {
    test_init();

    let mut out: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_pairing_get_trusted_peers(&mut out) };
    assert_eq!(r, PrivStackError::Ok);
    assert!(!out.is_null());
    unsafe { privstack_free_string(out) };

    privstack_shutdown();
}

// ══════════════════════════════════════════════════════════════
// Coverage: Cloud provider name without init
// ══════════════════════════════════════════════════════════════

#[test]
fn cloud_provider_name_standalone() {
    let name = privstack_cloud_provider_name(CloudProvider::GoogleDrive);
    let s = unsafe { CStr::from_ptr(name) }.to_str().unwrap();
    assert_eq!(s, "Google Drive");

    let name = privstack_cloud_provider_name(CloudProvider::ICloud);
    let s = unsafe { CStr::from_ptr(name) }.to_str().unwrap();
    assert_eq!(s, "iCloud Drive");
}

// ══════════════════════════════════════════════════════════════
// Coverage: Execute query with filters payload
// ══════════════════════════════════════════════════════════════

#[test]
#[serial]
fn execute_query_with_limit_param() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"qlimit_item","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    let create = CString::new(r#"{"plugin_id":"test","action":"create","entity_type":"qlimit_item","payload":"{\"n\":1}"}"#).unwrap();
    let r = unsafe { privstack_execute(create.as_ptr()) };
    unsafe { privstack_free_string(r) };

    let query = CString::new(r#"{"plugin_id":"test","action":"query","entity_type":"qlimit_item","parameters":{"limit":"1"}}"#).unwrap();
    let result = unsafe { privstack_execute(query.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("\"success\":true"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

// ══════════════════════════════════════════════════════════════
// Coverage: Execute update with nonexistent entity_id
// ══════════════════════════════════════════════════════════════

#[test]
#[serial]
fn execute_update_nonexistent_entity() {
    test_init();

    let schema = CString::new(r#"{"entity_type":"upd_ne","indexed_fields":[],"merge_strategy":"lww_document"}"#).unwrap();
    unsafe { privstack_register_entity_type(schema.as_ptr()) };

    let req = CString::new(r#"{"plugin_id":"test","action":"update","entity_type":"upd_ne","entity_id":"nonexistent-id","payload":"{\"title\":\"New\"}"}"#).unwrap();
    let result = unsafe { privstack_execute(req.as_ptr()) };
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("\"success\":true"));
    unsafe { privstack_free_string(result) };

    privstack_shutdown();
}

// ══════════════════════════════════════════════════════════════
// Coverage: Sync start with pairing code set
// ══════════════════════════════════════════════════════════════

#[test]
#[serial]
fn sync_start_with_pairing_code() {
    test_init();

    // Generate pairing code first
    let mut out_code: *mut c_char = ptr::null_mut();
    unsafe { privstack_pairing_generate_code(&mut out_code) };
    if !out_code.is_null() {
        unsafe { privstack_free_string(out_code) };
    }

    // Start sync with code set
    let r = privstack_sync_start();
    assert_eq!(r, PrivStackError::Ok);

    // Check running
    assert!(privstack_sync_is_running());

    // Stop
    let r = privstack_sync_stop();
    assert_eq!(r, PrivStackError::Ok);

    privstack_shutdown();
}

// ══════════════════════════════════════════════════════════════
// Coverage: Sync stop when not running
// ══════════════════════════════════════════════════════════════

#[test]
#[serial]
fn sync_stop_when_not_running() {
    test_init();

    // Stop without starting should be OK (no-op for None handles)
    let r = privstack_sync_stop();
    assert_eq!(r, PrivStackError::Ok);

    privstack_shutdown();
}

// ══════════════════════════════════════════════════════════════
// Coverage: Pairing join with valid code
// ══════════════════════════════════════════════════════════════

#[test]
#[serial]
fn pairing_join_with_generated_code() {
    test_init();

    // Generate a code
    let mut out_code: *mut c_char = ptr::null_mut();
    let r = unsafe { privstack_pairing_generate_code(&mut out_code) };
    assert_eq!(r, PrivStackError::Ok);
    assert!(!out_code.is_null());
    let code_json = unsafe { CStr::from_ptr(out_code) }.to_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(code_json).expect("should be valid JSON");
    let code_str = parsed["code"].as_str().expect("should have code field").to_string();
    unsafe { privstack_free_string(out_code) };

    // Join with that code
    let code = CString::new(code_str).unwrap();
    let r = unsafe { privstack_pairing_join_code(code.as_ptr()) };
    assert_eq!(r, PrivStackError::Ok);

    privstack_shutdown();
}
