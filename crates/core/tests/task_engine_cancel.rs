//! Cancel path: submit a slow stub provider, request cancel before it can
//! reach success, and verify the row lands in `Cancelled` within 1s.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use mango_core::account::keyring::KeyringStore;
use mango_core::db::open_async;
use mango_core::error::{CoreError, Result};
use mango_core::models::generation_task::{
    CreateGenerationTaskInput, GenerationTaskStatus, TaskKind,
};
use mango_core::provider::{
    GenerationParams, ModelProvider, PollOutcome, ProviderRegistry, ProviderTaskStatus,
    SubmitOutcome,
};
use mango_core::task_engine::{NoopMaterializer, TaskEngineHandle, TaskEvent};
use tempfile::tempdir;
use tokio::time::timeout;

#[derive(Default)]
struct LocalKeyring(Mutex<HashMap<String, String>>);

impl KeyringStore for LocalKeyring {
    fn store(&self, id: &str, key: &str) -> Result<()> {
        self.0.lock().unwrap().insert(id.into(), key.into());
        Ok(())
    }
    fn fetch(&self, id: &str) -> Result<String> {
        self.0
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| CoreError::Keyring(format!("not found: {id}")))
    }
    fn remove(&self, id: &str) -> Result<()> {
        self.0.lock().unwrap().remove(id);
        Ok(())
    }
}

/// Always reports Running. The runner only escapes via cancellation.
#[derive(Default)]
struct StuckProvider;

#[async_trait]
impl ModelProvider for StuckProvider {
    async fn submit(&self, _params: GenerationParams) -> Result<SubmitOutcome> {
        Ok(SubmitOutcome::new("ext-stuck"))
    }
    async fn poll(&self, _ext: &str) -> Result<PollOutcome> {
        Ok(PollOutcome::bare(ProviderTaskStatus::Running { progress: None }))
    }
    async fn cancel(&self, _ext: &str) -> Result<()> {
        Ok(())
    }
    async fn download(&self, _ext: &str, dest: &Path) -> Result<PathBuf> {
        Ok(dest.to_path_buf())
    }
}

#[tokio::test]
async fn cancel_running_task_lands_in_cancelled_state() {
    let dir = tempdir().unwrap();
    let db = open_async(&dir.path().join("cancel.db")).await.unwrap();

    db.call(|conn| {
        conn.execute_batch(
            "INSERT INTO provider (id, name) VALUES ('p1', 'P'); \
             INSERT INTO model (id, provider_id, name, model_type) \
                 VALUES ('m1', 'p1', 'M', 'image'); \
             INSERT INTO api_account (id, provider_id, label, api_key_ref, key_last4) \
                 VALUES ('a1', 'p1', 'L', 'api_account:a1', '0000');",
        )
        .map_err(tokio_rusqlite::Error::from)
    })
    .await
    .unwrap();

    let keyring: Arc<dyn KeyringStore> = Arc::new(LocalKeyring::default());
    keyring.store("a1", "secret").unwrap();

    let providers = ProviderRegistry::builder()
        .register("p1", Arc::new(StuckProvider))
        .build();

    let (engine, mut rx) = TaskEngineHandle::spawn_with(
        db.clone(),
        providers,
        keyring,
        Arc::new(NoopMaterializer),
        4,
        Duration::from_millis(50),
    );

    let task_id = engine
        .submit(CreateGenerationTaskInput {
            project_id: None,
            shot_id: None,
            provider_id: "p1".into(),
            model_id: "m1".into(),
            account_id: "a1".into(),
            task_type: TaskKind::Image,
            params_json: None,
        })
        .await
        .unwrap();

    // Wait for the runner to reach Running before cancelling so we exercise
    // the running→cancelled path (not pending→cancelled).
    timeout(Duration::from_secs(2), async {
        while let Some(ev) = rx.recv().await {
            if let TaskEvent::StatusChanged { status, task_id: tid, .. } = ev
                && tid == task_id
                && status == GenerationTaskStatus::Running
            {
                return;
            }
        }
        panic!("never observed Running");
    })
    .await
    .expect("timeout waiting for Running");

    engine.cancel(&task_id).await.unwrap();

    // The runner detects cancellation on the next poll tick; with a 50ms
    // interval this should land within 500ms.
    timeout(Duration::from_secs(2), async {
        loop {
            let task = engine.get(&task_id).await.unwrap();
            if task.status == GenerationTaskStatus::Cancelled {
                return task;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("timeout waiting for Cancelled");

    let final_task = engine.get(&task_id).await.unwrap();
    assert_eq!(final_task.status, GenerationTaskStatus::Cancelled);
    assert_eq!(final_task.retry_count, 0);
}
