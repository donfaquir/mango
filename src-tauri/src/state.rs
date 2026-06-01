use std::path::PathBuf;
use std::sync::Arc;

use mango_core::account::keyring::KeyringStore;
use mango_core::task_engine::TaskEngineHandle;
use tokio_rusqlite::Connection as AsyncConnection;

/// Tauri global managed state.
///
/// `tokio_rusqlite::Connection` is internally `Arc` + channel: calls from any task
/// are queued onto a dedicated DB thread, so it is `Clone + Send + Sync` and needs
/// no outer `Arc<Mutex<>>`.
///
/// `app_data_dir` is captured at startup and used by commands that need to
/// fall back to the convention project path (e.g. `create_project` with
/// `root_path = None`). Also where the pointer `config.json` lives so the
/// shell can find the user-chosen workspace at next boot.
///
/// `workspace_root` is the user-chosen workspace directory (which holds
/// `mango.db` and `projects/...`). `None` means the user has not picked a
/// workspace yet — the shell mounts a `:memory:` DB and the frontend should
/// route to the onboarding flow. Populated post-pick in the PR2 setup
/// refactor; PR1 only reserves the field so command code can start guarding
/// on it.
///
/// `keyring` is an `Arc<dyn KeyringStore>` so the production setup wires the
/// system keyring (via `keyring::use_native_store`) and tests can inject a
/// `InMemoryKeyring`.
///
/// `task_engine` is an `Arc<TaskEngineHandle>` because the handle itself is
/// already cheap to clone (its inner state is refcounted) but the forwarder
/// coroutine and command layer both need long-lived references; `Arc` keeps
/// the call sites uniform with `keyring`.
pub struct AppState {
    pub db: AsyncConnection,
    pub app_data_dir: PathBuf,
    pub workspace_root: Option<PathBuf>,
    pub keyring: Arc<dyn KeyringStore>,
    pub task_engine: Arc<TaskEngineHandle>,
}
