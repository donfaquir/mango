//! Key/value preferences persisted across app restarts. Values are stored as
//! JSON strings so heterogeneous types share one table (`task.max_concurrency`
//! is an i64; future entries may be objects).
//!
//! Currently consumed by the task engine to seed/save `task.max_concurrency`
//! (spec-27). New keys should pick a `namespace.key` form so a future settings
//! UI can group them.

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{CoreError, Result};

pub fn get_i64(conn: &Connection, key: &str) -> Result<Option<i64>> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT value_json FROM app_preference WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()?;
    let Some(s) = raw else { return Ok(None) };
    let parsed: serde_json::Value = serde_json::from_str(&s).map_err(|e| {
        CoreError::Validation(format!(
            "app_preference {key} is not valid JSON: {e}"
        ))
    })?;
    let n = parsed.as_i64().ok_or_else(|| {
        CoreError::Validation(format!(
            "app_preference {key} is {parsed}, expected an integer"
        ))
    })?;
    Ok(Some(n))
}

pub fn set_i64(conn: &Connection, key: &str, value: i64) -> Result<()> {
    let json = serde_json::Value::from(value).to_string();
    conn.execute(
        "INSERT INTO app_preference (key, value_json) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
        params![key, json],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use std::path::Path;

    #[test]
    fn default_concurrency_seeded_to_3() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        // The migrator seeds the row at install; we re-read to confirm.
        let n = get_i64(&conn, "task.max_concurrency").unwrap();
        assert_eq!(n, Some(3));
    }

    #[test]
    fn get_unknown_key_returns_none() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let n = get_i64(&conn, "no.such.key").unwrap();
        assert!(n.is_none());
    }

    #[test]
    fn set_then_get_roundtrip() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        set_i64(&conn, "task.max_concurrency", 6).unwrap();
        assert_eq!(get_i64(&conn, "task.max_concurrency").unwrap(), Some(6));
        // Upsert overwrites in-place.
        set_i64(&conn, "task.max_concurrency", 2).unwrap();
        assert_eq!(get_i64(&conn, "task.max_concurrency").unwrap(), Some(2));
    }

    #[test]
    fn get_rejects_non_integer_payload() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        conn.execute(
            "INSERT INTO app_preference (key, value_json) \
             VALUES ('bogus', '\"text\"') ON CONFLICT DO UPDATE SET value_json='\"text\"'",
            [],
        )
        .unwrap();
        let r = get_i64(&conn, "bogus");
        assert!(matches!(r, Err(CoreError::Validation(_))));
    }
}
