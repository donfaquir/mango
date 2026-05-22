use rusqlite::{params, Connection};

use crate::error::{CoreError, Result};
use crate::models::api_account::ApiAccount;

/// `api_key_ref` is intentionally omitted — it must never reach the IPC layer.
/// The column stays in the schema as an audit copy and is only managed by
/// `account::service`.
const SELECT_COLUMNS: &str = "id, provider_id, label, key_last4, \
                              usage_quota, usage_used, last_used_at, created_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<ApiAccount> {
    Ok(ApiAccount {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        label: row.get(2)?,
        key_last4: row.get(3)?,
        usage_quota: row.get(4)?,
        usage_used: row.get(5)?,
        last_used_at: row.get(6)?,
        created_at: row.get(7)?,
    })
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<ApiAccount> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM api_account WHERE id = ?1"),
        params![id],
        map_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
            entity: "api_account",
            id: id.to_string(),
        },
        other => CoreError::Sqlite(other),
    })
}

pub fn list(conn: &Connection, provider_id: Option<String>) -> Result<Vec<ApiAccount>> {
    if let Some(pid) = provider_id {
        let mut stmt = conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM api_account \
             WHERE provider_id = ?1 ORDER BY created_at DESC"
        ))?;
        let rows = stmt.query_map(params![pid], map_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(CoreError::from)
    } else {
        let mut stmt = conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM api_account ORDER BY created_at DESC"
        ))?;
        let rows = stmt.query_map([], map_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(CoreError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use std::path::Path;

    fn conn_with_provider() -> Connection {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        conn.execute(
            "INSERT INTO provider (id, name) VALUES ('p1', 'Provider1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO provider (id, name) VALUES ('p2', 'Provider2')",
            [],
        )
        .unwrap();
        conn
    }

    fn insert_raw(conn: &Connection, id: &str, provider_id: &str, last4: &str) {
        conn.execute(
            "INSERT INTO api_account (id, provider_id, label, api_key_ref, key_last4) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, provider_id, "label", format!("api_account:{id}"), last4],
        )
        .unwrap();
    }

    #[test]
    fn get_by_id_returns_not_found_when_missing() {
        let conn = conn_with_provider();
        assert!(matches!(
            get_by_id(&conn, "missing"),
            Err(CoreError::NotFound { .. })
        ));
    }

    #[test]
    fn list_filters_by_provider() {
        let conn = conn_with_provider();
        insert_raw(&conn, "a1", "p1", "1234");
        insert_raw(&conn, "a2", "p1", "5678");
        insert_raw(&conn, "a3", "p2", "9012");

        let all = list(&conn, None).unwrap();
        assert_eq!(all.len(), 3);

        let only_p1 = list(&conn, Some("p1".into())).unwrap();
        assert_eq!(only_p1.len(), 2);
        assert!(only_p1.iter().all(|a| a.provider_id == "p1"));
    }

    #[test]
    fn select_does_not_expose_api_key_ref() {
        // Confirm by reflection: the SELECT_COLUMNS string must not mention
        // api_key_ref. This is a guardrail against accidentally leaking the
        // backend-only column out through the IPC boundary.
        assert!(!SELECT_COLUMNS.contains("api_key_ref"));
    }
}
