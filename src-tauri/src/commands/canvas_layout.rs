use super::with_db;
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::db::queries::canvas_layout as canvas_queries;
use mango_core::models::canvas_layout::{CanvasLayout, UpsertCanvasLayoutInput};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn get_canvas_layout(
    state: State<'_, AppState>,
    episode_id: String,
) -> Result<Option<CanvasLayout>, IpcError> {
    with_db(&state, move |conn| {
        canvas_queries::get_by_episode(conn, &episode_id)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn upsert_canvas_layout(
    state: State<'_, AppState>,
    input: UpsertCanvasLayoutInput,
) -> Result<CanvasLayout, IpcError> {
    with_db(&state, move |conn| canvas_queries::upsert(conn, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_canvas_layout(
    state: State<'_, AppState>,
    episode_id: String,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| {
        canvas_queries::delete_by_episode(conn, &episode_id)
    })
    .await
}
