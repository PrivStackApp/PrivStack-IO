use privstack_db::*;
use tempfile::TempDir;

fn temp_db_path(dir: &TempDir, name: &str) -> std::path::PathBuf {
    dir.path().join(name)
}

#[test]
fn open_encrypted_db_and_read_back() {
    let dir = TempDir::new().unwrap();
    let path = temp_db_path(&dir, "test.db");

    let key = format_sqlcipher_key(&[0xAB; 32]);

    // Create and write
    {
        let conn = open_db(&path, &key).unwrap();
        conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT);")
            .unwrap();
        conn.execute("INSERT INTO t (id, val) VALUES (1, 'hello')", [])
            .unwrap();
    }

    // Reopen and read
    {
        let conn = open_db(&path, &key).unwrap();
        let val: String =
            conn.query_row("SELECT val FROM t WHERE id = 1", [], |row| row.get(0))
                .unwrap();
        assert_eq!(val, "hello");
    }
}

#[test]
fn wrong_key_fails_to_open() {
    let dir = TempDir::new().unwrap();
    let path = temp_db_path(&dir, "test.db");

    let key = format_sqlcipher_key(&[0xAB; 32]);
    let wrong_key = format_sqlcipher_key(&[0xCD; 32]);

    // Create with correct key
    {
        let conn = open_db(&path, &key).unwrap();
        conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY);")
            .unwrap();
    }

    // Attempt to open with wrong key
    let result = open_db(&path, &wrong_key);
    assert!(result.is_err());
}

#[test]
fn rekey_changes_password() {
    let dir = TempDir::new().unwrap();
    let path = temp_db_path(&dir, "test.db");

    let old_key = format_sqlcipher_key(&[0xAA; 32]);
    let new_key = format_sqlcipher_key(&[0xBB; 32]);

    // Create with old key, insert data, then rekey
    {
        let conn = open_db(&path, &old_key).unwrap();
        conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT);")
            .unwrap();
        conn.execute("INSERT INTO t (id, val) VALUES (1, 'secret')", [])
            .unwrap();
        rekey(&conn, &new_key).unwrap();
    }

    // Old key should fail
    assert!(open_db(&path, &old_key).is_err());

    // New key should work and data should be intact
    {
        let conn = open_db(&path, &new_key).unwrap();
        let val: String =
            conn.query_row("SELECT val FROM t WHERE id = 1", [], |row| row.get(0))
                .unwrap();
        assert_eq!(val, "secret");
    }
}

#[test]
fn in_memory_db_works() {
    let conn = open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT);")
        .unwrap();
    conn.execute("INSERT INTO t (id, val) VALUES (1, 'test')", [])
        .unwrap();
    let val: String =
        conn.query_row("SELECT val FROM t WHERE id = 1", [], |row| row.get(0))
            .unwrap();
    assert_eq!(val, "test");
}

#[test]
fn unencrypted_db_works() {
    let dir = TempDir::new().unwrap();
    let path = temp_db_path(&dir, "plain.db");

    {
        let conn = open_db_unencrypted(&path).unwrap();
        conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT);")
            .unwrap();
        conn.execute("INSERT INTO t (id, val) VALUES (1, 'plain')", [])
            .unwrap();
    }

    {
        let conn = open_db_unencrypted(&path).unwrap();
        let val: String =
            conn.query_row("SELECT val FROM t WHERE id = 1", [], |row| row.get(0))
                .unwrap();
        assert_eq!(val, "plain");
    }
}

#[test]
fn checkpoint_runs() {
    let dir = TempDir::new().unwrap();
    let path = temp_db_path(&dir, "wal.db");
    let conn = open_db_unencrypted(&path).unwrap();
    conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY);")
        .unwrap();
    checkpoint(&conn).unwrap();
}

#[test]
fn compact_reduces_or_maintains_size() {
    let dir = TempDir::new().unwrap();
    let path = temp_db_path(&dir, "compact.db");

    let conn = open_db_unencrypted(&path).unwrap();
    conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT);")
        .unwrap();

    // Insert and delete rows to create fragmentation
    for i in 0..1000 {
        conn.execute(
            "INSERT INTO t (id, val) VALUES (?1, ?2)",
            rusqlite::params![i, format!("value_{i}")],
        )
        .unwrap();
    }
    conn.execute("DELETE FROM t WHERE id < 900", []).unwrap();
    checkpoint(&conn).unwrap();
    drop(conn);

    // Reopen and compact
    let conn = open_db_unencrypted(&path).unwrap();
    let (before, after) = compact(&conn, &path).unwrap();
    assert!(after <= before, "Compacted size {after} should be <= original {before}");
}

#[test]
fn db_size_returns_nonzero() {
    let dir = TempDir::new().unwrap();
    let path = temp_db_path(&dir, "size.db");
    let conn = open_db_unencrypted(&path).unwrap();
    conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY);")
        .unwrap();
    let size = db_size(&conn).unwrap();
    assert!(size > 0);
}

#[test]
fn list_tables_returns_created_tables() {
    let conn = open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE alpha (id INTEGER PRIMARY KEY);
         CREATE TABLE bravo (id INTEGER PRIMARY KEY);",
    )
    .unwrap();
    let tables = list_tables(&conn).unwrap();
    assert!(tables.contains(&"alpha".to_string()));
    assert!(tables.contains(&"bravo".to_string()));
}

#[test]
fn table_exists_check() {
    let conn = open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE existing (id INTEGER PRIMARY KEY);")
        .unwrap();
    assert!(table_exists(&conn, "existing").unwrap());
    assert!(!table_exists(&conn, "nonexistent").unwrap());
}

#[test]
fn column_exists_check() {
    let conn = open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT);")
        .unwrap();
    assert!(column_exists(&conn, "t", "id").unwrap());
    assert!(column_exists(&conn, "t", "name").unwrap());
    assert!(!column_exists(&conn, "t", "missing").unwrap());
}

#[test]
fn add_column_if_not_exists_is_idempotent() {
    let conn = open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY);")
        .unwrap();

    // First call adds the column
    add_column_if_not_exists(&conn, "t", "new_col", "TEXT DEFAULT ''").unwrap();
    assert!(column_exists(&conn, "t", "new_col").unwrap());

    // Second call is a no-op (doesn't error)
    add_column_if_not_exists(&conn, "t", "new_col", "TEXT DEFAULT ''").unwrap();
}

#[test]
fn cosine_similarity_works_in_query() {
    let conn = open_in_memory().unwrap();
    register_custom_functions(&conn).unwrap();

    conn.execute_batch(
        "CREATE TABLE vectors (id INTEGER PRIMARY KEY, embedding TEXT);
         INSERT INTO vectors (id, embedding) VALUES (1, '[1.0, 0.0, 0.0]');
         INSERT INTO vectors (id, embedding) VALUES (2, '[0.0, 1.0, 0.0]');
         INSERT INTO vectors (id, embedding) VALUES (3, '[0.9, 0.1, 0.0]');",
    )
    .unwrap();

    // Query for vectors similar to [1.0, 0.0, 0.0]
    let mut stmt = conn
        .prepare(
            "SELECT id, cosine_similarity(embedding, '[1.0, 0.0, 0.0]') AS score
             FROM vectors ORDER BY score DESC",
        )
        .unwrap();
    let results: Vec<(i64, f64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(results[0].0, 1); // Most similar
    assert!((results[0].1 - 1.0).abs() < 1e-10);
    assert_eq!(results[1].0, 3); // Second most similar
    assert_eq!(results[2].0, 2); // Least similar (orthogonal)
}

#[test]
fn format_sqlcipher_key_format() {
    let key_bytes = [0u8; 32];
    let formatted = format_sqlcipher_key(&key_bytes);
    assert_eq!(
        formatted,
        "x'0000000000000000000000000000000000000000000000000000000000000000'"
    );

    let key_bytes = [0xFF; 32];
    let formatted = format_sqlcipher_key(&key_bytes);
    assert_eq!(
        formatted,
        "x'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff'"
    );
}

#[test]
fn wal_mode_is_set() {
    let dir = TempDir::new().unwrap();
    let path = temp_db_path(&dir, "wal.db");
    let conn = open_db_unencrypted(&path).unwrap();
    let mode: String =
        conn.pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
    assert_eq!(mode, "wal");
}

#[test]
fn foreign_keys_enabled() {
    let conn = open_in_memory().unwrap();
    let fk: i32 =
        conn.pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
    assert_eq!(fk, 1);
}

#[test]
fn fts5_is_available() {
    let conn = open_in_memory().unwrap();
    let result = conn.execute_batch(
        "CREATE VIRTUAL TABLE test_fts USING fts5(title, body);
         INSERT INTO test_fts (title, body) VALUES ('Hello World', 'This is a test document');
         INSERT INTO test_fts (title, body) VALUES ('Rust Programming', 'Systems programming language');",
    );
    assert!(result.is_ok(), "FTS5 should be available in bundled SQLCipher");

    let count: i32 = conn
        .query_row(
            "SELECT count(*) FROM test_fts WHERE test_fts MATCH 'test'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}
