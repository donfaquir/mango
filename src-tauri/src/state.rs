use std::path::PathBuf;
use std::sync::Arc;

use mango_core::account::keyring::KeyringStore;
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
///
/// `keyring` is an `Arc<dyn KeyringStore>` so the production setup wires the
/// system keyring (via `keyring::use_native_store`) and tests can inject a
/// `InMemoryKeyring`.
pub struct AppState {
    pub db: AsyncConnection,
    pub app_data_dir: PathBuf,
    pub keyring: Arc<dyn KeyringStore>,
}
