use super::with_db;
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::db::queries::episode_checkpoint as checkpoint_queries;
use mango_core::db::queries::episode_checkpoint_ops as checkpoint_ops;
use mango_core::models::episode::Episode;
use mango_core::models::episode_checkpoint::{
    CreateCheckpointInput, EpisodeCheckpoint, EpisodeCheckpointListItem,
};
use tauri::{AppHandle, State};
use tauri_specta::Event;

#[tauri::command]
#[specta::specta]
pub async fn create_episode_checkpoint(
    state: State<'_, AppState>,
    input: CreateCheckpointInput,
) -> Result<EpisodeCheckpoint, IpcError> {
    with_db(&state, move |conn| {
        checkpoint_queries::insert_full(conn, &input)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn list_episode_checkpoints(
    state: State<'_, AppState>,
    episode_id: String,
) -> Result<Vec<EpisodeCheckpointListItem>, IpcError> {
    with_db(&state, move |conn| {
        checkpoint_queries::list(conn, &episode_id)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn restore_episode_checkpoint(
    app: AppHandle,
    state: State<'_, AppState>,
    checkpoint_id: String,
) -> Result<Episode, IpcError> {
    let episode = with_db(&state, move |conn| {
        checkpoint_ops::restore(conn, &checkpoint_id)
    })
    .await?;

    let _ = crate::events::EpisodeDataRestored {
        episode_id: episode.id.clone(),
    }
    .emit(&app);

    Ok(episode)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_episode_checkpoint(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| checkpoint_queries::delete(conn, &id)).await
}
