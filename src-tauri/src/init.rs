//! Workspace initialisation helpers — shared by `lib.rs::setup` (boot-time
//! mount, used when the pointer config already points at a valid workspace)
//! and `commands::workspace::mount_workspace` (runtime mount, used during
//! onboarding when the user picks a directory).
//!
//! Splitting the two call paths' shared work into one place avoids drift:
//! the runtime mount uses the same DB open + provider seed + asset-scope
//! grant + task engine spawn sequence as the boot-time mount, so behaviour
//! after onboarding is indistinguishable from a "had workspace from the
//! start" launch.

use std::path::Path;
use std::sync::Arc;

use mango_core::account::keyring::KeyringStore;
use mango_core::provider::bailian::materializer::BailianResultMaterializer;
use mango_core::provider::bailian::BailianProvider;
use mango_core::provider::ProviderRegistry;
use mango_core::task_engine::{TaskEngineHandle, TaskEvent};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_rusqlite::Connection as AsyncConnection;

use crate::events;
use crate::state::MountedState;

/// Filename of the per-workspace metadata DB. Mirrors the constant in
/// `lib.rs` — duplicated rather than re-exported to keep the init module
/// independent of shell wiring.
pub const METADATA_DB_FILENAME: &str = "mango.db";

/// Materialise the workspace on disk so the next boot can mount it
/// cleanly: create the directory, open `mango.db`, and run migrations +
/// provider seed. Returns the open connection so the caller can either
/// hand it to `init_workspace` (in-process mount) or drop it (switch
/// flow — the user relaunches and `setup()` re-opens).
///
/// Split from `init_workspace` so the switch flow can prepare a target
/// workspace without spawning a task engine against it; the running
/// process is still tied to the *old* workspace.
pub async fn prepare_workspace(workspace_root: &Path) -> Result<AsyncConnection, String> {
    tokio::task::spawn_blocking({
        let p = workspace_root.to_path_buf();
        move || std::fs::create_dir_all(&p)
    })
    .await
    .map_err(|e| format!("mkdir join error: {e}"))?
    .map_err(|e| format!("failed to create workspace dir: {e}"))?;

    let db_path = workspace_root.join(METADATA_DB_FILENAME);

    let db = mango_core::db::open_async(&db_path)
        .await
        .map_err(|e| format!("failed to open workspace DB at {}: {e}", db_path.display()))?;

    // Provider seed + orphan-running reset. Fatal here would mean a corrupt
    // DB — surface the message so the caller can show it.
    db.call(
        |conn| -> std::result::Result<mango_core::error::Result<()>, rusqlite::Error> {
            Ok(mango_core::startup::initialize(conn))
        },
    )
    .await
    .map_err(|e| format!("startup initialize call failed on DB thread: {e}"))?
    .map_err(|e| format!("startup initialize failed: {e}"))?;

    Ok(db)
}

/// Hot-mount path: prepare the workspace, register the asset protocol
/// scope, spawn the task engine, and return `MountedState` for the caller
/// to install into `AppState.mounted`.
///
/// Failure is bubbled up so the runtime mount path (onboarding) can
/// surface a user-facing error instead of aborting the process. The boot
/// path in `setup()` still treats failure as fatal.
pub async fn init_workspace(
    app: &AppHandle,
    keyring: Arc<dyn KeyringStore>,
    workspace_root: &Path,
) -> Result<MountedState, String> {
    let db = prepare_workspace(workspace_root).await?;

    // Grant the asset protocol read access to the entire workspace once.
    // Every project root sits under `<workspace>/projects/`, so a single
    // recursive grant covers all current and future projects.
    app.asset_protocol_scope()
        .allow_directory(workspace_root, true)
        .map_err(|e| format!("failed to register workspace asset scope: {e}"))?;

    // Build the per-workspace task engine. Materializer holds the
    // workspace root so downloaded results land under the right project.
    let providers = ProviderRegistry::builder()
        .register("bailian", Arc::new(BailianProvider::new()))
        .build();
    let materializer: Arc<dyn mango_core::task_engine::ResultMaterializer> = Arc::new(
        BailianResultMaterializer::new(db.clone(), keyring.clone(), workspace_root.to_path_buf()),
    );
    let (engine, event_rx) = TaskEngineHandle::spawn(
        db.clone(),
        providers,
        keyring,
        materializer,
        workspace_root.to_path_buf(),
        4,
    );

    // Re-spawn runner coroutines for any pending tasks left from a previous
    // session. Runs in the background so the mount returns promptly even
    // for a DB with many pending rows.
    let engine_for_recovery = engine.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = engine_for_recovery.recover_pending().await {
            tracing::error!("failed to recover pending tasks: {e}");
        }
    });

    spawn_event_forwarder(app.clone(), event_rx);

    Ok(MountedState {
        db,
        workspace_root: workspace_root.to_path_buf(),
        task_engine: Arc::new(engine),
    })
}

/// Forward TaskEngine internal events into tauri-specta events for the
/// frontend. Lives here so both mount paths share the same wiring.
fn spawn_event_forwarder(app: AppHandle, mut rx: UnboundedReceiver<TaskEvent>) {
    tauri::async_runtime::spawn(async move {
        while let Some(ev) = rx.recv().await {
            match ev {
                TaskEvent::StatusChanged {
                    task_id,
                    status,
                    progress,
                    error_message,
                } => {
                    if let Err(e) = (events::TaskStatusChanged {
                        task_id,
                        status,
                        progress,
                        error_message,
                    })
                    .emit(&app)
                    {
                        tracing::warn!(error = %e, "failed to emit TaskStatusChanged");
                    }
                }
                TaskEvent::EventLogged { task_id, event } => {
                    if let Err(e) =
                        (events::TaskEventLogged { task_id, event }).emit(&app)
                    {
                        tracing::warn!(error = %e, "failed to emit TaskEventLogged");
                    }
                }
                TaskEvent::ProgressTick { task_id, progress } => {
                    if let Err(e) =
                        (events::TaskProgressTick { task_id, progress }).emit(&app)
                    {
                        tracing::warn!(error = %e, "failed to emit TaskProgressTick");
                    }
                }
            }
        }
    });
}
