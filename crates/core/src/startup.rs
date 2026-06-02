//! Unified startup hooks called by both the Tauri shell and the CLI binary.
//!
//! Anything that must run once per process before the DB is exposed to
//! application code lives here, so the GUI and CLI never drift out of sync.
//! Failure is fatal at startup: leaving the DB in an inconsistent state and
//! crashing later under user actions is much harder to debug than refusing
//! to boot with a clear error in the log.

use rusqlite::Connection;

use crate::error::Result;

/// Run all startup hooks. Currently:
/// 1. Apply Provider/Model seed rows (kling / jimeng + their models).
/// 2. Reset orphan `running` generation_task rows back to `pending` — the
///    runner coroutines that owned them died with the previous process.
///
/// The workspace-pointer indirection means a mounted DB always has a known
/// absolute workspace root, so the legacy `backfill_project_roots` step
/// (which used to fill NULL `root_path` rows with a convention path under
/// `app_data_dir`) is no longer needed and has been removed.
pub fn initialize(conn: &Connection) -> Result<()> {
    crate::seed::providers::apply(conn)?;
    let n = crate::db::queries::generation_task::reset_orphan_running(conn)?;
    if n > 0 {
        tracing::warn!("reset {n} orphan running task(s) to pending after restart");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use rusqlite::params;
    use std::path::Path as StdPath;

    fn conn() -> Connection {
        open_sync(StdPath::new(":memory:")).unwrap()
    }

    #[test]
    fn initialize_seeds_providers() {
        let conn = conn();
        initialize(&conn).unwrap();
        // Seed apply is idempotent — a second call must not error.
        initialize(&conn).unwrap();
    }

    #[test]
    fn initialize_resets_orphan_running() {
        let conn = conn();

        // Seed minimum FK chain for a generation_task row.
        conn.execute("INSERT INTO provider (id, name) VALUES ('p1', 'P1')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO model (id, provider_id, name, model_type) \
             VALUES ('m1', 'p1', 'M', 'image')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO api_account (id, provider_id, label, api_key_ref, key_last4) \
             VALUES ('a1', 'p1', 'L', 'api_account:a1', '0000')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO generation_task \
                (id, provider_id, model_id, account_id, task_type, status) \
             VALUES ('t1', 'p1', 'm1', 'a1', 'image', 'running')",
            [],
        )
        .unwrap();

        initialize(&conn).unwrap();

        let (status, retry, err): (String, i64, Option<String>) = conn
            .query_row(
                "SELECT status, retry_count, error_message FROM generation_task WHERE id = 't1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(status, "pending");
        assert_eq!(retry, 1);
        assert!(err.unwrap_or_default().contains("orphan reset on startup"));

        // params is imported only to silence unused-import warnings if any
        // downstream test removes its own usages; keep it referenced here.
        let _ = params![1];
    }
}
