//! End-to-end: submit a task wired to an inline stub provider, drain events
//! from the engine receiver until Success, then assert the DB row matches.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
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
    GenerationParams, ModelProvider, ProviderRegistry, ProviderTaskStatus,
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

/// Test-local stub. The crate-internal `provider::stub::StubProvider` is
/// `#[cfg(test)]`-gated to the lib crate, so integration tests roll their own.
#[derive(Default)]
struct TestStubProvider {
    polls: AtomicU8,
}

#[async_trait]
impl ModelProvider for TestStubProvider {
    async fn submit(&self, _params: GenerationParams) -> Result<String> {
        Ok("ext-test-1".into())
    }
    async fn poll(&self, _ext: &str) -> Result<ProviderTaskStatus> {
        let n = self.polls.fetch_add(1, Ordering::SeqCst);
        if n < 2 {
            Ok(ProviderTaskStatus::Running {
                progress: Some((n + 1) * 30),
            })
        } else {
            Ok(ProviderTaskStatus::Success {
                result_url: "stub://done".into(),
            })
        }
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
async fn submit_drives_to_success_and_persists() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("e2e.db");
    let db = open_async(&db_path).await.unwrap();

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
        .register("p1", Arc::new(TestStubProvider::default()))
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
            shot_id: None,
            provider_id: "p1".into(),
            model_id: "m1".into(),
            account_id: "a1".into(),
            task_type: TaskKind::Image,
            params_json: Some("{\"prompt\":\"hello\"}".into()),
        })
        .await
        .unwrap();

    timeout(Duration::from_secs(5), async {
        while let Some(ev) = rx.recv().await {
            let TaskEvent::StatusChanged { status, task_id: tid, .. } = ev;
            if tid == task_id && status == GenerationTaskStatus::Success {
                return;
            }
        }
        panic!("event stream closed before Success");
    })
    .await
    .expect("timeout waiting for Success event");

    let final_task = engine.get(&task_id).await.unwrap();
    assert_eq!(final_task.status, GenerationTaskStatus::Success);
    assert_eq!(final_task.external_task_id.as_deref(), Some("ext-test-1"));
    assert!(final_task.finished_at.is_some());
}
