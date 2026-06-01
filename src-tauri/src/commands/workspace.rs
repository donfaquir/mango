//! Workspace lifecycle commands — thin IPC wrappers around `mango_core::app_config`.
//!
//! Three responsibilities:
//! 1. `probe_workspace(path)` — classify a user-chosen directory so the UI can
//!    show the right confirmation (empty / has mango data / non-empty foreign).
//! 2. `get_workspace_status()` — tell the frontend whether a workspace is
//!    currently mounted (drives onboarding vs main-app routing).
//! 3. `set_workspace_and_relaunch(path)` — persist the pointer config and
//!    restart the app so the next boot mounts the chosen workspace.
//!
//! PR1 scope: ship the three commands. The conditional-mount setup that uses
//! them lands in PR2; until then `AppState.workspace_root` stays `None` and
//! `get_workspace_status` always reports `workspace_root: None`.

use std::path::PathBuf;

use mango_core::app_config::{self, AppConfig, WorkspaceProbe};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, State};

use crate::error::IpcError;
use crate::state::AppState;

/// Snapshot of workspace mount state for the frontend router.
#[derive(Debug, Serialize, Type)]
pub struct WorkspaceStatus {
    /// Absolute path of the mounted workspace, or `None` if onboarding is
    /// required.
    pub workspace_root: Option<PathBuf>,
}

/// Probe a candidate workspace directory. Pure inspection — no writes, no
/// state mutation. The frontend calls this before showing a confirmation
/// dialog so the user sees the right message for what they're about to do.
#[tauri::command]
#[specta::specta]
pub async fn probe_workspace(path: PathBuf) -> Result<WorkspaceProbe, IpcError> {
    // Spawn-blocking because filesystem inspection (and the best-effort
    // sqlite open in `count_projects`) shouldn't park the tokio worker on
    // a slow disk.
    tokio::task::spawn_blocking(move || app_config::probe_workspace(&path))
        .await
        .map_err(|e| IpcError::internal(format!("probe task join error: {e}")))
}

/// Current workspace mount state. PR1 always returns `None` because the
/// conditional-mount setup() lands in PR2. The frontend can already call
/// this — it will just always route to onboarding for now.
#[tauri::command]
#[specta::specta]
pub async fn get_workspace_status(
    state: State<'_, AppState>,
) -> Result<WorkspaceStatus, IpcError> {
    Ok(WorkspaceStatus {
        workspace_root: state.workspace_root.clone(),
    })
}

/// Persist `path` as the workspace pointer in `<app_data>/config.json`,
/// then restart the app so the next boot mounts the new workspace.
/// Caller is expected to have run `probe_workspace` and presented the
/// appropriate confirmation already — this command does NOT re-probe.
#[tauri::command]
#[specta::specta]
pub async fn set_workspace_and_relaunch(
    app: AppHandle,
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<(), IpcError> {
    let app_data_dir = state.app_data_dir.clone();
    let cfg = AppConfig {
        workspace_path: path,
    };
    tokio::task::spawn_blocking(move || app_config::write(&app_data_dir, &cfg))
        .await
        .map_err(|e| IpcError::internal(format!("config write join error: {e}")))?
        .map_err(IpcError::from)?;
    // `restart` returns `!` — process is replaced immediately.
    app.restart();
}
