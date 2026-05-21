use tokio_rusqlite::Connection as AsyncConnection;

/// Tauri global managed state.
///
/// `tokio_rusqlite::Connection` is internally `Arc` + channel: calls from any task
/// are queued onto a dedicated DB thread, so it is `Clone + Send + Sync` and needs
/// no outer `Arc<Mutex<>>`.
pub struct AppState {
    pub db: AsyncConnection,
}
