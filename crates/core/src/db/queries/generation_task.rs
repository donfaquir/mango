use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::generation_task::{
    CreateGenerationTaskInput, GenerationTask, GenerationTaskStatus, TaskKind,
};

const SELECT_COLUMNS: &str = "id, shot_id, provider_id, model_id, account_id, task_type, \
                              params_json, status, result_asset_id, external_task_id, \
                              started_at, finished_at, error_message, retry_count, created_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<GenerationTask> {
    let task_type_str: String = row.get(5)?;
    let status_str: String = row.get(7)?;
    Ok(GenerationTask {
        id: row.get(0)?,
        shot_id: row.get(1)?,
        provider_id: row.get(2)?,
        model_id: row.get(3)?,
        account_id: row.get(4)?,
        task_type: TaskKind::from_db_str(&task_type_str).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                5,
                rusqlite::types::Type::Text,
                Box::new(InvalidEnumValue(task_type_str.clone())),
            )
        })?,
        params_json: row.get(6)?,
        status: GenerationTaskStatus::from_db_str(&status_str).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                7,
                rusqlite::types::Type::Text,
                Box::new(InvalidEnumValue(status_str.clone())),
            )
        })?,
        result_asset_id: row.get(8)?,
        external_task_id: row.get(9)?,
        started_at: row.get(10)?,
        finished_at: row.get(11)?,
        error_message: row.get(12)?,
        retry_count: row.get(13)?,
        created_at: row.get(14)?,
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
            (id, shot_id, provider_id, model_id, account_id, task_type, params_json, status) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending')",
        params![
            id,
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

    // project_id requires joining shot → episode → project; rows with shot_id=NULL
    // are filtered out when project_id is set (they have no project association).
    let (sql, has_project, has_status) = match (project_id.is_some(), status.is_some()) {
        (true, true) => (
            format!(
                "SELECT {prefixed} FROM generation_task t \
                 INNER JOIN shot s ON s.id = t.shot_id \
                 INNER JOIN episode e ON e.id = s.episode_id \
                 WHERE e.project_id = ?1 AND t.status = ?2 \
                 ORDER BY t.created_at DESC LIMIT ?3",
                prefixed = prefixed_columns()
            ),
            true,
            true,
        ),
        (true, false) => (
            format!(
                "SELECT {prefixed} FROM generation_task t \
                 INNER JOIN shot s ON s.id = t.shot_id \
                 INNER JOIN episode e ON e.id = s.episode_id \
                 WHERE e.project_id = ?1 \
                 ORDER BY t.created_at DESC LIMIT ?2",
                prefixed = prefixed_columns()
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

fn prefixed_columns() -> String {
    SELECT_COLUMNS
        .split(", ")
        .map(|c| format!("t.{c}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Apply a state-machine transition. Allowed:
///   pending → running | cancelled
///   running → success | failed | cancelled
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
        (Pending, Running) | (Pending, Cancelled) | (Running, Success) | (Running, Failed) | (Running, Cancelled)
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

/// Reset all rows whose `status='running'` to `pending` on startup. The previous
/// app process owned a runner coroutine that no longer exists, so the row is
/// orphaned. Marker text is appended to `error_message` and `retry_count`
/// is incremented for audit. `external_task_id` is preserved so the user can
/// see what was running and decide whether to resubmit.
pub fn reset_orphan_running(conn: &Connection) -> Result<usize> {
    let n = conn.execute(
        "UPDATE generation_task \
         SET status = 'pending', \
             error_message = COALESCE(error_message, '') || '[orphan reset on startup]', \
             retry_count = retry_count + 1 \
         WHERE status = 'running'",
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

    fn seed_project_episode_shot(conn: &Connection, project_id: &str) -> String {
        let shot_id = format!("shot-{}", Uuid::new_v4());
        let episode_id = format!("ep-{}", Uuid::new_v4());
        conn.execute(
            "INSERT INTO project (id, name) VALUES (?1, 'P')",
            params![project_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO episode (id, project_id, title) VALUES (?1, ?2, 'E')",
            params![episode_id, project_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO shot (id, episode_id) VALUES (?1, ?2)",
            params![shot_id, episode_id],
        )
        .unwrap();
        shot_id
    }

    fn make_input(provider: &str, model: &str, account: &str) -> CreateGenerationTaskInput {
        CreateGenerationTaskInput {
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

        assert_eq!(running_after.status, GenerationTaskStatus::Pending);
        assert_eq!(running_after.retry_count, 1);
        assert!(running_after
            .error_message
            .as_deref()
            .unwrap_or("")
            .contains("orphan reset on startup"));

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
    fn list_filter_by_project_joins_through_shot() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let (p, m, a) = seed_provider_chain(&conn);
        let shot_a = seed_project_episode_shot(&conn, "proj-a");
        let shot_b = seed_project_episode_shot(&conn, "proj-b");

        // Two tasks in proj-a, one in proj-b, one without shot.
        let mut input_a1 = make_input(&p, &m, &a);
        input_a1.shot_id = Some(shot_a.clone());
        create(&conn, input_a1).unwrap();
        let mut input_a2 = make_input(&p, &m, &a);
        input_a2.shot_id = Some(shot_a.clone());
        create(&conn, input_a2).unwrap();
        let mut input_b = make_input(&p, &m, &a);
        input_b.shot_id = Some(shot_b);
        create(&conn, input_b).unwrap();
        create(&conn, make_input(&p, &m, &a)).unwrap();

        let in_a = list(&conn, Some("proj-a"), None, None).unwrap();
        assert_eq!(in_a.len(), 2);
        let in_b = list(&conn, Some("proj-b"), None, None).unwrap();
        assert_eq!(in_b.len(), 1);
    }
}
