use std::path::PathBuf;

use tokio_rusqlite::Connection as AsyncConnection;

/// Tauri global managed state.
///
/// `tokio_rusqlite::Connection` is internally `Arc` + channel: calls from any task
/// are queued onto a dedicated DB thread, so it is `Clone + Send + Sync` and needs
/// no outer `Arc<Mutex<>>`.
///
/// `app_data_dir` is captured at startup and used by commands that need to
/// fall back to the convention project path (e.g. `create_project` with
/// `root_path = None`).
pub struct AppState {
    pub db: AsyncConnection,
    pub app_data_dir: PathBuf,
}
