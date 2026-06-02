use super::{require_mount, with_db};
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::db::queries::app_preference as preference_q;
use mango_core::error::CoreError;
use mango_core::models::generation_task::{
    CreateGenerationTaskInput, GenerationTask, GenerationTaskStatus,
};
use mango_core::models::generation_task_event::GenerationTaskEvent;
use mango_core::task_engine::{ListFilter, SubmitBatchOutcome};
use tauri::State;

const MAX_CONCURRENCY_KEY: &str = "task.max_concurrency";
const MAX_CONCURRENCY_MIN: u32 = 1;
const MAX_CONCURRENCY_MAX: u32 = 8;
const MAX_CONCURRENCY_FALLBACK: u32 = 3;

#[tauri::command]
#[specta::specta]
pub async fn submit_task(
    state: State<'_, AppState>,
    input: CreateGenerationTaskInput,
) -> Result<String, IpcError> {
    let engine = require_mount(&state)?.task_engine.clone();
    engine.submit(input).await.map_err(IpcError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn submit_tasks_batch(
    state: State<'_, AppState>,
    inputs: Vec<CreateGenerationTaskInput>,
) -> Result<SubmitBatchOutcome, IpcError> {
    let engine = require_mount(&state)?.task_engine.clone();
    engine.submit_batch(inputs).await.map_err(IpcError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_task(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<(), IpcError> {
    let engine = require_mount(&state)?.task_engine.clone();
    engine.cancel(&task_id).await.map_err(IpcError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn get_task(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<GenerationTask, IpcError> {
    let engine = require_mount(&state)?.task_engine.clone();
    engine.get(&task_id).await.map_err(IpcError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn list_tasks(
    state: State<'_, AppState>,
    project_id: Option<String>,
    status: Option<GenerationTaskStatus>,
    limit: Option<u32>,
) -> Result<Vec<GenerationTask>, IpcError> {
    let engine = require_mount(&state)?.task_engine.clone();
    engine
        .list(ListFilter {
            project_id,
            status,
            limit,
        })
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn list_task_events(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<Vec<GenerationTaskEvent>, IpcError> {
    let engine = require_mount(&state)?.task_engine.clone();
    engine.list_events(&task_id).await.map_err(IpcError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn get_task_max_concurrency(state: State<'_, AppState>) -> Result<u32, IpcError> {
    with_db(&state, move |conn| {
        let raw = preference_q::get_i64(conn, MAX_CONCURRENCY_KEY)?
            .map(|n| n.clamp(MAX_CONCURRENCY_MIN as i64, MAX_CONCURRENCY_MAX as i64) as u32)
            .unwrap_or(MAX_CONCURRENCY_FALLBACK);
        Ok(raw)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn set_task_max_concurrency(
    state: State<'_, AppState>,
    value: u32,
) -> Result<(), IpcError> {
    if !(MAX_CONCURRENCY_MIN..=MAX_CONCURRENCY_MAX).contains(&value) {
        return Err(IpcError::from(CoreError::Validation(format!(
            "并发数需在 {MAX_CONCURRENCY_MIN}~{MAX_CONCURRENCY_MAX} 之间"
        ))));
    }
    // Persist first so the new value survives crashes; then push the live
    // semaphore swap so subsequent acquires honor the new cap immediately.
    let v_i64 = value as i64;
    with_db(&state, move |conn| {
        preference_q::set_i64(conn, MAX_CONCURRENCY_KEY, v_i64)
    })
    .await?;
    let engine = require_mount(&state)?.task_engine.clone();
    engine.set_max_concurrency(value as usize);
    Ok(())
}
