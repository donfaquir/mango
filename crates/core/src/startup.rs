//! Unified startup hooks called by both the Tauri shell and the CLI binary.
//!
//! Anything that must run once per process before the DB is exposed to
//! application code lives here, so the GUI and CLI never drift out of sync.
//! Failure is fatal at startup: leaving the DB in an inconsistent state and
//! crashing later under user actions is much harder to debug than refusing
//! to boot with a clear error in the log.

use std::path::Path;

use rusqlite::{params, Connection};

use crate::error::{CoreError, Result};
use crate::paths;

/// Run all startup hooks. Currently:
/// 1. Backfill `project.root_path` for rows with NULL (legacy MS0 data).
/// 2. Apply Provider/Model seed rows (kling / jimeng + their models).
pub fn initialize(conn: &Connection, app_data_dir: &Path) -> Result<()> {
    backfill_project_roots(conn, app_data_dir)?;
    crate::seed::providers::apply(conn)?;
    Ok(())
}

/// Assign the convention path to every project row whose `root_path` is NULL
/// and create the on-disk layout. Any failure aborts startup — see module doc.
pub fn backfill_project_roots(conn: &Connection, app_data_dir: &Path) -> Result<()> {
    let ids: Vec<String> = {
        let mut stmt = conn.prepare("SELECT id FROM project WHERE root_path IS NULL")?;
        stmt.query_map([], |row| row.get(0))?
            .collect::<std::result::Result<_, _>>()?
    };

    for id in &ids {
        let root = paths::convention_root(app_data_dir, id);
        paths::ensure_project_layout(&root).map_err(|e| {
            CoreError::Validation(format!(
                "backfill: failed to create layout for {id} at {}: {e}",
                root.display()
            ))
        })?;
        conn.execute(
            "UPDATE project SET root_path = ?1 WHERE id = ?2",
            params![root.to_string_lossy(), id],
        )?;
    }

    if !ids.is_empty() {
        tracing::info!("backfilled root_path for {} project(s)", ids.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use std::path::Path as StdPath;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn conn() -> Connection {
        open_sync(StdPath::new(":memory:")).unwrap()
    }

    #[test]
    fn backfill_assigns_root_and_creates_layout() {
        let conn = conn();
        let app_data = tempdir().unwrap();

        let id = Uuid::new_v4().to_string();
        // Insert a row directly with NULL root_path to simulate legacy data.
        conn.execute(
            "INSERT INTO project (id, name, root_path) VALUES (?1, ?2, NULL)",
            params![id, "legacy"],
        )
        .unwrap();

        backfill_project_roots(&conn, app_data.path()).unwrap();

        let root: String = conn
            .query_row(
                "SELECT root_path FROM project WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        let expected = paths::convention_root(app_data.path(), &id);
        assert_eq!(StdPath::new(&root), expected);
        assert!(paths::assets_dir(&expected).is_dir());
        assert!(paths::thumbnails_dir(&expected).is_dir());
    }

    #[test]
    fn backfill_is_noop_when_no_null_rows() {
        let conn = conn();
        let app_data = tempdir().unwrap();
        // No INSERT — table is empty.
        backfill_project_roots(&conn, app_data.path()).unwrap();
    }

    #[test]
    fn initialize_runs_backfill() {
        let conn = conn();
        let app_data = tempdir().unwrap();

        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO project (id, name, root_path) VALUES (?1, ?2, NULL)",
            params![id, "x"],
        )
        .unwrap();

        initialize(&conn, app_data.path()).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM project WHERE root_path IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }
}
