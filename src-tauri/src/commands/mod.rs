pub mod account;
pub mod asset;
pub mod canvas_layout;
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
use crate::state::AppState;
use tauri::State;

/// Execute a synchronous DB operation on the tokio-rusqlite DB thread.
///
/// The closure receives a `&mut rusqlite::Connection` and returns a domain
/// `Result<T, CoreError>`. We wrap that into `Ok(...)` so the outer `call`
/// future is `Result<mango_core::Result<T>, tokio_rusqlite::Error>` — there is
/// no `Other(Box<dyn Error>)` variant on tokio-rusqlite 0.7 to map CoreError
/// into directly, so this two-layer unwrap is the cleanest portable shape.
pub async fn with_db<T, F>(state: &State<'_, AppState>, f: F) -> Result<T, IpcError>
where
    T: Send + 'static,
    F: FnOnce(&mut rusqlite::Connection) -> mango_core::error::Result<T> + Send + 'static,
{
    let domain_result: mango_core::error::Result<T> = state
        .db
        .call(move |conn| Ok(f(conn)))
        .await
        .map_err(IpcError::from)?;

    domain_result.map_err(IpcError::from)
}
