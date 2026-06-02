//! Workspace lifecycle commands.
//!
//! Two paths to "have a mounted workspace":
//! - **First-time onboarding** → `mount_workspace(path)` does a hot mount
//!   in-process. No restart needed; the OnceLock fills, asset scope is
//!   granted, task engine spawns, and the frontend just invalidates the
//!   workspace-status query to swap into the main UI.
//! - **Switching to a different workspace** → `set_workspace_and_relaunch`
//!   writes the pointer config and schedules a clean exit. The user
//!   relaunches; the next boot mounts the new workspace via `setup()`.
//!   We don't hot-swap because the frontend's React Query cache + router
//!   state would be referencing the old workspace's projects.
//!
//! Plus `probe_workspace` (pure inspection, no state mutation) and
//! `get_workspace_status` (drives the boot-time routing decision).

use std::path::PathBuf;
use std::time::Duration;

use mango_core::app_config::{self, AppConfig, WorkspaceProbe};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, State};

use crate::error::IpcError;
use crate::init;
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

/// Current workspace mount state. Drives the frontend's onboarding vs
/// main-app routing decision.
#[tauri::command]
#[specta::specta]
pub async fn get_workspace_status(
    state: State<'_, AppState>,
) -> Result<WorkspaceStatus, IpcError> {
    Ok(WorkspaceStatus {
        workspace_root: state.mounted.get().map(|m| m.workspace_root.clone()),
    })
}

/// Hot-mount a workspace at `path`: write the pointer config, open the DB,
/// register the asset-protocol scope, spawn the task engine, and install
/// `MountedState` into the OnceLock. After this returns the frontend can
/// `invalidate(['workspace', 'status'])` and the app slides into the main
/// UI — no process restart involved.
///
/// Refuses if a workspace is already mounted (the OnceLock would reject
/// the `.set` anyway, but we want a clean error code for the frontend).
#[tauri::command]
#[specta::specta]
pub async fn mount_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<WorkspaceStatus, IpcError> {
    if state.mounted.get().is_some() {
        return Err(IpcError {
            message: "workspace already mounted; use switch flow to change it".into(),
            code: "WORKSPACE_ALREADY_MOUNTED".into(),
            request_id: None,
            http_status: None,
            kind: None,
        });
    }

    // Defensive re-probe in case the directory state changed between the
    // frontend's probe call and this commit. We don't try to coerce an
    // `Invalid` probe into success — the frontend should have already
    // surfaced the reason before getting here.
    let probe_path = path.clone();
    let probe = tokio::task::spawn_blocking(move || app_config::probe_workspace(&probe_path))
        .await
        .map_err(|e| IpcError::internal(format!("probe join error: {e}")))?;
    if let WorkspaceProbe::Invalid { reason } = probe {
        return Err(IpcError {
            message: format!("workspace path rejected: {reason}"),
            code: "WORKSPACE_INVALID".into(),
            request_id: None,
            http_status: None,
            kind: None,
        });
    }

    // Persist the pointer first so a crash between here and the OnceLock
    // set still leaves the user with a recoverable state on next boot.
    // (`init_workspace` mkdirs the target via `prepare_workspace`, so we
    // don't have to do it ourselves before opening the DB.)
    let app_data_dir = state.app_data_dir.clone();
    let cfg = AppConfig {
        workspace_path: path.clone(),
    };
    tokio::task::spawn_blocking(move || app_config::write(&app_data_dir, &cfg))
        .await
        .map_err(|e| IpcError::internal(format!("config write join error: {e}")))?
        .map_err(IpcError::from)?;

    // Open DB, run migrations + seed, spawn engine, hook event forwarder.
    let mounted = init::init_workspace(&app, state.keyring.clone(), &path)
        .await
        .map_err(IpcError::internal)?;
    let workspace_root = mounted.workspace_root.clone();

    state.mounted.set(mounted).map_err(|_| IpcError {
        message: "workspace was concurrently mounted".into(),
        code: "WORKSPACE_ALREADY_MOUNTED".into(),
        request_id: None,
        http_status: None,
        kind: None,
    })?;

    Ok(WorkspaceStatus {
        workspace_root: Some(workspace_root),
    })
}

/// Switch the workspace pointer and schedule a clean exit so the user
/// relaunches into the new workspace. Used when a workspace is already
/// mounted — hot-swap isn't feasible because the frontend's React Query
/// cache + router state would still reference projects under the old
/// workspace.
///
/// Steps:
/// 1. Probe + validate the target.
/// 2. Materialise the new workspace on disk (`prepare_workspace` mkdir +
///    open + migrations) so `setup()` finds a ready `mango.db` on the
///    next boot. **This is the bit that was missing in the first cut and
///    caused the relaunch to fall back to onboarding** when the user
///    picked an empty directory.
/// 3. Write the pointer config.
/// 4. Schedule `app.exit(0)` 200 ms out so the IPC response delivers and
///    the frontend can render a "please relaunch" toast.
///
/// We use `app.exit(0)` rather than `app.restart()` because under
/// `tauri dev` restart re-execs the cargo target binary and disconnects
/// from the cargo-tauri parent that owns the vite watcher (observed as
/// "click does nothing"). Exiting cleanly is reliable in both dev and
/// production.
#[tauri::command]
#[specta::specta]
pub async fn set_workspace_and_relaunch(
    app: AppHandle,
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<(), IpcError> {
    // Defensive re-probe: same reason as `mount_workspace`.
    let probe_path = path.clone();
    let probe = tokio::task::spawn_blocking(move || app_config::probe_workspace(&probe_path))
        .await
        .map_err(|e| IpcError::internal(format!("probe join error: {e}")))?;
    if let WorkspaceProbe::Invalid { reason } = probe {
        return Err(IpcError {
            message: format!("workspace path rejected: {reason}"),
            code: "WORKSPACE_INVALID".into(),
            request_id: None,
            http_status: None,
            kind: None,
        });
    }

    // Materialise the target so the next boot finds <ws>/mango.db ready.
    // Drop the connection immediately afterwards — the running process is
    // still bound to the old workspace's engine and we don't want two
    // open handles to the new DB (one here, one after relaunch).
    let prepared = init::prepare_workspace(&path).await.map_err(IpcError::internal)?;
    drop(prepared);

    let app_data_dir = state.app_data_dir.clone();
    let cfg = AppConfig {
        workspace_path: path,
    };
    tokio::task::spawn_blocking(move || app_config::write(&app_data_dir, &cfg))
        .await
        .map_err(|e| IpcError::internal(format!("config write join error: {e}")))?
        .map_err(IpcError::from)?;

    let app_for_exit = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        app_for_exit.exit(0);
    });
    Ok(())
}
