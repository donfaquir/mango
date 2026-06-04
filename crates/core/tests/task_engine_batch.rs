//! spec-26: submit_batch transactional insert + shared batch_id, and runners
//! drive every row in the batch to Success.

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

#[derive(Default)]
struct AlwaysSuccessProvider;

#[async_trait]
impl ModelProvider for AlwaysSuccessProvider {
    async fn submit(&self, _params: GenerationParams) -> Result<SubmitOutcome> {
        Ok(SubmitOutcome::new("ext-stub"))
    }
    async fn poll(&self, _ext: &str) -> Result<PollOutcome> {
        Ok(PollOutcome::bare(ProviderTaskStatus::Success {
            result_url: "stub://done".into(),
        }))
    }
    async fn cancel(&self, _ext: &str) -> Result<()> {
        Ok(())
    }
    async fn download(&self, _ext: &str, dest: &Path) -> Result<PathBuf> {
        std::fs::write(dest, b"x")?;
        Ok(dest.to_path_buf())
    }
}

fn make_input(provider: &str, model: &str, account: &str) -> CreateGenerationTaskInput {
    CreateGenerationTaskInput {
        project_id: None,
        shot_id: None,
        provider_id: provider.into(),
        model_id: model.into(),
        account_id: account.into(),
        task_type: TaskKind::Image,
        params_json: Some("{\"prompt\":\"hello\"}".into()),
    }
}

#[tokio::test]
async fn submit_batch_assigns_shared_batch_id_and_runs_all() {
    let dir = tempdir().unwrap();
    let db = open_async(&dir.path().join("batch.db")).await.unwrap();

    db.call(|conn| {
        conn.execute_batch(
            "INSERT INTO provider (id, name) VALUES ('p1', 'P'); \
             INSERT INTO model (id, provider_id, name, model_type) \
                 VALUES ('m1', 'p1', 'M1', 'image'); \
             INSERT INTO model (id, provider_id, name, model_type) \
                 VALUES ('m2', 'p1', 'M2', 'image'); \
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
        .register("p1", Arc::new(AlwaysSuccessProvider))
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

    // 2 shots × 2 models = 4 inputs (shot_id stays None — engine doesn't care).
    let inputs = vec![
        CreateGenerationTaskInput { model_id: "m1".into(), ..make_input("p1", "m1", "a1") },
        CreateGenerationTaskInput { model_id: "m2".into(), ..make_input("p1", "m2", "a1") },
        CreateGenerationTaskInput { model_id: "m1".into(), ..make_input("p1", "m1", "a1") },
        CreateGenerationTaskInput { model_id: "m2".into(), ..make_input("p1", "m2", "a1") },
    ];
    let outcome = engine.submit_batch(inputs).await.unwrap();
    assert_eq!(outcome.created_count, 4);
    assert_eq!(outcome.task_ids.len(), 4);

    // All rows share the same batch_id.
    for id in &outcome.task_ids {
        let row = engine.get(id).await.unwrap();
        assert_eq!(row.batch_id.as_deref(), Some(outcome.batch_id.as_str()));
    }

    // Wait for every task to reach Success.
    let mut remaining: std::collections::HashSet<String> =
        outcome.task_ids.iter().cloned().collect();
    timeout(Duration::from_secs(5), async {
        while let Some(ev) = rx.recv().await {
            if let TaskEvent::StatusChanged {
                task_id, status, ..
            } = ev
                && status == GenerationTaskStatus::Success
            {
                remaining.remove(&task_id);
                if remaining.is_empty() {
                    return;
                }
            }
        }
        panic!("event stream closed before all batch tasks succeeded");
    })
    .await
    .expect("timeout waiting for all batch tasks");

    // MS2 single submit alongside batch must keep batch_id NULL.
    let solo = engine
        .submit(make_input("p1", "m1", "a1"))
        .await
        .unwrap();
    let row = engine.get(&solo).await.unwrap();
    assert!(row.batch_id.is_none());
}

#[tokio::test]
async fn submit_batch_rejects_empty() {
    let dir = tempdir().unwrap();
    let db = open_async(&dir.path().join("empty.db")).await.unwrap();

    let keyring: Arc<dyn KeyringStore> = Arc::new(LocalKeyring::default());
    let providers = ProviderRegistry::builder().build();
    let (engine, _rx) = TaskEngineHandle::spawn_with(
        db,
        providers,
        keyring,
        Arc::new(NoopMaterializer),
        std::path::PathBuf::new(),
        2,
        Duration::from_millis(50),
    );

    let r = engine.submit_batch(Vec::new()).await;
    assert!(matches!(r, Err(CoreError::Validation(_))));
}
