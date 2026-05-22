use super::with_db;
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::db::queries::provider as provider_queries;
use mango_core::models::provider::{Model, Provider};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn list_providers(
    state: State<'_, AppState>,
) -> Result<Vec<Provider>, IpcError> {
    with_db(&state, |conn| provider_queries::list_providers(conn)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_models(
    state: State<'_, AppState>,
    provider_id: Option<String>,
) -> Result<Vec<Model>, IpcError> {
    with_db(&state, move |conn| {
        provider_queries::list_models(conn, provider_id)
    })
    .await
}
