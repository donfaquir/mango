use super::with_db;
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::db::queries::character as character_queries;
use mango_core::models::character::{
    Character, CreateCharacterInput, ListCharactersOptions, UpdateCharacterInput,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn create_character(
    state: State<'_, AppState>,
    input: CreateCharacterInput,
) -> Result<Character, IpcError> {
    with_db(&state, move |conn| character_queries::create(conn, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_character(
    state: State<'_, AppState>,
    id: String,
) -> Result<Character, IpcError> {
    with_db(&state, move |conn| character_queries::get_by_id(conn, &id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_characters(
    state: State<'_, AppState>,
    opts: ListCharactersOptions,
) -> Result<Vec<Character>, IpcError> {
    with_db(&state, move |conn| character_queries::list(conn, opts)).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_character(
    state: State<'_, AppState>,
    id: String,
    input: UpdateCharacterInput,
) -> Result<Character, IpcError> {
    with_db(&state, move |conn| character_queries::update(conn, &id, input)).await
}

/// Delete a character. Schema `ON DELETE CASCADE` removes its costumes;
/// callers display the affected count by querying costumes first.
#[tauri::command]
#[specta::specta]
pub async fn delete_character(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| character_queries::delete(conn, &id)).await
}
