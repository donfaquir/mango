use super::with_db;
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::db::queries::shot as shot_queries;
use mango_core::models::shot::{
    CreateShotInput, ListShotsOptions, Shot, ShotLinks, SubjectKind, UpdateShotInput,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn create_shot(
    state: State<'_, AppState>,
    input: CreateShotInput,
) -> Result<Shot, IpcError> {
    with_db(&state, move |conn| shot_queries::create(conn, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_shot(state: State<'_, AppState>, id: String) -> Result<Shot, IpcError> {
    with_db(&state, move |conn| shot_queries::get_by_id(conn, &id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_shots(
    state: State<'_, AppState>,
    opts: ListShotsOptions,
) -> Result<Vec<Shot>, IpcError> {
    with_db(&state, move |conn| shot_queries::list(conn, opts)).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_shot(
    state: State<'_, AppState>,
    id: String,
    input: UpdateShotInput,
) -> Result<Shot, IpcError> {
    with_db(&state, move |conn| shot_queries::update(conn, &id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_shot(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| shot_queries::delete(conn, &id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn reorder_shots(
    state: State<'_, AppState>,
    episode_id: String,
    ordered_ids: Vec<String>,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| {
        shot_queries::reorder_within_episode(conn, &episode_id, &ordered_ids)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn list_shot_links(
    state: State<'_, AppState>,
    shot_id: String,
) -> Result<ShotLinks, IpcError> {
    with_db(&state, move |conn| shot_queries::list_links(conn, &shot_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn link_shot_subject(
    state: State<'_, AppState>,
    shot_id: String,
    subject_id: String,
    subject_kind: SubjectKind,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| {
        shot_queries::link_subject(conn, &shot_id, &subject_id, subject_kind)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn unlink_shot_subject(
    state: State<'_, AppState>,
    shot_id: String,
    subject_id: String,
    subject_kind: SubjectKind,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| {
        shot_queries::unlink_subject(conn, &shot_id, &subject_id, subject_kind)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn adopt_task_result(
    state: State<'_, AppState>,
    shot_id: String,
    task_id: String,
) -> Result<Shot, IpcError> {
    with_db(&state, move |conn| {
        shot_queries::adopt_task_result(conn, &shot_id, &task_id)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn unadopt_shot(
    state: State<'_, AppState>,
    shot_id: String,
) -> Result<Shot, IpcError> {
    with_db(&state, move |conn| shot_queries::unadopt(conn, &shot_id)).await
}
