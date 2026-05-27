use super::with_db;
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::account::service as account_service;
use mango_core::db::queries::api_account as account_queries;
use mango_core::models::api_account::{
    ApiAccount, CreateApiAccountInput, UpdateApiAccountInput,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn create_api_account(
    state: State<'_, AppState>,
    input: CreateApiAccountInput,
) -> Result<ApiAccount, IpcError> {
    let keyring = state.keyring.clone();
    with_db(&state, move |conn| {
        account_service::create(conn, keyring.as_ref(), input)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn list_api_accounts(
    state: State<'_, AppState>,
    provider_id: Option<String>,
) -> Result<Vec<ApiAccount>, IpcError> {
    with_db(&state, move |conn| account_queries::list(conn, provider_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_api_account(
    state: State<'_, AppState>,
    id: String,
) -> Result<ApiAccount, IpcError> {
    with_db(&state, move |conn| account_queries::get_by_id(conn, &id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_api_account(
    state: State<'_, AppState>,
    id: String,
    input: UpdateApiAccountInput,
) -> Result<ApiAccount, IpcError> {
    let keyring = state.keyring.clone();
    with_db(&state, move |conn| {
        account_service::update(conn, keyring.as_ref(), &id, input)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_api_account(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    let keyring = state.keyring.clone();
    with_db(&state, move |conn| {
        account_service::delete(conn, keyring.as_ref(), &id)
    })
    .await
}

/// Confirms that the keyring still holds a non-empty credential for this
/// account. NOT a network test — that arrives in MS2 under a different name
/// (`test_connection`) so the UI can offer both without collision.
#[tauri::command]
#[specta::specta]
pub async fn verify_api_account_storage(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    let keyring = state.keyring.clone();
    with_db(&state, move |conn| {
        account_service::verify_storage(conn, keyring.as_ref(), &id)
    })
    .await
}
