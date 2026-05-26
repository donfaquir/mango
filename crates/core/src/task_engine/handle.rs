//! Public engine API. The handle is `Clone`-able so commands can hold it inside
//! `AppState` and the runner coroutines hold their own copy. All inner state is
//! refcounted (`Arc<Semaphore>`, `tokio_rusqlite::Connection` is itself an Arc
//! over a worker thread).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::Semaphore;
use tokio_rusqlite::Connection as AsyncConnection;

use crate::account::keyring::KeyringStore;
use crate::db::queries::generation_task as q;
use crate::error::Result;
use crate::models::generation_task::{
    CreateGenerationTaskInput, GenerationTask, GenerationTaskStatus,
};
use crate::provider::registry::ProviderRegistry;

use super::events::TaskEvent;
use super::runner;

/// Default poll interval when [`TaskEngineHandle::spawn`] is used. Tests use
/// [`TaskEngineHandle::spawn_with`] to drive the loop faster.
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Filter passed to [`TaskEngineHandle::list`]. `project_id` joins through
/// `shot → episode → project`, so tasks with no shot are excluded when a
/// project filter is set.
#[derive(Debug, Default, Clone)]
pub struct ListFilter {
    pub project_id: Option<String>,
    pub status: Option<GenerationTaskStatus>,
    pub limit: Option<u32>,
}

#[derive(Clone)]
pub struct TaskEngineHandle {
    pub(super) db: AsyncConnection,
    pub(super) providers: ProviderRegistry,
    pub(super) semaphore: Arc<Semaphore>,
    pub(super) event_tx: UnboundedSender<TaskEvent>,
    pub(super) keyring: Arc<dyn KeyringStore>,
    pub(super) poll_interval: Duration,
}

impl TaskEngineHandle {
    /// Build the engine. Returns the handle plus the event receiver — the
    /// Tauri shell forwards events into `tauri-specta` Event::emit; the CLI
    /// can drop the receiver.
    pub fn spawn(
        db: AsyncConnection,
        providers: ProviderRegistry,
        keyring: Arc<dyn KeyringStore>,
        max_concurrency: usize,
    ) -> (Self, UnboundedReceiver<TaskEvent>) {
        Self::spawn_with(
            db,
            providers,
            keyring,
            max_concurrency,
            DEFAULT_POLL_INTERVAL,
        )
    }

    /// Variant that accepts a custom poll interval. Tests use a sub-second
    /// interval so the e2e suite finishes in <1s.
    pub fn spawn_with(
        db: AsyncConnection,
        providers: ProviderRegistry,
        keyring: Arc<dyn KeyringStore>,
        max_concurrency: usize,
        poll_interval: Duration,
    ) -> (Self, UnboundedReceiver<TaskEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let handle = Self {
            db,
            providers,
            semaphore: Arc::new(Semaphore::new(max_concurrency)),
            event_tx: tx,
            keyring,
            poll_interval,
        };
        (handle, rx)
    }

    pub async fn submit(&self, input: CreateGenerationTaskInput) -> Result<String> {
        let task = self
            .db
            .call(move |conn| Ok(q::create(conn, input)))
            .await
            .map_err(map_async_err)??;
        let id = task.id.clone();
        tokio::spawn(runner::run(self.clone(), id.clone()));
        Ok(id)
    }

    pub async fn cancel(&self, task_id: &str) -> Result<()> {
        let id = task_id.to_string();
        let task = self
            .db
            .call({
                let id = id.clone();
                move |conn| Ok(q::get_by_id(conn, &id))
            })
            .await
            .map_err(map_async_err)??;

        // Idempotent: terminal tasks accept cancel as a no-op so the UI does
        // not race itself when the user clicks Cancel just as the task lands.
        if task.status.is_terminal() {
            return Ok(());
        }

        let id_for_call = id.clone();
        let new_status = GenerationTaskStatus::Cancelled;
        self.db
            .call(move |conn| {
                Ok(q::transition_status(conn, &id_for_call, new_status, None))
            })
            .await
            .map_err(map_async_err)??;

        let _ = self.event_tx.send(TaskEvent::status_changed(
            id.clone(),
            new_status,
            None,
            None,
        ));

        // Best-effort provider cancel; ignore errors. Only meaningful when an
        // external_task_id was already obtained.
        if let Some(ext) = task.external_task_id
            && let Ok(p) = self.providers.get(&task.provider_id)
        {
            let _ = p.cancel(&ext).await;
        }
        Ok(())
    }

    pub async fn get(&self, task_id: &str) -> Result<GenerationTask> {
        let id = task_id.to_string();
        self.db
            .call(move |conn| Ok(q::get_by_id(conn, &id)))
            .await
            .map_err(map_async_err)?
    }

    pub async fn list(&self, filter: ListFilter) -> Result<Vec<GenerationTask>> {
        let ListFilter {
            project_id,
            status,
            limit,
        } = filter;
        self.db
            .call(move |conn| Ok(q::list(conn, project_id.as_deref(), status, limit)))
            .await
            .map_err(map_async_err)?
    }
}

fn map_async_err(e: tokio_rusqlite::Error) -> crate::error::CoreError {
    match e {
        tokio_rusqlite::Error::Error(inner) => crate::error::CoreError::Sqlite(inner),
        other => crate::error::CoreError::TaskEngine(format!("db worker error: {other}")),
    }
}
