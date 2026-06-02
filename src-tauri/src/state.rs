use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use mango_core::account::keyring::KeyringStore;
use mango_core::task_engine::TaskEngineHandle;
use tokio_rusqlite::Connection as AsyncConnection;

/// Tauri global managed state.
///
/// Split into two halves: immutable pre-mount fields (always available) and
/// `mounted`, a `OnceLock<MountedState>` that is filled exactly once when
/// the user picks a workspace.
///
/// Why `OnceLock` instead of a `RwLock` / `ArcSwap`: workspace can only be
/// chosen once per process lifetime — switching to a different workspace
/// goes through `set_workspace_and_relaunch` (write config + exit; user
/// relaunches). That matches `OnceLock`'s "set once, read freely" semantic
/// exactly, and gives us lock-free reads on the hot path with no `await`.
///
/// `app_data_dir` is captured at startup and used to read/write the pointer
/// config (`<app_data>/config.json`). It is always available, even before
/// a workspace is mounted, because the pointer file lives outside the
/// workspace.
///
/// `keyring` is an `Arc<dyn KeyringStore>` so the production setup wires
/// the system keyring (via `keyring::use_native_store`) and tests can
/// inject `InMemoryKeyring`. API keys never depend on a mounted workspace,
/// so this stays at the top level.
pub struct AppState {
    pub app_data_dir: PathBuf,
    pub keyring: Arc<dyn KeyringStore>,
    pub mounted: OnceLock<MountedState>,
}

/// State that only exists once a workspace has been mounted. Filled by
/// `init_workspace` (called from `setup()` on a happy boot, or from the
/// `mount_workspace` IPC command on first-time onboarding).
pub struct MountedState {
    /// Async connection to `<workspace>/mango.db`. Cheap to clone — the
    /// inner channel handle is `Arc`.
    pub db: AsyncConnection,
    /// Absolute path of the mounted workspace. Joined with the
    /// workspace-relative `project.root_path` whenever the runner or
    /// the import pipeline needs to touch on-disk files.
    pub workspace_root: PathBuf,
    /// Task engine spawned against the mounted DB and workspace. `Arc` so
    /// command code can clone the handle cheaply per call.
    pub task_engine: Arc<TaskEngineHandle>,
}
