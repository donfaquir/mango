//! Thin async wrapper layer between the runner and the DB. Each helper:
//!   1. Schedules a synchronous DB closure on the tokio-rusqlite worker thread.
//!   2. On success, emits a [`TaskEvent`] when the change is observable to the UI.
//!
//! Keeping the SQL inside `db::queries::generation_task` (single source of truth)
//! and putting only the cross-cutting concerns (events) here.

use tokio::sync::mpsc::UnboundedSender;
use tokio_rusqlite::Connection as AsyncConnection;

use crate::db::queries::generation_task as q;
use crate::error::{CoreError, Result};
use crate::models::generation_task::{GenerationTask, GenerationTaskStatus};

use super::events::TaskEvent;

pub async fn load(db: &AsyncConnection, task_id: &str) -> Result<GenerationTask> {
    let id = task_id.to_string();
    db.call(move |conn| Ok(q::get_by_id(conn, &id)))
        .await
        .map_err(map_async_err)?
}

pub async fn is_cancelled(db: &AsyncConnection, task_id: &str) -> Result<bool> {
    let task = load(db, task_id).await?;
    Ok(matches!(task.status, GenerationTaskStatus::Cancelled))
}

pub async fn set_external_id(db: &AsyncConnection, task_id: &str, external_id: &str) -> Result<()> {
    let id = task_id.to_string();
    let ext = external_id.to_string();
    db.call(move |conn| Ok(q::set_external_id(conn, &id, &ext)))
        .await
        .map_err(map_async_err)?
}

pub async fn set_result_asset_id(db: &AsyncConnection, task_id: &str, asset_id: &str) -> Result<()> {
    let id = task_id.to_string();
    let aid = asset_id.to_string();
    db.call(move |conn| Ok(q::set_result_asset_id(conn, &id, &aid)))
        .await
        .map_err(map_async_err)?
}

/// Apply a state-machine transition and emit a [`TaskEvent::StatusChanged`].
/// `progress` is informational only — the DB does not store it; it rides along
/// in the event so the UI can render a percentage during `Running`.
pub async fn transition(
    db: &AsyncConnection,
    event_tx: &UnboundedSender<TaskEvent>,
    task_id: &str,
    new_status: GenerationTaskStatus,
    progress: Option<u8>,
    error_message: Option<String>,
) -> Result<()> {
    let id = task_id.to_string();
    let err_clone = error_message.clone();
    db.call(move |conn| Ok(q::transition_status(conn, &id, new_status, err_clone.as_deref())))
        .await
        .map_err(map_async_err)??;

    // Best-effort send. A dropped receiver means the forwarder coroutine
    // exited; the engine still functions, the UI just stops getting events.
    let _ = event_tx.send(TaskEvent::status_changed(
        task_id,
        new_status,
        progress,
        error_message,
    ));
    Ok(())
}

fn map_async_err(e: tokio_rusqlite::Error) -> CoreError {
    match e {
        tokio_rusqlite::Error::Error(inner) => CoreError::Sqlite(inner),
        other => CoreError::TaskEngine(format!("db worker error: {other}")),
    }
}
