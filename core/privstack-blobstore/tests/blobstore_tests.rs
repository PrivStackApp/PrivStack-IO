use privstack_blobstore::{BlobStore, BlobStoreError};
use std::sync::Arc;

// ── Error type coverage ─────────────────────────────────────────

#[test]
fn error_display() {
    let err = BlobStoreError::NotFound("ns".to_string(), "id".to_string());
    assert!(format!("{err}").contains("ns"));
    assert!(format!("{err}").contains("id"));

    let err = BlobStoreError::Storage("disk full".to_string());
    assert!(format!("{err}").contains("disk full"));
}

#[test]
fn error_debug() {
    let err = BlobStoreError::NotFound("ns".to_string(), "id".to_string());
    assert!(format!("{err:?}").contains("NotFound"));

    let err = BlobStoreError::Storage("oops".to_string());
    assert!(format!("{err:?}").contains("Storage"));
}

// ── open_with_conn ──────────────────────────────────────────────

#[test]
fn open_with_conn() {
    let conn = privstack_db::open_in_memory().unwrap();
    let conn = Arc::new(std::sync::Mutex::new(conn));
    let store = BlobStore::open_with_conn(conn).unwrap();

    store.store("ns", "b1", b"data", None).unwrap();
    assert_eq!(store.read("ns", "b1").unwrap(), b"data");
}

// ── BlobMetadata Debug ──────────────────────────────────────────

#[test]
fn blob_metadata_debug() {
    let store = BlobStore::open_in_memory().unwrap();
    store
        .store("ns", "b1", b"data", Some(r#"{"k":"v"}"#))
        .unwrap();
    let items = store.list("ns").unwrap();
    let debug = format!("{:?}", items[0]);
    assert!(debug.contains("BlobMetadata"));
    assert!(debug.contains("b1"));
}

// ── BlobMetadata Serialize ──────────────────────────────────────

#[test]
fn blob_metadata_serialize() {
    let store = BlobStore::open_in_memory().unwrap();
    store
        .store("ns", "s1", b"ser test", Some(r#"{"x":1}"#))
        .unwrap();
    let items = store.list("ns").unwrap();
    let json = serde_json::to_string(&items[0]).unwrap();
    assert!(json.contains("s1"));
    assert!(json.contains("ns"));
}

// ── Multiple namespaces listing ─────────────────────────────────

#[test]
fn list_multiple_namespaces_independent() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("a", "1", b"x", None).unwrap();
    store.store("b", "2", b"y", None).unwrap();
    store.store("b", "3", b"z", None).unwrap();

    assert_eq!(store.list("a").unwrap().len(), 1);
    assert_eq!(store.list("b").unwrap().len(), 2);
    assert!(store.list("c").unwrap().is_empty());
}

// ── Update metadata preserves data ──────────────────────────────

#[test]
fn update_metadata_preserves_data() {
    let store = BlobStore::open_in_memory().unwrap();
    store
        .store("ns", "b1", b"original data", Some(r#"{"v":1}"#))
        .unwrap();
    store.update_metadata("ns", "b1", r#"{"v":2}"#).unwrap();

    let data = store.read("ns", "b1").unwrap();
    assert_eq!(data, b"original data");
    let items = store.list("ns").unwrap();
    assert_eq!(items[0].metadata_json.as_deref(), Some(r#"{"v":2}"#));
}

// ── Store no metadata ───────────────────────────────────────────

#[test]
fn store_no_metadata_returns_none() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "b1", b"data", None).unwrap();
    let items = store.list("ns").unwrap();
    assert!(items[0].metadata_json.is_none());
}

#[test]
fn store_and_read() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("media", "img1", b"png data", None).unwrap();
    let data = store.read("media", "img1").unwrap();
    assert_eq!(data, b"png data");
}

#[test]
fn store_with_metadata() {
    let store = BlobStore::open_in_memory().unwrap();
    store
        .store("docs", "f1", b"content", Some(r#"{"mime":"text/plain"}"#))
        .unwrap();
    let items = store.list("docs").unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].metadata_json.as_deref(),
        Some(r#"{"mime":"text/plain"}"#)
    );
}

#[test]
fn read_nonexistent_fails() {
    let store = BlobStore::open_in_memory().unwrap();
    assert!(store.read("ns", "nope").is_err());
}

#[test]
fn delete_blob() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "b1", b"data", None).unwrap();
    store.delete("ns", "b1").unwrap();
    assert!(store.read("ns", "b1").is_err());
}

#[test]
fn delete_nonexistent_fails() {
    let store = BlobStore::open_in_memory().unwrap();
    assert!(store.delete("ns", "nope").is_err());
}

#[test]
fn list_empty_namespace() {
    let store = BlobStore::open_in_memory().unwrap();
    assert!(store.list("empty").unwrap().is_empty());
}

#[test]
fn list_multiple_blobs() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "a", b"aaa", None).unwrap();
    store.store("ns", "b", b"bbb", None).unwrap();
    store.store("other", "c", b"ccc", None).unwrap();

    assert_eq!(store.list("ns").unwrap().len(), 2);
    assert_eq!(store.list("other").unwrap().len(), 1);
}

#[test]
fn blob_metadata_fields() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "b1", b"hello", None).unwrap();
    let items = store.list("ns").unwrap();
    assert_eq!(items[0].namespace, "ns");
    assert_eq!(items[0].blob_id, "b1");
    assert_eq!(items[0].size, 5);
    assert!(items[0].content_hash.is_some());
    assert!(items[0].created_at > 0);
    assert!(items[0].modified_at > 0);
}

#[test]
fn content_hash_is_sha256() {
    use sha2::{Digest, Sha256};
    let store = BlobStore::open_in_memory().unwrap();
    let data = b"test data for hashing";
    store.store("ns", "h1", data, None).unwrap();

    let expected: String = Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect();
    let items = store.list("ns").unwrap();
    assert_eq!(items[0].content_hash.as_deref(), Some(expected.as_str()));
}

#[test]
fn overwrite_blob() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "b1", b"v1", None).unwrap();
    store.store("ns", "b1", b"v2 updated", None).unwrap();

    let data = store.read("ns", "b1").unwrap();
    assert_eq!(data, b"v2 updated");
    let items = store.list("ns").unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].size, 10);
}

#[test]
fn update_metadata() {
    let store = BlobStore::open_in_memory().unwrap();
    store
        .store("ns", "b1", b"data", Some(r#"{"mime":"image/png"}"#))
        .unwrap();
    store
        .update_metadata("ns", "b1", r#"{"tags":["new"]}"#)
        .unwrap();

    let items = store.list("ns").unwrap();
    assert_eq!(
        items[0].metadata_json.as_deref(),
        Some(r#"{"tags":["new"]}"#)
    );
}

#[test]
fn update_metadata_nonexistent_fails() {
    let store = BlobStore::open_in_memory().unwrap();
    assert!(store.update_metadata("ns", "nope", "{}").is_err());
}

#[test]
fn empty_blob() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("ns", "empty", b"", None).unwrap();
    let data = store.read("ns", "empty").unwrap();
    assert!(data.is_empty());
    let items = store.list("ns").unwrap();
    assert_eq!(items[0].size, 0);
}

#[test]
fn large_blob() {
    let store = BlobStore::open_in_memory().unwrap();
    let data: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
    store.store("ns", "large", &data, None).unwrap();
    let retrieved = store.read("ns", "large").unwrap();
    assert_eq!(retrieved, data);
}

#[test]
fn namespace_isolation() {
    let store = BlobStore::open_in_memory().unwrap();
    store.store("a", "key", b"from_a", None).unwrap();
    store.store("b", "key", b"from_b", None).unwrap();

    assert_eq!(store.read("a", "key").unwrap(), b"from_a");
    assert_eq!(store.read("b", "key").unwrap(), b"from_b");

    store.delete("a", "key").unwrap();
    assert!(store.read("a", "key").is_err());
    assert_eq!(store.read("b", "key").unwrap(), b"from_b");
}

#[test]
fn open_with_path() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("blobs.db");
    let store = BlobStore::open(&db_path).unwrap();
    store.store("ns", "b1", b"data", None).unwrap();
    drop(store);

    // Reopen and verify persistence
    let store2 = BlobStore::open(&db_path).unwrap();
    let data = store2.read("ns", "b1").unwrap();
    assert_eq!(data, b"data");
}

// ── Poisoned mutex tests ────────────────────────────────────────

fn make_poisoned_store() -> BlobStore {
    let conn = privstack_db::open_in_memory().unwrap();
    let shared = Arc::new(std::sync::Mutex::new(conn));
    let store = BlobStore::open_with_conn(shared.clone()).unwrap();
    // Poison the mutex by panicking while holding the lock
    let shared2 = shared.clone();
    let _ = std::thread::spawn(move || {
        let _guard = shared2.lock().unwrap();
        panic!("intentional poison");
    })
    .join();
    store
}

#[test]
fn store_with_poisoned_mutex() {
    let store = make_poisoned_store();
    let result = store.store("ns", "b1", b"data", None);
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(msg.contains("poison"), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn read_with_poisoned_mutex() {
    let store = make_poisoned_store();
    let result = store.read("ns", "b1");
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(msg.contains("poison"), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn delete_with_poisoned_mutex() {
    let store = make_poisoned_store();
    let result = store.delete("ns", "b1");
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(msg.contains("poison"), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn list_with_poisoned_mutex() {
    let store = make_poisoned_store();
    let result = store.list("ns");
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(msg.contains("poison"), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn update_metadata_with_poisoned_mutex() {
    let store = make_poisoned_store();
    let result = store.update_metadata("ns", "b1", "{}");
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(msg.contains("poison"), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn ensure_tables_with_poisoned_conn() {
    let conn = privstack_db::open_in_memory().unwrap();
    let shared = Arc::new(std::sync::Mutex::new(conn));
    let shared2 = shared.clone();
    let _ = std::thread::spawn(move || {
        let _guard = shared2.lock().unwrap();
        panic!("intentional poison");
    })
    .join();
    let result = BlobStore::open_with_conn(shared);
    assert!(result.is_err());
    match result.err().unwrap() {
        BlobStoreError::Storage(msg) => assert!(msg.contains("poison"), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

// ── Dropped-table tests ─────────────────────────────────────────

fn make_broken_store() -> BlobStore {
    let conn = privstack_db::open_in_memory().unwrap();
    let shared = Arc::new(std::sync::Mutex::new(conn));
    let store = BlobStore::open_with_conn(shared.clone()).unwrap();
    store.store("ns", "b1", b"data", None).unwrap();
    {
        let c = shared.lock().unwrap();
        c.execute_batch("DROP TABLE blobs").unwrap();
    }
    store
}

#[test]
fn store_with_dropped_table() {
    let store = make_broken_store();
    let result = store.store("ns", "b2", b"data", None);
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(!msg.is_empty(), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn read_with_dropped_table() {
    let store = make_broken_store();
    let result = store.read("ns", "b1");
    assert!(result.is_err());
}

#[test]
fn delete_with_dropped_table() {
    let store = make_broken_store();
    let result = store.delete("ns", "b1");
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(!msg.is_empty(), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn list_with_dropped_table() {
    let store = make_broken_store();
    let result = store.list("ns");
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(!msg.is_empty(), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

#[test]
fn update_metadata_with_dropped_table() {
    let store = make_broken_store();
    let result = store.update_metadata("ns", "b1", "{}");
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage(msg) => assert!(!msg.is_empty(), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}

// ── Invalid path tests ──────────────────────────────────────────

#[test]
fn open_with_invalid_path() {
    let result = BlobStore::open(std::path::Path::new(
        "/nonexistent/dir/that/does/not/exist/db.sqlite",
    ));
    assert!(result.is_err());
    match result.err().unwrap() {
        BlobStoreError::Storage(msg) => assert!(!msg.is_empty(), "got: {msg}"),
        other => panic!("expected Storage error, got: {other}"),
    }
}
