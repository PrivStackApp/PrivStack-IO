//! SQLCipher database connection management for PrivStack.
//!
//! Provides a unified abstraction over `rusqlite` with SQLCipher encryption,
//! replacing the previous per-crate DuckDB connections. All database access
//! in PrivStack routes through this module.

mod error;
mod functions;

pub use error::{DbError, DbResult};
pub use functions::register_custom_functions;
pub use rusqlite;

use rusqlite::Connection;
use std::path::Path;

/// Open a SQLCipher-encrypted database with standard PRAGMAs.
///
/// The `encryption_key` should be a hex-encoded key string formatted as
/// `x'<hex_bytes>'` for raw key mode, or a passphrase string.
pub fn open_db(path: &Path, encryption_key: &str) -> DbResult<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "key", encryption_key)?;

    // Verify the key is correct by reading the first page.
    // SQLCipher will fail here if the key is wrong.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))?;

    apply_pragmas(&conn)?;
    Ok(conn)
}

/// Open an unencrypted database with standard PRAGMAs.
///
/// Used for databases that don't require encryption (e.g., datasets in
/// scenarios where encryption is disabled).
pub fn open_db_unencrypted(path: &Path) -> DbResult<Connection> {
    let conn = Connection::open(path)?;
    apply_pragmas(&conn)?;
    Ok(conn)
}

/// Open an in-memory database for testing (no encryption).
pub fn open_in_memory() -> DbResult<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

/// Change the database encryption key from inside an already-open connection.
///
/// This re-encrypts every page of the database file. The database must
/// already be open with the current key.
pub fn rekey(conn: &Connection, new_key: &str) -> DbResult<()> {
    conn.pragma_update(None, "rekey", new_key)?;
    Ok(())
}

/// Flush the WAL (Write-Ahead Log) to the main database file.
pub fn checkpoint(conn: &Connection) -> DbResult<()> {
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    Ok(())
}

/// Create a compacted copy of the database, then atomically swap it in.
///
/// Returns `(size_before, size_after)` in bytes.
pub fn compact(conn: &Connection, db_path: &Path) -> DbResult<(u64, u64)> {
    let size_before = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);
    let compact_path = db_path.with_extension("db.compact");
    let compact_str = compact_path
        .to_str()
        .ok_or_else(|| DbError::InvalidPath(compact_path.display().to_string()))?;

    conn.execute_batch(&format!("VACUUM INTO '{compact_str}';"))?;
    std::fs::rename(&compact_path, db_path)?;

    let size_after = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);
    Ok((size_before, size_after))
}

/// Get database file size in bytes via page_count * page_size.
pub fn db_size(conn: &Connection) -> DbResult<u64> {
    let page_count: u64 = conn.pragma_query_value(None, "page_count", |row| row.get(0))?;
    let page_size: u64 = conn.pragma_query_value(None, "page_size", |row| row.get(0))?;
    Ok(page_count * page_size)
}

/// List all user tables in the database.
pub fn list_tables(conn: &Connection) -> DbResult<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
    let tables = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tables)
}

/// Check if a table exists in the database.
pub fn table_exists(conn: &Connection, table_name: &str) -> DbResult<bool> {
    let count: u32 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
        [table_name],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Check if a column exists on a table.
pub fn column_exists(conn: &Connection, table_name: &str, column_name: &str) -> DbResult<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info('{table_name}')"))?;
    let exists = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .any(|name| name.is_ok_and(|n| n == column_name));
    Ok(exists)
}

/// Add a column to a table if it doesn't already exist.
///
/// SQLite doesn't support `ADD COLUMN IF NOT EXISTS`, so we check
/// `PRAGMA table_info()` first.
pub fn add_column_if_not_exists(
    conn: &Connection,
    table_name: &str,
    column_name: &str,
    column_def: &str,
) -> DbResult<()> {
    if !column_exists(conn, table_name, column_name)? {
        conn.execute_batch(&format!(
            "ALTER TABLE {table_name} ADD COLUMN {column_name} {column_def};"
        ))?;
    }
    Ok(())
}

/// Apply standard performance PRAGMAs to a connection.
fn apply_pragmas(conn: &Connection) -> DbResult<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "cache_size", "-8000")?; // 8MB page cache
    conn.pragma_update(None, "busy_timeout", "5000")?; // 5s busy timeout
    Ok(())
}

/// Format a DerivedKey's bytes as a SQLCipher raw hex key.
///
/// Returns the key in the format `x'<hex>'` which SQLCipher accepts
/// as a raw 256-bit key, bypassing its internal key derivation.
pub fn format_sqlcipher_key(key_bytes: &[u8; 32]) -> String {
    let hex: String = key_bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("x'{hex}'")
}
