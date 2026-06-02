use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::generation_task::{
    CreateGenerationTaskInput, GenerationTask, GenerationTaskStatus, TaskKind,
};

const SELECT_COLUMNS: &str = "id, project_id, shot_id, provider_id, model_id, account_id, task_type, \
                              params_json, status, result_asset_id, external_task_id, \
                              started_at, finished_at, error_message, retry_count, created_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<GenerationTask> {
    let task_type_str: String = row.get(6)?;
    let status_str: String = row.get(8)?;
    Ok(GenerationTask {
        id: row.get(0)?,
        project_id: row.get(1)?,
        shot_id: row.get(2)?,
        provider_id: row.get(3)?,
        model_id: row.get(4)?,
        account_id: row.get(5)?,
        task_type: TaskKind::from_db_str(&task_type_str).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                6,
                rusqlite::types::Type::Text,
                Box::new(InvalidEnumValue(task_type_str.clone())),
            )
        })?,
        params_json: row.get(7)?,
        status: GenerationTaskStatus::from_db_str(&status_str).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                8,
                rusqlite::types::Type::Text,
                Box::new(InvalidEnumValue(status_str.clone())),
            )
        })?,
        result_asset_id: row.get(9)?,
        external_task_id: row.get(10)?,
        started_at: row.get(11)?,
        finished_at: row.get(12)?,
        error_message: row.get(13)?,
        retry_count: row.get(14)?,
        created_at: row.get(15)?,
    })
}

#[derive(Debug)]
struct InvalidEnumValue(String);

impl std::fmt::Display for InvalidEnumValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid enum value in DB: {}", self.0)
    }
}

impl std::error::Error for InvalidEnumValue {}

pub fn create(conn: &Connection, input: CreateGenerationTaskInput) -> Result<GenerationTask> {
    let params_json = match input.params_json.as_deref() {
        None | Some("") => "{}".to_string(),
        Some(raw) => {
            serde_json::from_str::<serde_json::Value>(raw)
                .map_err(|e| CoreError::Validation(format!("params_json is not valid JSON: {e}")))?;
            raw.to_string()
        }
    };

    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO generation_task \
            (id, project_id, shot_id, provider_id, model_id, account_id, task_type, params_json, status) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending')",
        params![
            id,
            input.project_id,
            input.shot_id,
            input.provider_id,
            input.model_id,
            input.account_id,
            input.task_type.as_str(),
            params_json,
        ],
    )?;
    get_by_id(conn, &id)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<GenerationTask> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM generation_task WHERE id = ?1"),
        params![id],
        map_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
            entity: "generation_task",
            id: id.to_string(),
        },
        other => CoreError::Sqlite(other),
    })
}

pub fn list(
    conn: &Connection,
    project_id: Option<&str>,
    status: Option<GenerationTaskStatus>,
    limit: Option<u32>,
) -> Result<Vec<GenerationTask>> {
    let limit = limit.unwrap_or(100).clamp(1, 500);

    // Filter directly by project_id column; no JOIN needed.
    let (sql, has_project, has_status) = match (project_id.is_some(), status.is_some()) {
        (true, true) => (
            format!(
                "SELECT {SELECT_COLUMNS} FROM generation_task \
                 WHERE project_id = ?1 AND status = ?2 \
                 ORDER BY created_at DESC LIMIT ?3"
            ),
            true,
            true,
        ),
        (true, false) => (
            format!(
                "SELECT {SELECT_COLUMNS} FROM generation_task \
                 WHERE project_id = ?1 \
                 ORDER BY created_at DESC LIMIT ?2"
            ),
            true,
            false,
        ),
        (false, true) => (
            format!(
                "SELECT {SELECT_COLUMNS} FROM generation_task \
                 WHERE status = ?1 ORDER BY created_at DESC LIMIT ?2"
            ),
            false,
            true,
        ),
        (false, false) => (
            format!(
                "SELECT {SELECT_COLUMNS} FROM generation_task \
                 ORDER BY created_at DESC LIMIT ?1"
            ),
            false,
            false,
        ),
    };

    let mut stmt = conn.prepare(&sql)?;
    let rows = match (has_project, has_status) {
        (true, true) => stmt.query_map(
            params![project_id.unwrap(), status.unwrap().as_str(), limit],
            map_row,
        )?,
        (true, false) => stmt.query_map(params![project_id.unwrap(), limit], map_row)?,
        (false, true) => stmt.query_map(params![status.unwrap().as_str(), limit], map_row)?,
        (false, false) => stmt.query_map(params![limit], map_row)?,
    };
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(CoreError::from)
}

/// Apply a state-machine transition. Allowed:
///   pending → running | failed | cancelled
///   running → success | failed | cancelled
/// `pending → failed` covers early failures the runner detects before it can
/// transition the task to running (credential resolution, param validation,
/// asset path resolution). Without this edge those failures are silently
/// dropped and the task stays pending forever.
/// Any other source state (terminal, or non-matching pair) returns
/// `CoreError::TaskEngine("invalid transition: ...")`.
///
/// `started_at` is filled when entering Running; `finished_at` when entering
/// any terminal state. `error_message` is only set when supplied (typically
/// for Failed transitions).
pub fn transition_status(
    conn: &Connection,
    id: &str,
    new_status: GenerationTaskStatus,
    error_message: Option<&str>,
) -> Result<()> {
    let current = get_by_id(conn, id)?;
    if !is_valid_transition(current.status, new_status) {
        return Err(CoreError::TaskEngine(format!(
            "invalid transition: {} → {}",
            current.status.as_str(),
            new_status.as_str()
        )));
    }

    let entering_running = matches!(new_status, GenerationTaskStatus::Running);
    let entering_terminal = new_status.is_terminal();

    let mut sets: Vec<String> = vec!["status = ?1".to_string()];
    if entering_running {
        sets.push("started_at = datetime('now')".to_string());
    }
    if entering_terminal {
        sets.push("finished_at = datetime('now')".to_string());
    }
    if error_message.is_some() {
        sets.push("error_message = ?2".to_string());
    }

    let sql = format!(
        "UPDATE generation_task SET {} WHERE id = ?{}",
        sets.join(", "),
        if error_message.is_some() { 3 } else { 2 }
    );

    let n = if let Some(msg) = error_message {
        conn.execute(&sql, params![new_status.as_str(), msg, id])?
    } else {
        conn.execute(&sql, params![new_status.as_str(), id])?
    };
    if n == 0 {
        return Err(CoreError::NotFound {
            entity: "generation_task",
            id: id.to_string(),
        });
    }
    Ok(())
}

fn is_valid_transition(from: GenerationTaskStatus, to: GenerationTaskStatus) -> bool {
    use GenerationTaskStatus::*;
    matches!(
        (from, to),
        (Pending, Running)
            | (Pending, Failed)
            | (Pending, Cancelled)
            | (Running, Success)
            | (Running, Failed)
            | (Running, Cancelled)
    )
}

pub fn set_external_id(conn: &Connection, id: &str, external_id: &str) -> Result<()> {
    let n = conn.execute(
        "UPDATE generation_task SET external_task_id = ?1 WHERE id = ?2",
        params![external_id, id],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound {
            entity: "generation_task",
            id: id.to_string(),
        });
    }
    Ok(())
}

pub fn set_result_asset_id(conn: &Connection, id: &str, asset_id: &str) -> Result<()> {
    let n = conn.execute(
        "UPDATE generation_task SET result_asset_id = ?1 WHERE id = ?2",
        params![asset_id, id],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound {
            entity: "generation_task",
            id: id.to_string(),
        });
    }
    Ok(())
}

/// Overwrite the `params_json` column verbatim. The caller is responsible for
/// merging in fields like `_internal.uploaded_remote_ids` — this helper is the
/// dumbest possible writer so it can't mistake the caller's intent.
pub fn set_params_json(conn: &Connection, id: &str, params_json: &str) -> Result<()> {
    let n = conn.execute(
        "UPDATE generation_task SET params_json = ?1 WHERE id = ?2",
        params![params_json, id],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound {
            entity: "generation_task",
            id: id.to_string(),
        });
    }
    Ok(())
}

/// Return IDs of all tasks still in `pending` status (used for recovery on startup).
pub fn list_pending_ids(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id FROM generation_task WHERE status = 'pending'")?;
    let ids = stmt
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(ids)
}

/// Reset all rows whose `status='running'` to `pending` on startup. The
/// previous app process owned a runner coroutine that no longer exists, so
/// the row is orphaned and `recover_pending` will re-spawn a runner for it.
///
/// `external_task_id` is preserved on purpose: if it's `Some`, the runner's
/// resume path (in `task_engine::runner::run`) skips re-submission and
/// jumps straight into the poll loop with the existing id, so neither the
/// cloud nor the user's quota are double-charged. If it's `None` (orphan
/// happened before the provider submit ever returned), the runner does
/// submit afresh — that's the normal "retry the un-submitted" path.
///
/// We don't bump `retry_count` or pollute `error_message` here: this is
/// the routine recovery flow on every restart, not a failure. Failures
/// still drive both fields via `transition_status` from the runner.
pub fn reset_orphan_running(conn: &Connection) -> Result<usize> {
    let n = conn.execute(
        "UPDATE generation_task SET status = 'pending' WHERE status = 'running'",
        [],
    )?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use std::path::Path;

    /// Seed minimal provider + model + api_account rows so FK constraints pass.
    /// Returns (provider_id, model_id, account_id).
    fn seed_provider_chain(conn: &Connection) -> (String, String, String) {
        conn.execute(
            "INSERT INTO provider (id, name) VALUES ('prov1', 'Provider1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO model (id, provider_id, name, model_type) \
             VALUES ('mod1', 'prov1', 'M1', 'image')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO api_account (id, provider_id, label, api_key_ref, key_last4) \
             VALUES ('acc1', 'prov1', 'L', 'api_account:acc1', '1234')",
            [],
        )
        .unwrap();
        ("prov1".into(), "mod1".into(), "acc1".into())
    }

    fn make_input(provider: &str, model: &str, account: &str) -> CreateGenerationTaskInput {
        CreateGenerationTaskInput {
            project_id: None,
            shot_id: None,
            provider_id: provider.into(),
            model_id: model.into(),
            account_id: account.into(),
            task_type: TaskKind::Image,
            params_json: None,
        }
    }

    #[test]
    fn create_then_get_roundtrip() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let (p, m, a) = seed_provider_chain(&conn);
        let task = create(&conn, make_input(&p, &m, &a)).unwrap();
        assert_eq!(task.status, GenerationTaskStatus::Pending);
        assert_eq!(task.task_type, TaskKind::Image);
        assert_eq!(task.params_json, "{}");
        assert_eq!(task.retry_count, 0);
        assert!(task.started_at.is_none());

        let fetched = get_by_id(&conn, &task.id).unwrap();
        assert_eq!(fetched.id, task.id);
    }

    #[test]
    fn create_rejects_invalid_json() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let (p, m, a) = seed_provider_chain(&conn);
        let mut input = make_input(&p, &m, &a);
        input.params_json = Some("not json".into());
        assert!(matches!(create(&conn, input), Err(CoreError::Validation(_))));
    }

    #[test]
    fn get_by_id_returns_not_found() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        assert!(matches!(
            get_by_id(&conn, "missing"),
            Err(CoreError::NotFound { .. })
        ));
    }

    #[test]
    fn transition_pending_to_running_sets_started_at() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let (p, m, a) = seed_provider_chain(&conn);
        let task = create(&conn, make_input(&p, &m, &a)).unwrap();

        transition_status(&conn, &task.id, GenerationTaskStatus::Running, None).unwrap();
        let after = get_by_id(&conn, &task.id).unwrap();
        assert_eq!(after.status, GenerationTaskStatus::Running);
        assert!(after.started_at.is_some());
        assert!(after.finished_at.is_none());
    }

    #[test]
    fn transition_running_to_failed_records_error_and_finished_at() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let (p, m, a) = seed_provider_chain(&conn);
        let task = create(&conn, make_input(&p, &m, &a)).unwrap();
        transition_status(&conn, &task.id, GenerationTaskStatus::Running, None).unwrap();

        transition_status(
            &conn,
            &task.id,
            GenerationTaskStatus::Failed,
            Some("boom"),
        )
        .unwrap();
        let after = get_by_id(&conn, &task.id).unwrap();
        assert_eq!(after.status, GenerationTaskStatus::Failed);
        assert_eq!(after.error_message.as_deref(), Some("boom"));
        assert!(after.finished_at.is_some());
    }

    #[test]
    fn transition_pending_to_cancelled_is_allowed() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let (p, m, a) = seed_provider_chain(&conn);
        let task = create(&conn, make_input(&p, &m, &a)).unwrap();
        transition_status(&conn, &task.id, GenerationTaskStatus::Cancelled, None).unwrap();
        let after = get_by_id(&conn, &task.id).unwrap();
        assert_eq!(after.status, GenerationTaskStatus::Cancelled);
    }

    /// Regression: an early failure in the runner (credential / param /
    /// asset path resolution) must be allowed to transition the still-pending
    /// task straight to failed. Previously this edge was missing and tasks
    /// stayed pending forever.
    #[test]
    fn transition_pending_to_failed_is_allowed() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let (p, m, a) = seed_provider_chain(&conn);
        let task = create(&conn, make_input(&p, &m, &a)).unwrap();

        transition_status(
            &conn,
            &task.id,
            GenerationTaskStatus::Failed,
            Some("asset not found"),
        )
        .unwrap();
        let after = get_by_id(&conn, &task.id).unwrap();
        assert_eq!(after.status, GenerationTaskStatus::Failed);
        assert_eq!(after.error_message.as_deref(), Some("asset not found"));
        assert!(after.finished_at.is_some());
        assert!(after.started_at.is_none());
    }

    #[test]
    fn transition_terminal_to_anything_is_rejected() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let (p, m, a) = seed_provider_chain(&conn);
        let task = create(&conn, make_input(&p, &m, &a)).unwrap();
        transition_status(&conn, &task.id, GenerationTaskStatus::Running, None).unwrap();
        transition_status(&conn, &task.id, GenerationTaskStatus::Success, None).unwrap();

        let r = transition_status(&conn, &task.id, GenerationTaskStatus::Failed, None);
        assert!(matches!(r, Err(CoreError::TaskEngine(_))));
    }

    #[test]
    fn transition_pending_to_success_is_rejected() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let (p, m, a) = seed_provider_chain(&conn);
        let task = create(&conn, make_input(&p, &m, &a)).unwrap();
        let r = transition_status(&conn, &task.id, GenerationTaskStatus::Success, None);
        assert!(matches!(r, Err(CoreError::TaskEngine(_))));
    }

    #[test]
    fn set_external_id_updates_row() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let (p, m, a) = seed_provider_chain(&conn);
        let task = create(&conn, make_input(&p, &m, &a)).unwrap();
        set_external_id(&conn, &task.id, "ext-123").unwrap();
        let after = get_by_id(&conn, &task.id).unwrap();
        assert_eq!(after.external_task_id.as_deref(), Some("ext-123"));
    }

    #[test]
    fn reset_orphan_running_only_touches_running_rows() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let (p, m, a) = seed_provider_chain(&conn);
        // pending row, running row, success row.
        let pending = create(&conn, make_input(&p, &m, &a)).unwrap();
        let running = create(&conn, make_input(&p, &m, &a)).unwrap();
        transition_status(&conn, &running.id, GenerationTaskStatus::Running, None).unwrap();
        let success = create(&conn, make_input(&p, &m, &a)).unwrap();
        transition_status(&conn, &success.id, GenerationTaskStatus::Running, None).unwrap();
        transition_status(&conn, &success.id, GenerationTaskStatus::Success, None).unwrap();

        let n = reset_orphan_running(&conn).unwrap();
        assert_eq!(n, 1);

        let pending_after = get_by_id(&conn, &pending.id).unwrap();
        let running_after = get_by_id(&conn, &running.id).unwrap();
        let success_after = get_by_id(&conn, &success.id).unwrap();

        assert_eq!(pending_after.status, GenerationTaskStatus::Pending);
        assert_eq!(pending_after.retry_count, 0);

        // Recovery is routine — running rows just get nudged back to
        // pending without bumping retry_count or polluting error_message.
        // The runner's resume path then re-attaches to the cloud-side
        // job via the preserved external_task_id (if any).
        assert_eq!(running_after.status, GenerationTaskStatus::Pending);
        assert_eq!(running_after.retry_count, 0);
        assert!(running_after.error_message.is_none());

        assert_eq!(success_after.status, GenerationTaskStatus::Success);
    }

    #[test]
    fn list_filter_by_status_and_limit() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let (p, m, a) = seed_provider_chain(&conn);
        for _ in 0..3 {
            create(&conn, make_input(&p, &m, &a)).unwrap();
        }
        let all = list(&conn, None, None, None).unwrap();
        assert_eq!(all.len(), 3);

        let only_pending = list(&conn, None, Some(GenerationTaskStatus::Pending), None).unwrap();
        assert_eq!(only_pending.len(), 3);

        let limited = list(&conn, None, None, Some(2)).unwrap();
        assert_eq!(limited.len(), 2);
    }

    #[test]
    fn list_filter_by_project_uses_project_id_column() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let (p, m, a) = seed_provider_chain(&conn);
        // Seed two projects
        conn.execute(
            "INSERT INTO project (id, name) VALUES ('proj-a', 'PA')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO project (id, name) VALUES ('proj-b', 'PB')",
            [],
        )
        .unwrap();

        // Two tasks in proj-a, one in proj-b, one without project.
        let mut input_a1 = make_input(&p, &m, &a);
        input_a1.project_id = Some("proj-a".into());
        create(&conn, input_a1).unwrap();
        let mut input_a2 = make_input(&p, &m, &a);
        input_a2.project_id = Some("proj-a".into());
        create(&conn, input_a2).unwrap();
        let mut input_b = make_input(&p, &m, &a);
        input_b.project_id = Some("proj-b".into());
        create(&conn, input_b).unwrap();
        create(&conn, make_input(&p, &m, &a)).unwrap();

        let in_a = list(&conn, Some("proj-a"), None, None).unwrap();
        assert_eq!(in_a.len(), 2);
        let in_b = list(&conn, Some("proj-b"), None, None).unwrap();
        assert_eq!(in_b.len(), 1);
    }
}
