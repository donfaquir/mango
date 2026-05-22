use super::with_db;
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::db::queries::costume as costume_queries;
use mango_core::models::costume::{
    Costume, CreateCostumeInput, ListCostumesOptions, UpdateCostumeInput,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn create_costume(
    state: State<'_, AppState>,
    input: CreateCostumeInput,
) -> Result<Costume, IpcError> {
    with_db(&state, move |conn| costume_queries::create(conn, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_costume(
    state: State<'_, AppState>,
    id: String,
) -> Result<Costume, IpcError> {
    with_db(&state, move |conn| costume_queries::get_by_id(conn, &id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_costumes(
    state: State<'_, AppState>,
    opts: ListCostumesOptions,
) -> Result<Vec<Costume>, IpcError> {
    with_db(&state, move |conn| costume_queries::list(conn, opts)).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_costume(
    state: State<'_, AppState>,
    id: String,
    input: UpdateCostumeInput,
) -> Result<Costume, IpcError> {
    with_db(&state, move |conn| costume_queries::update(conn, &id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_costume(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| costume_queries::delete(conn, &id)).await
}
