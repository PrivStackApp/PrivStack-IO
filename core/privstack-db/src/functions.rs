//! Custom SQL functions registered on SQLite connections.

use crate::error::DbResult;
use rusqlite::functions::FunctionFlags;
use rusqlite::Connection;

/// Register all custom SQL functions on the given connection.
///
/// Currently registers:
/// - `cosine_similarity(a, b)`: Computes cosine similarity between two
///   JSON-encoded float arrays. Returns 0.0 for mismatched dimensions or
///   zero-magnitude vectors.
pub fn register_custom_functions(conn: &Connection) -> DbResult<()> {
    register_cosine_similarity(conn)?;
    Ok(())
}

fn register_cosine_similarity(conn: &Connection) -> DbResult<()> {
    conn.create_scalar_function(
        "cosine_similarity",
        2,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let a_json: String = ctx.get(0)?;
            let b_json: String = ctx.get(1)?;

            let a: Vec<f64> = serde_json::from_str(&a_json).unwrap_or_default();
            let b: Vec<f64> = serde_json::from_str(&b_json).unwrap_or_default();

            if a.len() != b.len() || a.is_empty() {
                return Ok(0.0f64);
            }

            let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
            let mag_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
            let mag_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();

            if mag_a == 0.0 || mag_b == 0.0 {
                Ok(0.0f64)
            } else {
                Ok(dot / (mag_a * mag_b))
            }
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        register_custom_functions(&conn).unwrap();
        conn
    }

    #[test]
    fn cosine_similarity_identical_vectors() {
        let conn = setup_conn();
        let score: f64 = conn
            .query_row(
                "SELECT cosine_similarity('[1.0, 0.0, 0.0]', '[1.0, 0.0, 0.0]')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!((score - 1.0).abs() < 1e-10);
    }

    #[test]
    fn cosine_similarity_orthogonal_vectors() {
        let conn = setup_conn();
        let score: f64 = conn
            .query_row(
                "SELECT cosine_similarity('[1.0, 0.0]', '[0.0, 1.0]')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(score.abs() < 1e-10);
    }

    #[test]
    fn cosine_similarity_opposite_vectors() {
        let conn = setup_conn();
        let score: f64 = conn
            .query_row(
                "SELECT cosine_similarity('[1.0, 0.0]', '[-1.0, 0.0]')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!((score - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn cosine_similarity_mismatched_dimensions() {
        let conn = setup_conn();
        let score: f64 = conn
            .query_row(
                "SELECT cosine_similarity('[1.0, 2.0]', '[1.0, 2.0, 3.0]')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(score, 0.0);
    }

    #[test]
    fn cosine_similarity_empty_vectors() {
        let conn = setup_conn();
        let score: f64 = conn
            .query_row("SELECT cosine_similarity('[]', '[]')", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(score, 0.0);
    }

    #[test]
    fn cosine_similarity_zero_magnitude() {
        let conn = setup_conn();
        let score: f64 = conn
            .query_row(
                "SELECT cosine_similarity('[0.0, 0.0]', '[1.0, 1.0]')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(score, 0.0);
    }

    #[test]
    fn cosine_similarity_known_value() {
        let conn = setup_conn();
        // cos([1,2,3], [4,5,6]) = (4+10+18) / (sqrt(14) * sqrt(77)) ≈ 0.9746
        let score: f64 = conn
            .query_row(
                "SELECT cosine_similarity('[1.0, 2.0, 3.0]', '[4.0, 5.0, 6.0]')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!((score - 0.9746318461970762).abs() < 1e-10);
    }
}
