use super::with_db;
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::db::queries::scene as scene_queries;
use mango_core::models::scene::{
    CreateSceneInput, ListScenesOptions, Scene, UpdateSceneInput,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn create_scene(
    state: State<'_, AppState>,
    input: CreateSceneInput,
) -> Result<Scene, IpcError> {
    with_db(&state, move |conn| scene_queries::create(conn, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_scene(
    state: State<'_, AppState>,
    id: String,
) -> Result<Scene, IpcError> {
    with_db(&state, move |conn| scene_queries::get_by_id(conn, &id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_scenes(
    state: State<'_, AppState>,
    opts: ListScenesOptions,
) -> Result<Vec<Scene>, IpcError> {
    with_db(&state, move |conn| scene_queries::list(conn, opts)).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_scene(
    state: State<'_, AppState>,
    id: String,
    input: UpdateSceneInput,
) -> Result<Scene, IpcError> {
    with_db(&state, move |conn| scene_queries::update(conn, &id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_scene(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| scene_queries::delete(conn, &id)).await
}
