use super::with_db;
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::db::queries::episode as episode_queries;
use mango_core::models::episode::{
    CreateEpisodeInput, Episode, ListEpisodesOptions, UpdateEpisodeInput,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn create_episode(
    state: State<'_, AppState>,
    input: CreateEpisodeInput,
) -> Result<Episode, IpcError> {
    with_db(&state, move |conn| episode_queries::create(conn, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_episode(
    state: State<'_, AppState>,
    id: String,
) -> Result<Episode, IpcError> {
    with_db(&state, move |conn| episode_queries::get_by_id(conn, &id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_episodes(
    state: State<'_, AppState>,
    opts: ListEpisodesOptions,
) -> Result<Vec<Episode>, IpcError> {
    with_db(&state, move |conn| episode_queries::list(conn, opts)).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_episode(
    state: State<'_, AppState>,
    id: String,
    input: UpdateEpisodeInput,
) -> Result<Episode, IpcError> {
    with_db(&state, move |conn| episode_queries::update(conn, &id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_episode(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| episode_queries::delete(conn, &id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn reorder_episodes(
    state: State<'_, AppState>,
    project_id: String,
    ordered_ids: Vec<String>,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| {
        episode_queries::reorder_within_project(conn, &project_id, &ordered_ids)
    })
    .await
}
