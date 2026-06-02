//! Resume / orphan-recovery: insert a task directly with
//! `external_task_id = Some(...)` (simulating a previous app session that
//! submitted to the cloud but died before the poll loop completed), boot a
//! fresh engine, call `recover_pending`, and assert the runner skipped
//! `submit` and drove the task to Success purely via polling.
//!
//! This guards the contract described in `db/queries/generation_task::
//! reset_orphan_running` and the resume early-return in `task_engine::
//! runner::run`: orphan recovery must not double-charge the cloud or
//! orphan the previous external_task_id.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use mango_core::account::keyring::KeyringStore;
use mango_core::db::open_async;
use mango_core::error::{CoreError, Result};
use mango_core::models::generation_task::GenerationTaskStatus;
use mango_core::provider::{
    GenerationParams, ModelProvider, PollOutcome, ProviderRegistry, ProviderTaskStatus,
    SubmitOutcome,
};
use mango_core::task_engine::{NoopMaterializer, TaskEngineHandle, TaskEvent};
use tempfile::tempdir;
use tokio::time::timeout;

#[derive(Default)]
struct LocalKeyring {
    inner: Mutex<HashMap<String, String>>,
}

impl KeyringStore for LocalKeyring {
    fn store(&self, id: &str, key: &str) -> Result<()> {
        self.inner
            .lock()
            .unwrap()
            .insert(id.to_string(), key.to_string());
        Ok(())
    }
    fn fetch(&self, id: &str) -> Result<String> {
        self.inner
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| CoreError::Keyring(format!("not found: {id}")))
    }
    fn remove(&self, id: &str) -> Result<()> {
        self.inner.lock().unwrap().remove(id);
        Ok(())
    }
}

/// Provider whose `submit` is a tripwire: if the resume path is broken and
/// the runner tries to re-submit, the test fails loudly here. `poll` walks
/// running → success the same way `task_engine_e2e` does.
#[derive(Default)]
struct ResumeOnlyProvider {
    polls: AtomicU8,
    submit_calls: AtomicU8,
}

#[async_trait]
impl ModelProvider for ResumeOnlyProvider {
    async fn submit(&self, _params: GenerationParams) -> Result<SubmitOutcome> {
        self.submit_calls.fetch_add(1, Ordering::SeqCst);
        // Returning an error here would also flag the bug, but a panic
        // surfaces the failure in the test summary far more loudly than
        // a buried CoreError::Provider in the timeline.
        panic!("resume path should not re-submit; provider.submit() was called");
    }
    async fn poll(&self, ext: &str) -> Result<PollOutcome> {
        // The runner must poll with the *preserved* external_task_id, not
        // some freshly-minted one (which would also be a bug).
        assert_eq!(ext, "ext-resumed-1", "poll called with wrong external_task_id");
        let n = self.polls.fetch_add(1, Ordering::SeqCst);
        let status = if n < 1 {
            ProviderTaskStatus::Running { progress: Some(50) }
        } else {
            ProviderTaskStatus::Success {
                result_url: "stub://resumed".into(),
            }
        };
        Ok(PollOutcome::bare(status))
    }
    async fn cancel(&self, _ext: &str) -> Result<()> {
        Ok(())
    }
    async fn download(&self, _ext: &str, dest: &Path) -> Result<PathBuf> {
        std::fs::write(dest, b"x")?;
        Ok(dest.to_path_buf())
    }
}

#[tokio::test]
async fn recover_pending_resumes_poll_without_resubmit() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("resume.db");
    let db = open_async(&db_path).await.unwrap();

    // Seed FK chain + insert a task that *already* has an external_task_id
    // (the previous session submitted, then was killed). We also reset the
    // status to 'pending' to mimic what `reset_orphan_running` does on
    // boot — `recover_pending` only spawns runners for pending rows.
    db.call(|conn| {
        conn.execute_batch(
            "INSERT INTO provider (id, name) VALUES ('p1', 'P'); \
             INSERT INTO model (id, provider_id, name, model_type) \
                 VALUES ('m1', 'p1', 'M', 'image'); \
             INSERT INTO api_account (id, provider_id, label, api_key_ref, key_last4) \
                 VALUES ('a1', 'p1', 'L', 'api_account:a1', '0000'); \
             INSERT INTO generation_task \
                 (id, provider_id, model_id, account_id, task_type, params_json, \
                  status, external_task_id) \
             VALUES ('task-resumed', 'p1', 'm1', 'a1', 'image', '{}', \
                     'pending', 'ext-resumed-1');",
        )
        .map_err(tokio_rusqlite::Error::from)
    })
    .await
    .unwrap();

    let keyring: Arc<dyn KeyringStore> = Arc::new(LocalKeyring::default());
    keyring.store("a1", "secret").unwrap();

    let provider = Arc::new(ResumeOnlyProvider::default());
    let provider_for_assert = provider.clone();
    let providers = ProviderRegistry::builder()
        .register("p1", provider as Arc<dyn ModelProvider>)
        .build();

    let (engine, mut rx) = TaskEngineHandle::spawn_with(
        db.clone(),
        providers,
        keyring,
        Arc::new(NoopMaterializer),
        std::path::PathBuf::new(),
        4,
        Duration::from_millis(50),
    );

    // `recover_pending` is what the shell runs on every boot. It's the
    // public entry point we want exercised — using `engine.submit()` here
    // would write a brand-new row instead of resuming the seeded one.
    let recovered = engine.recover_pending().await.unwrap();
    assert_eq!(recovered, 1, "the seeded pending task should be recovered");

    timeout(Duration::from_secs(5), async {
        while let Some(ev) = rx.recv().await {
            if let TaskEvent::StatusChanged { task_id, status, .. } = ev
                && task_id == "task-resumed"
                && status == GenerationTaskStatus::Success
            {
                return;
            }
        }
        panic!("event stream closed before Success");
    })
    .await
    .expect("timeout waiting for Success on resumed task");

    // The external_task_id stays the same — the runner did not overwrite
    // it with a fresh submit response.
    let final_task = engine.get("task-resumed").await.unwrap();
    assert_eq!(final_task.status, GenerationTaskStatus::Success);
    assert_eq!(
        final_task.external_task_id.as_deref(),
        Some("ext-resumed-1"),
        "external_task_id must be preserved across recovery"
    );

    // submit() was never called. (The panic inside `submit` would have
    // poisoned this assertion long before, but we belt-and-brace it.)
    assert_eq!(
        provider_for_assert.submit_calls.load(Ordering::SeqCst),
        0,
        "resume path must not call provider.submit()"
    );
}
