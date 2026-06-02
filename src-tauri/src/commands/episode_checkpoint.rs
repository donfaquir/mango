use super::with_db;
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::db::queries::episode_checkpoint as checkpoint_queries;
use mango_core::models::episode_checkpoint::{CreateCheckpointInput, EpisodeCheckpoint};
use tauri::State;

/// Persist a placeholder checkpoint for the given episode. Reads the current
/// `canvas_layout` row on the DB worker and snapshots its three JSON columns
/// into a new `episode_checkpoint` row. Full version-management (list,
/// restore, retention) lands in MS4.
#[tauri::command]
#[specta::specta]
pub async fn create_episode_checkpoint(
    state: State<'_, AppState>,
    input: CreateCheckpointInput,
) -> Result<EpisodeCheckpoint, IpcError> {
    with_db(&state, move |conn| {
        checkpoint_queries::insert_minimal(conn, &input)
    })
    .await
}
