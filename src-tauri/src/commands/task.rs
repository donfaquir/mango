use crate::error::IpcError;
use crate::state::AppState;
use mango_core::models::generation_task::{
    CreateGenerationTaskInput, GenerationTask, GenerationTaskStatus,
};
use mango_core::task_engine::ListFilter;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn submit_task(
    state: State<'_, AppState>,
    input: CreateGenerationTaskInput,
) -> Result<String, IpcError> {
    state
        .task_engine
        .submit(input)
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_task(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<(), IpcError> {
    state
        .task_engine
        .cancel(&task_id)
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn get_task(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<GenerationTask, IpcError> {
    state
        .task_engine
        .get(&task_id)
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn list_tasks(
    state: State<'_, AppState>,
    project_id: Option<String>,
    status: Option<GenerationTaskStatus>,
    limit: Option<u32>,
) -> Result<Vec<GenerationTask>, IpcError> {
    state
        .task_engine
        .list(ListFilter {
            project_id,
            status,
            limit,
        })
        .await
        .map_err(IpcError::from)
}
