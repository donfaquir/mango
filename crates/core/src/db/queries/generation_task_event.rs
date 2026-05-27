use rusqlite::{params, Connection};

use crate::error::{CoreError, Result};
use crate::models::generation_task_event::{EventPhase, EventSeverity, GenerationTaskEvent};

const SELECT_COLUMNS: &str = "id, task_id, occurred_at, phase, severity, \
                              request_id, http_status, details_json, message";

/// Hard cap on event rows per task. The poll loop runs every few seconds and
/// can produce a long tail of warn-level rows on a sticky transient failure;
/// without a cap a single stuck task could grow this table unbounded. Above
/// the cap [`insert`] silently drops with a `tracing::warn!` — the event
/// subsystem must never break the main flow.
const MAX_EVENTS_PER_TASK: i64 = 200;

#[derive(Debug)]
pub struct NewEvent<'a> {
    pub task_id: &'a str,
    pub phase: EventPhase,
    pub severity: EventSeverity,
    pub request_id: Option<&'a str>,
    pub http_status: Option<i64>,
    pub details_json: &'a str,
    pub message: &'a str,
}

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<GenerationTaskEvent> {
    let phase_str: String = row.get(3)?;
    let severity_str: String = row.get(4)?;
    Ok(GenerationTaskEvent {
        id: row.get(0)?,
        task_id: row.get(1)?,
        occurred_at: row.get(2)?,
        phase: EventPhase::from_db_str(&phase_str).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                Box::new(InvalidEnumValue(phase_str.clone())),
            )
        })?,
        severity: EventSeverity::from_db_str(&severity_str).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(InvalidEnumValue(severity_str.clone())),
            )
        })?,
        request_id: row.get(5)?,
        http_status: row.get(6)?,
        details_json: row.get(7)?,
        message: row.get(8)?,
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

/// Insert one event row. Returns `Ok(None)` if the per-task cap is exceeded;
/// the row is silently dropped (with a `tracing::warn!`) so a runaway poll
/// loop cannot break the main flow or fill the DB.
pub fn insert(conn: &Connection, input: NewEvent<'_>) -> Result<Option<GenerationTaskEvent>> {
    let count = count_by_task(conn, input.task_id)?;
    if count >= MAX_EVENTS_PER_TASK {
        tracing::warn!(
            task_id = %input.task_id,
            cap = MAX_EVENTS_PER_TASK,
            phase = %input.phase.as_str(),
            "event cap reached; dropping event"
        );
        return Ok(None);
    }

    conn.execute(
        "INSERT INTO generation_task_event \
            (task_id, phase, severity, request_id, http_status, details_json, message) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            input.task_id,
            input.phase.as_str(),
            input.severity.as_str(),
            input.request_id,
            input.http_status,
            input.details_json,
            input.message,
        ],
    )?;
    let id = conn.last_insert_rowid();
    let row = conn
        .query_row(
            &format!("SELECT {SELECT_COLUMNS} FROM generation_task_event WHERE id = ?1"),
            params![id],
            map_row,
        )
        .map_err(CoreError::from)?;
    Ok(Some(row))
}

pub fn list_by_task(conn: &Connection, task_id: &str) -> Result<Vec<GenerationTaskEvent>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM generation_task_event \
         WHERE task_id = ?1 ORDER BY occurred_at ASC, id ASC"
    ))?;
    let rows = stmt.query_map(params![task_id], map_row)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(CoreError::from)
}

pub fn count_by_task(conn: &Connection, task_id: &str) -> Result<i64> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM generation_task_event WHERE task_id = ?1",
        params![task_id],
        |r| r.get(0),
    )?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use crate::db::queries::generation_task as task_q;
    use crate::models::generation_task::{CreateGenerationTaskInput, TaskKind};
    use std::path::Path;

    fn seed_task(conn: &Connection) -> String {
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
        let task = task_q::create(
            conn,
            CreateGenerationTaskInput {
                project_id: None,
                shot_id: None,
                provider_id: "prov1".into(),
                model_id: "mod1".into(),
                account_id: "acc1".into(),
                task_type: TaskKind::Image,
                params_json: None,
            },
        )
        .unwrap();
        task.id
    }

    fn make_event<'a>(task_id: &'a str, message: &'a str) -> NewEvent<'a> {
        NewEvent {
            task_id,
            phase: EventPhase::SubmitCall,
            severity: EventSeverity::Info,
            request_id: Some("req-1"),
            http_status: Some(200),
            details_json: "{}",
            message,
        }
    }

    #[test]
    fn insert_then_list_roundtrip() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let task_id = seed_task(&conn);
        let inserted = insert(&conn, make_event(&task_id, "hello")).unwrap().unwrap();
        assert_eq!(inserted.task_id, task_id);
        assert_eq!(inserted.phase, EventPhase::SubmitCall);
        assert_eq!(inserted.message, "hello");
        assert_eq!(inserted.request_id.as_deref(), Some("req-1"));
        assert_eq!(inserted.http_status, Some(200));

        let listed = list_by_task(&conn, &task_id).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, inserted.id);
    }

    #[test]
    fn count_by_task_isolates_per_task() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let task_id = seed_task(&conn);
        for i in 0..5 {
            let msg = format!("m{i}");
            insert(&conn, make_event(&task_id, &msg)).unwrap();
        }
        assert_eq!(count_by_task(&conn, &task_id).unwrap(), 5);
        assert_eq!(count_by_task(&conn, "other").unwrap(), 0);
    }

    #[test]
    fn insert_above_cap_returns_none() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let task_id = seed_task(&conn);
        for _ in 0..MAX_EVENTS_PER_TASK {
            assert!(insert(&conn, make_event(&task_id, "x")).unwrap().is_some());
        }
        let dropped = insert(&conn, make_event(&task_id, "drop")).unwrap();
        assert!(dropped.is_none());
        assert_eq!(count_by_task(&conn, &task_id).unwrap(), MAX_EVENTS_PER_TASK);
    }

    #[test]
    fn cascade_delete_when_task_removed() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        // Foreign keys must be enabled for CASCADE to fire; open_sync sets PRAGMA.
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        let task_id = seed_task(&conn);
        insert(&conn, make_event(&task_id, "e1")).unwrap();
        insert(&conn, make_event(&task_id, "e2")).unwrap();
        assert_eq!(count_by_task(&conn, &task_id).unwrap(), 2);
        conn.execute(
            "DELETE FROM generation_task WHERE id = ?1",
            params![task_id],
        )
        .unwrap();
        assert_eq!(count_by_task(&conn, &task_id).unwrap(), 0);
    }
}
