use super::{require_mount, with_db};
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::db::queries::project as project_queries;
use mango_core::models::project::{
    CreateProjectInput, ListProjectsOptions, Project, UpdateProjectInput,
};
use mango_core::paths;
use std::path::{Path, PathBuf};
use tauri::State;

/// Pull the mounted workspace root out of state, or return a stable error so
/// the frontend can route the user back to onboarding.
fn require_workspace(state: &State<'_, AppState>) -> Result<PathBuf, IpcError> {
    Ok(require_mount(state)?.workspace_root.clone())
}

/// Replace the project's workspace-relative `root_path` with the absolute
/// resolved form before crossing the IPC boundary. Frontend consumers treat
/// `Project.root_path` as an absolute path (it gets fed straight into
/// `convertFileSrc` via the resolveAssetUrl helper), so the relative form
/// must not leak past this layer.
fn absolutize_root(workspace: &Path, mut project: Project) -> Result<Project, IpcError> {
    let abs = paths::resolve_project_root(workspace, &project.root_path)
        .map_err(IpcError::from)?;
    project.root_path = abs.to_string_lossy().to_string();
    Ok(project)
}

#[tauri::command]
#[specta::specta]
pub async fn create_project(
    state: State<'_, AppState>,
    input: CreateProjectInput,
) -> Result<Project, IpcError> {
    let workspace = require_workspace(&state)?;
    let workspace_for_db = workspace.clone();
    let project = with_db(&state, move |conn| {
        project_queries::create(conn, &workspace_for_db, input)
    })
    .await?;
    absolutize_root(&workspace, project)
}

#[tauri::command]
#[specta::specta]
pub async fn get_project(
    state: State<'_, AppState>,
    id: String,
) -> Result<Project, IpcError> {
    let workspace = require_workspace(&state)?;
    let project = with_db(&state, move |conn| project_queries::get_by_id(conn, &id)).await?;
    absolutize_root(&workspace, project)
}

#[tauri::command]
#[specta::specta]
pub async fn list_projects(
    state: State<'_, AppState>,
    opts: Option<ListProjectsOptions>,
) -> Result<Vec<Project>, IpcError> {
    let workspace = require_workspace(&state)?;
    let opts = opts.unwrap_or_default();
    let projects = with_db(&state, move |conn| project_queries::list(conn, opts)).await?;
    projects
        .into_iter()
        .map(|p| absolutize_root(&workspace, p))
        .collect()
}

#[tauri::command]
#[specta::specta]
pub async fn update_project(
    state: State<'_, AppState>,
    id: String,
    input: UpdateProjectInput,
) -> Result<Project, IpcError> {
    let workspace = require_workspace(&state)?;
    let project =
        with_db(&state, move |conn| project_queries::update(conn, &id, input)).await?;
    absolutize_root(&workspace, project)
}

/// Delete project metadata only. The on-disk root_path directory and its
/// contents are intentionally preserved; V2 will add an explicit purge flag.
#[tauri::command]
#[specta::specta]
pub async fn delete_project(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    // Allowed even without a workspace mounted — callers shouldn't be able
    // to reach this command in that state via the UI, but if they do (e.g.
    // CLI), the operation is purely DB-bound so we don't need the path.
    with_db(&state, move |conn| project_queries::delete(conn, &id)).await
}
