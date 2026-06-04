pub mod account;
pub mod asset;
pub mod canvas_layout;
pub mod cli;
pub mod character;
pub mod costume;
pub mod dialog;
pub mod episode;
pub mod episode_checkpoint;
pub mod project;
pub mod provider;
pub mod prop;
pub mod scene;
pub mod shot;
pub mod task;
pub mod workspace;

use crate::error::IpcError;
use crate::state::{AppState, MountedState};
use tauri::State;

/// Return the mounted workspace state, or a stable error so the frontend
/// can route the user to onboarding. All business commands flow through
/// `with_db` (which calls this) or call this directly when they need
/// `workspace_root` / `task_engine` without touching the DB.
pub fn require_mount<'a>(
    state: &'a State<'_, AppState>,
) -> Result<&'a MountedState, IpcError> {
    state.mounted.get().ok_or_else(|| IpcError {
        message: "no workspace mounted".into(),
        code: "WORKSPACE_NOT_MOUNTED".into(),
        request_id: None,
        http_status: None,
        kind: None,
    })
}

/// Execute a synchronous DB operation on the tokio-rusqlite DB thread.
///
/// The closure receives a `&mut rusqlite::Connection` and returns a domain
/// `Result<T, CoreError>`. We wrap that into `Ok(...)` so the outer `call`
/// future is `Result<mango_core::Result<T>, tokio_rusqlite::Error>` — there is
/// no `Other(Box<dyn Error>)` variant on tokio-rusqlite 0.7 to map CoreError
/// into directly, so this two-layer unwrap is the cleanest portable shape.
///
/// Implicitly requires a mounted workspace — callers reaching this helper
/// before onboarding completes get `WORKSPACE_NOT_MOUNTED` back, which the
/// frontend already routes around.
pub async fn with_db<T, F>(state: &State<'_, AppState>, f: F) -> Result<T, IpcError>
where
    T: Send + 'static,
    F: FnOnce(&mut rusqlite::Connection) -> mango_core::error::Result<T> + Send + 'static,
{
    let db = require_mount(state)?.db.clone();
    let domain_result: mango_core::error::Result<T> = db
        .call(move |conn| Ok(f(conn)))
        .await
        .map_err(IpcError::from)?;

    domain_result.map_err(IpcError::from)
}
