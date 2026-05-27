use super::with_db;
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::db::queries::project as project_queries;
use mango_core::models::project::{
    CreateProjectInput, ListProjectsOptions, Project, UpdateProjectInput,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn create_project(
    state: State<'_, AppState>,
    input: CreateProjectInput,
) -> Result<Project, IpcError> {
    let app_data_dir = state.app_data_dir.clone();
    with_db(&state, move |conn| {
        project_queries::create(conn, &app_data_dir, input)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn get_project(
    state: State<'_, AppState>,
    id: String,
) -> Result<Project, IpcError> {
    with_db(&state, move |conn| project_queries::get_by_id(conn, &id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_projects(
    state: State<'_, AppState>,
    opts: Option<ListProjectsOptions>,
) -> Result<Vec<Project>, IpcError> {
    let opts = opts.unwrap_or_default();
    with_db(&state, move |conn| project_queries::list(conn, opts)).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_project(
    state: State<'_, AppState>,
    id: String,
    input: UpdateProjectInput,
) -> Result<Project, IpcError> {
    with_db(&state, move |conn| project_queries::update(conn, &id, input)).await
}

/// Delete project metadata only. The on-disk root_path directory and its
/// contents are intentionally preserved; V2 will add an explicit purge flag.
#[tauri::command]
#[specta::specta]
pub async fn delete_project(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| project_queries::delete(conn, &id)).await
}
