use privstack_sync::pairing::{
    DiscoveredPeerInfo, PairingManager, PairingMessage, PairingStatus, SyncCode, SyncCodeError,
    TrustedPeer,
};

// ── SyncCode ────────────────────────────────────────────────────

#[test]
fn sync_code_generate_produces_4_words() {
    let code = SyncCode::generate();
    let words: Vec<&str> = code.code.split('-').collect();
    assert_eq!(words.len(), 4);
    for w in &words {
        assert!(!w.is_empty());
        assert!(w.chars().all(|c| c.is_ascii_uppercase()));
    }
}

#[test]
fn sync_code_generate_has_hash() {
    let code = SyncCode::generate();
    assert!(!code.hash.is_empty());
    // SHA-256 hex = 64 chars
    assert_eq!(code.hash.len(), 64);
}

#[test]
fn sync_code_generate_unique() {
    let a = SyncCode::generate();
    let b = SyncCode::generate();
    // Vanishingly unlikely collision
    assert_ne!(a.code, b.code);
    assert_ne!(a.hash, b.hash);
}

#[test]
fn sync_code_from_input_normalizes_lowercase() {
    let code = SyncCode::from_input("apple-banana-cherry-delta").unwrap();
    assert_eq!(code.code, "APPLE-BANANA-CHERRY-DELTA");
}

#[test]
fn sync_code_from_input_normalizes_spaces() {
    let code = SyncCode::from_input("APPLE BANANA CHERRY DELTA").unwrap();
    assert_eq!(code.code, "APPLE-BANANA-CHERRY-DELTA");
}

#[test]
fn sync_code_from_input_normalizes_underscores() {
    let code = SyncCode::from_input("apple_banana_cherry_delta").unwrap();
    assert_eq!(code.code, "APPLE-BANANA-CHERRY-DELTA");
}

#[test]
fn sync_code_from_input_trims_whitespace() {
    let code = SyncCode::from_input("  apple-banana-cherry-delta  ").unwrap();
    assert_eq!(code.code, "APPLE-BANANA-CHERRY-DELTA");
}

#[test]
fn sync_code_from_input_same_hash_regardless_of_format() {
    let a = SyncCode::from_input("apple-banana-cherry-delta").unwrap();
    let b = SyncCode::from_input("APPLE BANANA CHERRY DELTA").unwrap();
    let c = SyncCode::from_input("apple_banana_cherry_delta").unwrap();
    assert_eq!(a.hash, b.hash);
    assert_eq!(b.hash, c.hash);
}

#[test]
fn sync_code_from_input_rejects_too_few_words() {
    let result = SyncCode::from_input("apple-banana");
    assert!(result.is_err());
}

#[test]
fn sync_code_from_input_rejects_too_many_words() {
    let result = SyncCode::from_input("apple-banana-cherry-delta-echo");
    assert!(result.is_err());
}

#[test]
fn sync_code_from_input_rejects_single_word() {
    let result = SyncCode::from_input("apple");
    assert!(result.is_err());
}

#[test]
fn sync_code_from_input_rejects_empty() {
    let result = SyncCode::from_input("");
    assert!(result.is_err());
}

#[test]
fn sync_code_dht_namespace_returns_32_bytes() {
    let code = SyncCode::from_input("apple-banana-cherry-delta").unwrap();
    let ns = code.dht_namespace();
    assert_eq!(ns.len(), 32); // SHA-256 = 32 bytes
}

#[test]
fn sync_code_dht_namespace_deterministic() {
    let a = SyncCode::from_input("apple-banana-cherry-delta").unwrap();
    let b = SyncCode::from_input("APPLE-BANANA-CHERRY-DELTA").unwrap();
    assert_eq!(a.dht_namespace(), b.dht_namespace());
}

#[test]
fn sync_code_serde_roundtrip() {
    let code = SyncCode::from_input("apple-banana-cherry-delta").unwrap();
    let json = serde_json::to_string(&code).unwrap();
    let parsed: SyncCode = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.code, code.code);
    assert_eq!(parsed.hash, code.hash);
}

#[test]
fn sync_code_clone_and_eq() {
    let code = SyncCode::from_input("apple-banana-cherry-delta").unwrap();
    let cloned = code.clone();
    assert_eq!(code, cloned);
}

// ── SyncCodeError ───────────────────────────────────────────────

#[test]
fn sync_code_error_display() {
    let err = SyncCodeError::InvalidFormat("test message".into());
    let msg = format!("{err}");
    assert!(msg.contains("Invalid sync code format"));
    assert!(msg.contains("test message"));
}

#[test]
fn sync_code_error_is_std_error() {
    let err = SyncCodeError::InvalidFormat("x".into());
    let _: &dyn std::error::Error = &err;
}

#[test]
fn sync_code_error_serde_roundtrip() {
    let err = SyncCodeError::InvalidFormat("bad".into());
    let json = serde_json::to_string(&err).unwrap();
    let parsed: SyncCodeError = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{parsed}"), format!("{err}"));
}

// ── PairingStatus ───────────────────────────────────────────────

#[test]
fn pairing_status_eq_and_copy() {
    let a = PairingStatus::Trusted;
    let b = a; // Copy
    assert_eq!(a, b);
}

#[test]
fn pairing_status_serde_roundtrip() {
    for status in [
        PairingStatus::PendingLocalApproval,
        PairingStatus::PendingRemoteApproval,
        PairingStatus::Trusted,
        PairingStatus::Rejected,
    ] {
        let json = serde_json::to_string(&status).unwrap();
        let parsed: PairingStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, status);
    }
}

// ── DiscoveredPeerInfo ──────────────────────────────────────────

fn make_discovered_peer(id: &str) -> DiscoveredPeerInfo {
    DiscoveredPeerInfo {
        peer_id: id.to_string(),
        device_name: "Device".to_string(),
        discovered_at: 1000,
        status: PairingStatus::PendingLocalApproval,
        addresses: vec!["/ip4/127.0.0.1/tcp/4001".to_string()],
    }
}

#[test]
fn discovered_peer_info_serde_roundtrip() {
    let peer = make_discovered_peer("peer-1");
    let json = serde_json::to_string(&peer).unwrap();
    let parsed: DiscoveredPeerInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.peer_id, "peer-1");
    assert_eq!(parsed.device_name, "Device");
    assert_eq!(parsed.discovered_at, 1000);
    assert_eq!(parsed.status, PairingStatus::PendingLocalApproval);
    assert_eq!(parsed.addresses.len(), 1);
}

#[test]
fn discovered_peer_info_clone() {
    let peer = make_discovered_peer("peer-1");
    let cloned = peer.clone();
    assert_eq!(cloned.peer_id, peer.peer_id);
}

// ── TrustedPeer ─────────────────────────────────────────────────

#[test]
fn trusted_peer_from_discovered() {
    let discovered = make_discovered_peer("peer-1");
    let trusted = TrustedPeer::from_discovered(&discovered);
    assert_eq!(trusted.peer_id, "peer-1");
    assert_eq!(trusted.device_name, "Device");
    assert!(trusted.approved_at > 0);
    assert!(trusted.last_synced.is_none());
    assert_eq!(trusted.addresses, discovered.addresses);
}

#[test]
fn trusted_peer_mark_synced() {
    let discovered = make_discovered_peer("peer-1");
    let mut trusted = TrustedPeer::from_discovered(&discovered);
    assert!(trusted.last_synced.is_none());

    trusted.mark_synced();
    assert!(trusted.last_synced.is_some());
    let ts = trusted.last_synced.unwrap();
    assert!(ts > 0);
}

#[test]
fn trusted_peer_serde_roundtrip() {
    let discovered = make_discovered_peer("peer-1");
    let trusted = TrustedPeer::from_discovered(&discovered);
    let json = serde_json::to_string(&trusted).unwrap();
    let parsed: TrustedPeer = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.peer_id, trusted.peer_id);
    assert_eq!(parsed.device_name, trusted.device_name);
    assert_eq!(parsed.approved_at, trusted.approved_at);
}

// ── PairingManager ──────────────────────────────────────────────

#[test]
fn pairing_manager_new_is_empty() {
    let mgr = PairingManager::new();
    assert!(mgr.current_code().is_none());
    assert!(mgr.discovered_peers().is_empty());
    assert!(mgr.trusted_peers().is_empty());
}

#[test]
fn pairing_manager_default_equals_new() {
    let a = PairingManager::new();
    let b = PairingManager::default();
    assert!(a.current_code().is_none());
    assert!(b.current_code().is_none());
}

#[test]
fn pairing_manager_set_and_get_sync_code() {
    let mut mgr = PairingManager::new();
    let code = SyncCode::generate();
    mgr.set_sync_code(code.clone());
    assert_eq!(mgr.current_code().unwrap().code, code.code);
}

#[test]
fn pairing_manager_set_sync_code_clears_discovered() {
    let mut mgr = PairingManager::new();
    mgr.set_sync_code(SyncCode::generate());
    mgr.add_discovered_peer(make_discovered_peer("p1"));
    assert_eq!(mgr.discovered_peers().len(), 1);

    // Setting a new code clears discovered peers
    mgr.set_sync_code(SyncCode::generate());
    assert!(mgr.discovered_peers().is_empty());
}

#[test]
fn pairing_manager_clear_sync_code() {
    let mut mgr = PairingManager::new();
    mgr.set_sync_code(SyncCode::generate());
    mgr.add_discovered_peer(make_discovered_peer("p1"));

    mgr.clear_sync_code();
    assert!(mgr.current_code().is_none());
    assert!(mgr.discovered_peers().is_empty());
}

#[test]
fn pairing_manager_add_discovered_peer() {
    let mut mgr = PairingManager::new();
    mgr.add_discovered_peer(make_discovered_peer("p1"));
    assert_eq!(mgr.discovered_peers().len(), 1);
}

#[test]
fn pairing_manager_add_discovered_skips_trusted() {
    let mut mgr = PairingManager::new();
    mgr.add_discovered_peer(make_discovered_peer("p1"));
    mgr.approve_peer("p1");
    assert!(mgr.is_trusted("p1"));

    // Adding same peer as discovered should be a no-op
    mgr.add_discovered_peer(make_discovered_peer("p1"));
    assert!(mgr.discovered_peers().is_empty());
}

#[test]
fn pairing_manager_get_discovered_peer() {
    let mut mgr = PairingManager::new();
    assert!(mgr.get_discovered_peer("p1").is_none());

    mgr.add_discovered_peer(make_discovered_peer("p1"));
    let peer = mgr.get_discovered_peer("p1").unwrap();
    assert_eq!(peer.peer_id, "p1");
}

#[test]
fn pairing_manager_remove_discovered_peer() {
    let mut mgr = PairingManager::new();
    mgr.add_discovered_peer(make_discovered_peer("p1"));
    assert!(mgr.get_discovered_peer("p1").is_some());

    mgr.remove_discovered_peer("p1");
    assert!(mgr.get_discovered_peer("p1").is_none());
}

#[test]
fn pairing_manager_remove_discovered_peer_nonexistent_is_noop() {
    let mut mgr = PairingManager::new();
    mgr.remove_discovered_peer("nope"); // should not panic
}

#[test]
fn pairing_manager_approve_peer() {
    let mut mgr = PairingManager::new();
    mgr.add_discovered_peer(make_discovered_peer("p1"));

    let trusted = mgr.approve_peer("p1").unwrap();
    assert_eq!(trusted.peer_id, "p1");
    assert!(mgr.is_trusted("p1"));
    assert!(mgr.get_discovered_peer("p1").is_none()); // removed from discovered
}

#[test]
fn pairing_manager_approve_nonexistent_returns_none() {
    let mut mgr = PairingManager::new();
    assert!(mgr.approve_peer("nope").is_none());
}

#[test]
fn pairing_manager_reject_peer() {
    let mut mgr = PairingManager::new();
    mgr.add_discovered_peer(make_discovered_peer("p1"));

    mgr.reject_peer("p1");
    let peer = mgr.get_discovered_peer("p1").unwrap();
    assert_eq!(peer.status, PairingStatus::Rejected);
}

#[test]
fn pairing_manager_reject_nonexistent_is_noop() {
    let mut mgr = PairingManager::new();
    mgr.reject_peer("nope"); // should not panic
}

#[test]
fn pairing_manager_trusted_peers() {
    let mut mgr = PairingManager::new();
    mgr.add_discovered_peer(make_discovered_peer("p1"));
    mgr.add_discovered_peer(make_discovered_peer("p2"));
    mgr.approve_peer("p1");
    mgr.approve_peer("p2");

    assert_eq!(mgr.trusted_peers().len(), 2);
}

#[test]
fn pairing_manager_get_trusted_peer() {
    let mut mgr = PairingManager::new();
    assert!(mgr.get_trusted_peer("p1").is_none());

    mgr.add_discovered_peer(make_discovered_peer("p1"));
    mgr.approve_peer("p1");
    assert!(mgr.get_trusted_peer("p1").is_some());
}

#[test]
fn pairing_manager_get_trusted_peer_mut() {
    let mut mgr = PairingManager::new();
    mgr.add_discovered_peer(make_discovered_peer("p1"));
    mgr.approve_peer("p1");

    let peer = mgr.get_trusted_peer_mut("p1").unwrap();
    peer.device_name = "Renamed".to_string();
    assert_eq!(mgr.get_trusted_peer("p1").unwrap().device_name, "Renamed");
}

#[test]
fn pairing_manager_get_trusted_peer_mut_missing() {
    let mut mgr = PairingManager::new();
    assert!(mgr.get_trusted_peer_mut("nope").is_none());
}

#[test]
fn pairing_manager_remove_trusted_peer() {
    let mut mgr = PairingManager::new();
    mgr.add_discovered_peer(make_discovered_peer("p1"));
    mgr.approve_peer("p1");
    assert!(mgr.is_trusted("p1"));

    mgr.remove_trusted_peer("p1");
    assert!(!mgr.is_trusted("p1"));
}

#[test]
fn pairing_manager_remove_trusted_peer_nonexistent_is_noop() {
    let mut mgr = PairingManager::new();
    mgr.remove_trusted_peer("nope"); // should not panic
}

#[test]
fn pairing_manager_update_peer_addresses() {
    let mut mgr = PairingManager::new();
    mgr.add_discovered_peer(make_discovered_peer("p1"));
    mgr.approve_peer("p1");

    let new_addrs = vec!["/ip4/10.0.0.1/tcp/9000".to_string()];
    mgr.update_peer_addresses("p1", new_addrs.clone());
    assert_eq!(mgr.get_trusted_peer("p1").unwrap().addresses, new_addrs);
}

#[test]
fn pairing_manager_update_peer_addresses_nonexistent_is_noop() {
    let mut mgr = PairingManager::new();
    mgr.update_peer_addresses("nope", vec![]); // should not panic
}

#[test]
fn pairing_manager_mark_peer_synced() {
    let mut mgr = PairingManager::new();
    mgr.add_discovered_peer(make_discovered_peer("p1"));
    mgr.approve_peer("p1");
    assert!(mgr.get_trusted_peer("p1").unwrap().last_synced.is_none());

    mgr.mark_peer_synced("p1");
    assert!(mgr.get_trusted_peer("p1").unwrap().last_synced.is_some());
}

#[test]
fn pairing_manager_mark_peer_synced_nonexistent_is_noop() {
    let mut mgr = PairingManager::new();
    mgr.mark_peer_synced("nope"); // should not panic
}

#[test]
fn pairing_manager_is_trusted() {
    let mut mgr = PairingManager::new();
    assert!(!mgr.is_trusted("p1"));

    mgr.add_discovered_peer(make_discovered_peer("p1"));
    assert!(!mgr.is_trusted("p1"));

    mgr.approve_peer("p1");
    assert!(mgr.is_trusted("p1"));
}

// ── PairingManager JSON persistence ─────────────────────────────

#[test]
fn pairing_manager_to_json_and_from_json() {
    let mut mgr = PairingManager::new();
    mgr.set_sync_code(SyncCode::from_input("apple-banana-cherry-delta").unwrap());
    mgr.add_discovered_peer(make_discovered_peer("p1"));
    mgr.add_discovered_peer(make_discovered_peer("p2"));
    mgr.approve_peer("p2");

    let json = mgr.to_json().unwrap();
    let restored = PairingManager::from_json(&json).unwrap();

    assert_eq!(
        restored.current_code().unwrap().code,
        "APPLE-BANANA-CHERRY-DELTA"
    );
    assert_eq!(restored.discovered_peers().len(), 1);
    assert!(restored.get_discovered_peer("p1").is_some());
    assert!(restored.is_trusted("p2"));
}

#[test]
fn pairing_manager_from_json_invalid() {
    let result = PairingManager::from_json("not json");
    assert!(result.is_err());
}

#[test]
fn pairing_manager_from_json_empty_object() {
    // Default fields should work
    let mgr = PairingManager::from_json(r#"{"current_code":null,"discovered_peers":{},"trusted_peers":{}}"#).unwrap();
    assert!(mgr.current_code().is_none());
}

// ── PairingMessage ──────────────────────────────────────────────

#[test]
fn pairing_message_announce_serde() {
    let msg = PairingMessage::Announce {
        peer_id: "p1".into(),
        device_name: "Dev".into(),
        addresses: vec!["/ip4/1.2.3.4/tcp/4001".into()],
    };
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: PairingMessage = serde_json::from_str(&json).unwrap();
    if let PairingMessage::Announce { peer_id, device_name, addresses } = parsed {
        assert_eq!(peer_id, "p1");
        assert_eq!(device_name, "Dev");
        assert_eq!(addresses.len(), 1);
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn pairing_message_pair_request_serde() {
    let msg = PairingMessage::PairRequest {
        peer_id: "p1".into(),
        device_name: "Dev".into(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: PairingMessage = serde_json::from_str(&json).unwrap();
    if let PairingMessage::PairRequest { peer_id, device_name } = parsed {
        assert_eq!(peer_id, "p1");
        assert_eq!(device_name, "Dev");
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn pairing_message_pair_accept_serde() {
    let msg = PairingMessage::PairAccept {
        peer_id: "p1".into(),
        device_name: "Dev".into(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: PairingMessage = serde_json::from_str(&json).unwrap();
    if let PairingMessage::PairAccept { peer_id, .. } = parsed {
        assert_eq!(peer_id, "p1");
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn pairing_message_pair_reject_serde() {
    let msg = PairingMessage::PairReject {
        peer_id: "p1".into(),
        reason: Some("not trusted".into()),
    };
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: PairingMessage = serde_json::from_str(&json).unwrap();
    if let PairingMessage::PairReject { peer_id, reason } = parsed {
        assert_eq!(peer_id, "p1");
        assert_eq!(reason.unwrap(), "not trusted");
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn pairing_message_pair_reject_no_reason() {
    let msg = PairingMessage::PairReject {
        peer_id: "p1".into(),
        reason: None,
    };
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: PairingMessage = serde_json::from_str(&json).unwrap();
    if let PairingMessage::PairReject { reason, .. } = parsed {
        assert!(reason.is_none());
    } else {
        panic!("wrong variant");
    }
}
