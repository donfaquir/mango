use super::with_db;
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::db::queries::prop as prop_queries;
use mango_core::models::prop::{CreatePropInput, ListPropsOptions, Prop, UpdatePropInput};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn create_prop(
    state: State<'_, AppState>,
    input: CreatePropInput,
) -> Result<Prop, IpcError> {
    with_db(&state, move |conn| prop_queries::create(conn, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_prop(
    state: State<'_, AppState>,
    id: String,
) -> Result<Prop, IpcError> {
    with_db(&state, move |conn| prop_queries::get_by_id(conn, &id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_props(
    state: State<'_, AppState>,
    opts: ListPropsOptions,
) -> Result<Vec<Prop>, IpcError> {
    with_db(&state, move |conn| prop_queries::list(conn, opts)).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_prop(
    state: State<'_, AppState>,
    id: String,
    input: UpdatePropInput,
) -> Result<Prop, IpcError> {
    with_db(&state, move |conn| prop_queries::update(conn, &id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_prop(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| prop_queries::delete(conn, &id)).await
}
