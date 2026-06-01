use std::path::Path;

use super::with_db;
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::asset::import as import_pipeline;
use mango_core::db::queries::asset as asset_queries;
use mango_core::models::asset::{Asset, ImportAssetInput, ListAssetsOptions};
use tauri::{AppHandle, Manager, State};

/// Import an external image into the project.
///
/// Runs in three stages so the file-IO stage (potentially tens of MiB of
/// copy + image decode) does NOT hold the tokio-rusqlite worker. Stages 1
/// and 3 are short DB-bound calls; stage 2 is `spawn_blocking` filesystem
/// work. See spec-12 §"导入流水线（三段式）" for the design rationale.
#[tauri::command]
#[specta::specta]
pub async fn import_asset(
    state: State<'_, AppState>,
    app: AppHandle,
    input: ImportAssetInput,
) -> Result<Asset, IpcError> {
    // Stage 1 — resolve project root from DB (very short lock hold).
    let project_id_stage1 = input.project_id.clone();
    let project_root = with_db(&state, move |conn| {
        import_pipeline::resolve_project_root(conn, &project_id_stage1)
    })
    .await?;

    // 1.5 — register the project root with the asset-protocol scope so
    // `convertFileSrc` URLs from the webview actually resolve. Idempotent.
    register_project_scope(&app, &project_root)?;

    // Stage 2 — copy + hash + thumbnail, off the DB worker.
    let source_path = input.source_path.clone();
    let project_root_io = project_root.clone();
    let artifacts = tokio::task::spawn_blocking(move || {
        import_pipeline::prepare_artifacts(&project_root_io, &source_path)
    })
    .await
    .map_err(|e| IpcError::internal(e.to_string()))?
    .map_err(IpcError::from)?;

    // Stage 3 — dedupe check + INSERT (very short lock hold).
    let project_id_stage3 = input.project_id.clone();
    let shot_id_raw = input.shot_id.clone();
    let source = input.source;
    let project_root_persist = project_root;
    with_db(&state, move |conn| {
        let shot = import_pipeline::trim_shot_id(shot_id_raw.as_deref());
        import_pipeline::persist_artifacts(
            conn,
            &project_id_stage3,
            shot,
            &project_root_persist,
            artifacts,
            source,
        )
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn list_assets(
    state: State<'_, AppState>,
    opts: ListAssetsOptions,
) -> Result<Vec<Asset>, IpcError> {
    with_db(&state, move |conn| asset_queries::list(conn, opts)).await
}

/// Return the distinct non-empty `label` values currently used across the
/// project's assets. The frontend asset library uses this to populate the
/// label filter dropdown; "all" and "unlabeled" options are added by the UI.
#[tauri::command]
#[specta::specta]
pub async fn list_asset_labels(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<String>, IpcError> {
    with_db(&state, move |conn| asset_queries::list_labels(conn, &project_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_asset(state: State<'_, AppState>, id: String) -> Result<Asset, IpcError> {
    with_db(&state, move |conn| asset_queries::get_by_id(conn, &id)).await
}

/// Translate a project-root-relative `file_path` (as stored on rows like
/// `character.reference_image_path`) back to the canonical asset row. Returns
/// `Ok(None)` when no asset matches — callers decide how to surface that to
/// the user (e.g. "selected reference image is not in the asset library").
#[tauri::command]
#[specta::specta]
pub async fn find_asset_by_path(
    state: State<'_, AppState>,
    project_id: String,
    file_path: String,
) -> Result<Option<Asset>, IpcError> {
    with_db(&state, move |conn| {
        asset_queries::find_by_file_path(conn, &project_id, &file_path)
    })
    .await
}

/// Delete an asset row. Files on disk are intentionally not removed; a future
/// GC sweep (V2) reconciles orphaned files. See spec-12 §"错误场景".
#[tauri::command]
#[specta::specta]
pub async fn delete_asset(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    with_db(&state, move |conn| asset_queries::delete(conn, &id)).await
}

/// Update the free-form `label` of an asset (e.g. user-applied tag in the
/// asset library). An empty string clears the tag. Returns the post-update
/// row so the caller's cache can refresh in a single roundtrip.
#[tauri::command]
#[specta::specta]
pub async fn update_asset_label(
    state: State<'_, AppState>,
    id: String,
    label: String,
) -> Result<Asset, IpcError> {
    with_db(&state, move |conn| asset_queries::update_label(conn, &id, &label)).await
}

/// Bind (or unbind) an asset to a shot. `Some(shot_id)` overwrites the prior
/// binding silently — UI surfaces (e.g. canvas drag-to-shot) are expected to
/// confirm overwrites themselves before calling. `None` clears the binding.
/// Returns the post-update row so the caller's cache can refresh in a single
/// roundtrip.
#[tauri::command]
#[specta::specta]
pub async fn assign_asset_to_shot(
    state: State<'_, AppState>,
    id: String,
    shot_id: Option<String>,
) -> Result<Asset, IpcError> {
    with_db(&state, move |conn| {
        asset_queries::assign_to_shot(conn, &id, shot_id.as_deref())
    })
    .await
}

/// Allow the asset protocol to read files under `project_root`. The webview
/// needs this before `convertFileSrc(<absolute path>)` URLs can resolve.
/// Safe to call repeatedly; `allow_directory` is idempotent.
#[tauri::command]
#[specta::specta]
pub async fn register_project_asset_scope(
    state: State<'_, AppState>,
    app: AppHandle,
    project_id: String,
) -> Result<(), IpcError> {
    let project_root = with_db(&state, move |conn| {
        import_pipeline::resolve_project_root(conn, &project_id)
    })
    .await?;
    register_project_scope(&app, &project_root)
}

fn register_project_scope(app: &AppHandle, project_root: &Path) -> Result<(), IpcError> {
    app.asset_protocol_scope()
        .allow_directory(project_root, true)
        .map_err(|e| IpcError::internal(format!("failed to register asset scope: {e}")))
}
