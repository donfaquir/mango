//! Application-level pointer config, stored in Tauri's `app_data_dir` as
//! `config.json`. Holds the single piece of state that must survive a
//! workspace-relocation: the absolute path to the user-chosen workspace
//! directory (which itself contains `mango.db` + `projects/...`).
//!
//! Everything else lives in the workspace; this file is the only thing the
//! shell needs at boot to know which workspace to mount.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::{CoreError, Result};

pub const CONFIG_FILENAME: &str = "config.json";

/// Filename of the per-workspace metadata DB. Owned here so both the shell
/// (which mounts it) and the probe (which inspects candidate workspaces)
/// agree on the canonical name.
pub const METADATA_DB_FILENAME: &str = "mango.db";

/// Classification of a candidate workspace directory. Each variant drives a
/// different confirmation flow in the frontend onboarding / switch UI.
#[derive(Debug, Serialize, Deserialize, Type, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkspaceProbe {
    /// Path does not exist OR is an empty directory. Safe to initialise.
    Empty,
    /// Path contains `mango.db`. Reports the number of projects found so the
    /// UI can show "detected N projects, mount here?". Project count is
    /// best-effort — a corrupt DB still classifies as ExistingMangoData
    /// with `project_count = 0` rather than failing.
    ExistingMangoData {
        #[specta(type = specta_typescript::Number)]
        project_count: i64,
    },
    /// Path exists, is non-empty, but no `mango.db` was found. The UI should
    /// warn before initialising on top of unrelated files.
    NonEmptyForeign,
    /// Path is unreadable, not a directory, or otherwise unusable. The string
    /// is a human-readable reason for the UI to surface.
    Invalid {
        reason: String,
    },
}

/// Probe a candidate workspace directory. Pure inspection — no writes, no
/// state mutation. Always returns `Ok`; classification problems are encoded
/// in the returned variant rather than the outer Result so the IPC layer
/// doesn't have to distinguish "user picked a bad path" from "the probe
/// itself crashed".
pub fn probe_workspace(path: &Path) -> WorkspaceProbe {
    if !path.exists() {
        return WorkspaceProbe::Empty;
    }
    if !path.is_dir() {
        return WorkspaceProbe::Invalid {
            reason: "path exists but is not a directory".into(),
        };
    }

    let db_path = path.join(METADATA_DB_FILENAME);
    if db_path.is_file() {
        return WorkspaceProbe::ExistingMangoData {
            project_count: count_projects_best_effort(&db_path),
        };
    }

    let has_entries = match std::fs::read_dir(path) {
        Ok(mut iter) => iter.next().is_some(),
        Err(e) => {
            return WorkspaceProbe::Invalid {
                reason: format!("cannot read directory: {e}"),
            };
        }
    };
    if has_entries {
        WorkspaceProbe::NonEmptyForeign
    } else {
        WorkspaceProbe::Empty
    }
}

/// Counts rows in `project` if the DB is readable. Returns 0 on any failure
/// (missing table, locked file, corrupt header) — the probe still classifies
/// as ExistingMangoData since `mango.db` is present, and the count is purely
/// informational.
fn count_projects_best_effort(db_path: &Path) -> i64 {
    let conn = match rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    conn.query_row("SELECT COUNT(*) FROM project", [], |row| row.get(0))
        .unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    pub workspace_path: PathBuf,
}

fn config_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(CONFIG_FILENAME)
}

/// Read the pointer config. Returns `Ok(None)` when the file is missing —
/// callers treat that as "no workspace mounted yet". A malformed file is a
/// hard `Validation` error so the user sees a clear message rather than
/// silently re-onboarding (which would mask data loss).
pub fn read(app_data_dir: &Path) -> Result<Option<AppConfig>> {
    let path = config_path(app_data_dir);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(CoreError::Io(e)),
    };
    let cfg: AppConfig = serde_json::from_slice(&bytes).map_err(|e| {
        CoreError::Validation(format!("invalid {CONFIG_FILENAME}: {e}"))
    })?;
    Ok(Some(cfg))
}

/// Write the pointer config atomically (write-temp + rename) so a crash mid
/// write cannot leave the user with a half-truncated file that fails to
/// parse on the next launch.
pub fn write(app_data_dir: &Path, cfg: &AppConfig) -> Result<()> {
    std::fs::create_dir_all(app_data_dir)?;
    let final_path = config_path(app_data_dir);
    let tmp_path = final_path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(cfg).map_err(|e| {
        CoreError::Validation(format!("failed to serialize {CONFIG_FILENAME}: {e}"))
    })?;
    std::fs::write(&tmp_path, bytes)?;
    std::fs::rename(&tmp_path, &final_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn probe_nonexistent_path_is_empty() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("not-there");
        assert!(matches!(probe_workspace(&missing), WorkspaceProbe::Empty));
    }

    #[test]
    fn probe_empty_directory_is_empty() {
        let dir = tempdir().unwrap();
        assert!(matches!(probe_workspace(dir.path()), WorkspaceProbe::Empty));
    }

    #[test]
    fn probe_file_path_is_invalid() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("not-a-dir");
        std::fs::write(&file, b"x").unwrap();
        assert!(matches!(probe_workspace(&file), WorkspaceProbe::Invalid { .. }));
    }

    #[test]
    fn probe_directory_with_other_files_is_foreign() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("readme.txt"), b"hi").unwrap();
        assert!(matches!(
            probe_workspace(dir.path()),
            WorkspaceProbe::NonEmptyForeign
        ));
    }

    #[test]
    fn probe_directory_with_mango_db_reports_existing() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join(METADATA_DB_FILENAME);
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute("CREATE TABLE project (id TEXT PRIMARY KEY)", []).unwrap();
        conn.execute("INSERT INTO project (id) VALUES ('a'), ('b')", []).unwrap();
        drop(conn);

        match probe_workspace(dir.path()) {
            WorkspaceProbe::ExistingMangoData { project_count } => assert_eq!(project_count, 2),
            other => panic!("expected ExistingMangoData, got {other:?}"),
        }
    }

    #[test]
    fn probe_directory_with_garbage_mango_db_still_reports_existing() {
        // A non-SQLite file at the canonical name still indicates "user
        // thinks this is a workspace" — surface it as ExistingMangoData
        // with count 0 rather than silently downgrading to foreign.
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(METADATA_DB_FILENAME), b"not sqlite").unwrap();
        match probe_workspace(dir.path()) {
            WorkspaceProbe::ExistingMangoData { project_count } => assert_eq!(project_count, 0),
            other => panic!("expected ExistingMangoData, got {other:?}"),
        }
    }

    #[test]
    fn read_missing_returns_none() {
        let dir = tempdir().unwrap();
        assert!(read(dir.path()).unwrap().is_none());
    }

    #[test]
    fn write_then_read_round_trip() {
        let dir = tempdir().unwrap();
        let cfg = AppConfig {
            workspace_path: PathBuf::from("/some/abs/workspace"),
        };
        write(dir.path(), &cfg).unwrap();
        let loaded = read(dir.path()).unwrap().unwrap();
        assert_eq!(loaded, cfg);
    }

    #[test]
    fn write_creates_parent_directory() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("not-yet-existing");
        let cfg = AppConfig {
            workspace_path: PathBuf::from("/x"),
        };
        write(&nested, &cfg).unwrap();
        assert!(nested.join(CONFIG_FILENAME).is_file());
    }

    #[test]
    fn read_malformed_returns_validation_error() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(CONFIG_FILENAME), b"not json").unwrap();
        match read(dir.path()) {
            Err(CoreError::Validation(msg)) => assert!(msg.contains(CONFIG_FILENAME)),
            other => panic!("expected Validation error, got {other:?}"),
        }
    }

    #[test]
    fn write_replaces_existing_file_atomically() {
        let dir = tempdir().unwrap();
        let cfg1 = AppConfig {
            workspace_path: PathBuf::from("/a"),
        };
        let cfg2 = AppConfig {
            workspace_path: PathBuf::from("/b"),
        };
        write(dir.path(), &cfg1).unwrap();
        write(dir.path(), &cfg2).unwrap();
        assert_eq!(read(dir.path()).unwrap().unwrap(), cfg2);
        // The .tmp sidecar must not linger.
        assert!(!dir.path().join("config.json.tmp").exists());
    }
}
