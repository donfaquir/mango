use super::with_db;
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::db::queries::shot_audio as shot_audio_q;
use mango_core::models::shot_audio::{CreateShotAudioInput, ShotAudio, UpdateShotAudioInput};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn list_shot_audio(
    state: State<'_, AppState>,
    shot_id: String,
) -> Result<Vec<ShotAudio>, IpcError> {
    with_db(&state, move |conn| shot_audio_q::list_by_shot(conn, &shot_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_shot_audio(
    state: State<'_, AppState>,
    input: CreateShotAudioInput,
) -> Result<ShotAudio, IpcError> {
    with_db(&state, move |conn| shot_audio_q::create(conn, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_shot_audio(
    state: State<'_, AppState>,
    id: String,
    input: UpdateShotAudioInput,
) -> Result<ShotAudio, IpcError> {
    with_db(&state, move |conn| shot_audio_q::update(conn, &id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_shot_audio(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| shot_audio_q::delete(conn, &id)).await
}
