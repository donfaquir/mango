//! Public engine API. The handle is `Clone`-able so commands can hold it inside
//! `AppState` and the runner coroutines hold their own copy. All inner state is
//! refcounted (`Arc<Semaphore>`, `tokio_rusqlite::Connection` is itself an Arc
//! over a worker thread).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::Semaphore;
use tokio_rusqlite::Connection as AsyncConnection;

use serde::Serialize;
use specta::Type;

use crate::account::keyring::KeyringStore;
use crate::db::queries::generation_task as q;
use crate::db::queries::generation_task_event as event_q;
use crate::error::Result;
use crate::models::generation_task::{
    CreateGenerationTaskInput, GenerationTask, GenerationTaskStatus,
};
use crate::models::generation_task_event::GenerationTaskEvent;
use crate::provider::registry::ProviderRegistry;

#[derive(Debug, Clone, Serialize, Type)]
pub struct SubmitBatchOutcome {
    /// UUID v4 shared by every task row created by this submission. Echoes
    /// the `generation_task.batch_id` column for downstream filtering.
    pub batch_id: String,
    /// Task IDs in the caller-supplied input order.
    pub task_ids: Vec<String>,
    #[specta(type = specta_typescript::Number)]
    pub created_count: usize,
}

use super::events::TaskEvent;
use super::materializer::ResultMaterializer;
use super::runner;

/// Default poll interval when [`TaskEngineHandle::spawn`] is used. Tests use
/// [`TaskEngineHandle::spawn_with`] to drive the loop faster.
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Default backoff schedule for retryable provider failures (spec-27 §3.2).
/// The Nth entry is slept before the (N+1)th attempt. Tests override this
/// with a near-zero schedule to keep the suite quick.
pub const DEFAULT_RETRY_BACKOFFS: &[Duration] = &[
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];

/// Filter passed to [`TaskEngineHandle::list`]. Uses the `project_id` column
/// directly on the `generation_task` table.
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
    /// Outer Arc lets the handle be cloned cheaply; the Mutex lets
    /// [`Self::set_max_concurrency`] swap the inner `Arc<Semaphore>` at
    /// runtime. Existing `OwnedSemaphorePermit`s hold their own strong
    /// reference to the old semaphore, so swapping never invalidates a
    /// permit in flight — it only changes which semaphore *new* acquires
    /// see (spec-27 §2).
    pub(super) semaphore: Arc<Mutex<Arc<Semaphore>>>,
    pub(super) event_tx: UnboundedSender<TaskEvent>,
    pub(super) keyring: Arc<dyn KeyringStore>,
    pub(super) materializer: Arc<dyn ResultMaterializer>,
    pub(super) poll_interval: Duration,
    /// Backoff schedule applied between auto-retry attempts. Length doubles
    /// as the retry cap: 3-entry default → 3 retries max (spec-27 §3.2).
    pub(super) retry_backoffs: Arc<Vec<Duration>>,
    /// Absolute path of the mounted workspace. Joined with the
    /// workspace-relative `project.root_path` whenever the runner needs to
    /// touch on-disk files (asset path resolution, downloaded result
    /// materialisation). Set at construction time by the shell after the
    /// user picks a workspace.
    pub(super) workspace_root: std::path::PathBuf,
}

impl TaskEngineHandle {
    /// Build the engine. Returns the handle plus the event receiver — the
    /// Tauri shell forwards events into `tauri-specta` Event::emit; the CLI
    /// can drop the receiver.
    pub fn spawn(
        db: AsyncConnection,
        providers: ProviderRegistry,
        keyring: Arc<dyn KeyringStore>,
        materializer: Arc<dyn ResultMaterializer>,
        workspace_root: std::path::PathBuf,
        max_concurrency: usize,
    ) -> (Self, UnboundedReceiver<TaskEvent>) {
        Self::spawn_with(
            db,
            providers,
            keyring,
            materializer,
            workspace_root,
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
        materializer: Arc<dyn ResultMaterializer>,
        workspace_root: std::path::PathBuf,
        max_concurrency: usize,
        poll_interval: Duration,
    ) -> (Self, UnboundedReceiver<TaskEvent>) {
        Self::spawn_full(
            db,
            providers,
            keyring,
            materializer,
            workspace_root,
            max_concurrency,
            poll_interval,
            DEFAULT_RETRY_BACKOFFS.to_vec(),
        )
    }

    /// Full constructor — every knob explicit. Tests pin both `poll_interval`
    /// and `retry_backoffs` to near-zero so the auto-retry suite finishes in
    /// well under a second.
    #[allow(clippy::too_many_arguments)] // wiring knobs for tests; not a public-facing builder
    pub fn spawn_full(
        db: AsyncConnection,
        providers: ProviderRegistry,
        keyring: Arc<dyn KeyringStore>,
        materializer: Arc<dyn ResultMaterializer>,
        workspace_root: std::path::PathBuf,
        max_concurrency: usize,
        poll_interval: Duration,
        retry_backoffs: Vec<Duration>,
    ) -> (Self, UnboundedReceiver<TaskEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let handle = Self {
            db,
            providers,
            semaphore: Arc::new(Mutex::new(Arc::new(Semaphore::new(max_concurrency)))),
            event_tx: tx,
            keyring,
            materializer,
            poll_interval,
            retry_backoffs: Arc::new(retry_backoffs),
            workspace_root,
        };
        (handle, rx)
    }

    /// Snapshot the current concurrency-limit semaphore. Runners call this
    /// once per task to acquire a permit. Holding a permit across a swap is
    /// safe because the old semaphore stays alive as long as any permit
    /// references it.
    pub(super) fn semaphore_snapshot(&self) -> Arc<Semaphore> {
        self.semaphore.lock().expect("semaphore mutex poisoned").clone()
    }

    /// Swap the concurrency-limit semaphore. New `acquire`s see the new cap
    /// immediately; permits already held by running tasks remain valid until
    /// they drop on their own (spec-27 §2 "已在跑的不中断").
    pub fn set_max_concurrency(&self, n: usize) {
        let mut guard = self.semaphore.lock().expect("semaphore mutex poisoned");
        *guard = Arc::new(Semaphore::new(n));
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

    /// Transactional batch submission: every (shot, model) row in `inputs`
    /// is inserted under a single shared `batch_id`. If any row fails
    /// validation or insertion, the whole transaction rolls back and no
    /// runner is spawned. On success, runners are spawned one-per-row in
    /// the caller-supplied order.
    ///
    /// The caller (frontend `BatchSubmitDialog`) is responsible for skipping
    /// shots with empty prompts and resolving character→asset references
    /// before calling this — core does not synthesize defaults here.
    pub async fn submit_batch(
        &self,
        inputs: Vec<CreateGenerationTaskInput>,
    ) -> Result<SubmitBatchOutcome> {
        let (batch_id, task_ids) = self
            .db
            .call(move |conn| Ok(q::create_batch(conn, &inputs)))
            .await
            .map_err(map_async_err)??;
        for id in &task_ids {
            tokio::spawn(runner::run(self.clone(), id.clone()));
        }
        let created_count = task_ids.len();
        Ok(SubmitBatchOutcome {
            batch_id,
            task_ids,
            created_count,
        })
    }

    /// Re-spawn runner coroutines for all tasks that are still `pending` in the DB.
    /// Called once at application startup to recover tasks that lost their runners
    /// due to a previous app exit/crash.
    pub async fn recover_pending(&self) -> Result<usize> {
        let ids: Vec<String> = self
            .db
            .call(|conn| Ok(q::list_pending_ids(conn)))
            .await
            .map_err(map_async_err)??;

        let count = ids.len();
        for id in ids {
            tokio::spawn(runner::run(self.clone(), id));
        }
        if count > 0 {
            tracing::info!("recovered {count} pending task(s) from previous session");
        }
        Ok(count)
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

    /// Return the full diagnostic event timeline for a task, ordered by
    /// `occurred_at` (insertion order).
    pub async fn list_events(&self, task_id: &str) -> Result<Vec<GenerationTaskEvent>> {
        let id = task_id.to_string();
        self.db
            .call(move |conn| Ok(event_q::list_by_task(conn, &id)))
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
