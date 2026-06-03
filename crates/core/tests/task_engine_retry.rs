//! spec-27 §3 — auto-retry: retryable kinds escalate via backoff up to a cap,
//! non-retryable kinds fail immediately, and the configurable semaphore caps
//! concurrent runners.

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
use mango_core::provider::error::{ProviderErrorDetail, ProviderErrorKind};
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

/// Fails the first `failures_until_success` poll calls with a Network error,
/// then returns Success. Submit always succeeds. Used to exercise the retry
/// loop while keeping the e2e clock under a second.
struct FlakyProvider {
    failures_until_success: u8,
    poll_calls: AtomicU8,
}

#[async_trait]
impl ModelProvider for FlakyProvider {
    async fn submit(&self, _params: GenerationParams) -> Result<SubmitOutcome> {
        Ok(SubmitOutcome::new("ext-flaky"))
    }
    async fn poll(&self, _ext: &str) -> Result<PollOutcome> {
        let n = self.poll_calls.fetch_add(1, Ordering::SeqCst);
        if n < self.failures_until_success {
            Err(CoreError::Provider(ProviderErrorDetail::new(
                ProviderErrorKind::Network,
                "连接超时",
            )))
        } else {
            Ok(PollOutcome::bare(ProviderTaskStatus::Success {
                result_url: "stub://done".into(),
            }))
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

/// Always responds with an Auth error on submit. Used to prove non-retryable
/// kinds skip the retry path entirely.
struct AuthFailProvider;

#[async_trait]
impl ModelProvider for AuthFailProvider {
    async fn submit(&self, _params: GenerationParams) -> Result<SubmitOutcome> {
        Err(CoreError::Provider(
            ProviderErrorDetail::new(ProviderErrorKind::Auth, "API key 已过期")
                .with_http_status(401),
        ))
    }
    async fn poll(&self, _ext: &str) -> Result<PollOutcome> {
        Ok(PollOutcome::bare(ProviderTaskStatus::Failed {
            message: "n/a".into(),
        }))
    }
    async fn cancel(&self, _ext: &str) -> Result<()> {
        Ok(())
    }
    async fn download(&self, _ext: &str, dest: &Path) -> Result<PathBuf> {
        Ok(dest.to_path_buf())
    }
}

/// Hangs on the first `pending_until_release` polls so we can observe the
/// semaphore cap (concurrent tasks parked vs. running). Once `release()` is
/// called, every subsequent poll returns Success.
struct GatedProvider {
    pending_until_release: Arc<std::sync::atomic::AtomicBool>,
}

#[async_trait]
impl ModelProvider for GatedProvider {
    async fn submit(&self, _params: GenerationParams) -> Result<SubmitOutcome> {
        Ok(SubmitOutcome::new("ext-gated"))
    }
    async fn poll(&self, _ext: &str) -> Result<PollOutcome> {
        if self
            .pending_until_release
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Ok(PollOutcome::bare(ProviderTaskStatus::Pending));
        }
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
        params_json: Some("{\"prompt\":\"hi\"}".into()),
    }
}

async fn seed_provider_chain(db: &tokio_rusqlite::Connection) {
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
}

#[tokio::test]
async fn retries_network_errors_until_success() {
    let dir = tempdir().unwrap();
    let db = open_async(&dir.path().join("retry.db")).await.unwrap();
    seed_provider_chain(&db).await;

    let keyring: Arc<dyn KeyringStore> = Arc::new(LocalKeyring::default());
    keyring.store("a1", "secret").unwrap();

    let providers = ProviderRegistry::builder()
        .register(
            "p1",
            Arc::new(FlakyProvider {
                failures_until_success: 2,
                poll_calls: AtomicU8::new(0),
            }),
        )
        .build();

    let (engine, mut rx) = TaskEngineHandle::spawn_full(
        db.clone(),
        providers,
        keyring,
        Arc::new(NoopMaterializer),
        std::path::PathBuf::new(),
        4,
        Duration::from_millis(20),
        // Near-zero backoffs so the test finishes promptly.
        vec![
            Duration::from_millis(10),
            Duration::from_millis(10),
            Duration::from_millis(10),
        ],
    );

    let task_id = engine
        .submit(make_input("p1", "m1", "a1"))
        .await
        .unwrap();

    // Wait until the task settles in Success.
    timeout(Duration::from_secs(5), async {
        while let Some(ev) = rx.recv().await {
            if let TaskEvent::StatusChanged {
                task_id: tid,
                status,
                ..
            } = ev
                && tid == task_id
                && status == GenerationTaskStatus::Success
            {
                return;
            }
        }
        panic!("event stream closed before Success");
    })
    .await
    .expect("timeout waiting for Success after retries");

    let final_task = engine.get(&task_id).await.unwrap();
    assert_eq!(final_task.status, GenerationTaskStatus::Success);
    // 2 transient network failures escalated through 2 retries — the third
    // poll attempt was the one that succeeded.
    assert_eq!(final_task.retry_count, 2);
}

#[tokio::test]
async fn auth_errors_do_not_retry() {
    let dir = tempdir().unwrap();
    let db = open_async(&dir.path().join("auth.db")).await.unwrap();
    seed_provider_chain(&db).await;

    let keyring: Arc<dyn KeyringStore> = Arc::new(LocalKeyring::default());
    keyring.store("a1", "secret").unwrap();

    let providers = ProviderRegistry::builder()
        .register("p1", Arc::new(AuthFailProvider))
        .build();

    let (engine, mut rx) = TaskEngineHandle::spawn_full(
        db.clone(),
        providers,
        keyring,
        Arc::new(NoopMaterializer),
        std::path::PathBuf::new(),
        4,
        Duration::from_millis(20),
        vec![Duration::from_millis(10); 3],
    );

    let task_id = engine
        .submit(make_input("p1", "m1", "a1"))
        .await
        .unwrap();

    timeout(Duration::from_secs(2), async {
        while let Some(ev) = rx.recv().await {
            if let TaskEvent::StatusChanged {
                task_id: tid,
                status,
                ..
            } = ev
                && tid == task_id
                && status == GenerationTaskStatus::Failed
            {
                return;
            }
        }
        panic!("event stream closed before Failed");
    })
    .await
    .expect("timeout waiting for terminal Failed on auth error");

    let final_task = engine.get(&task_id).await.unwrap();
    assert_eq!(final_task.status, GenerationTaskStatus::Failed);
    // Auth is not retryable — retry_count must remain 0.
    assert_eq!(final_task.retry_count, 0);
}

#[tokio::test]
async fn semaphore_caps_concurrent_runners_at_one() {
    let dir = tempdir().unwrap();
    let db = open_async(&dir.path().join("sem.db")).await.unwrap();
    seed_provider_chain(&db).await;

    let keyring: Arc<dyn KeyringStore> = Arc::new(LocalKeyring::default());
    keyring.store("a1", "secret").unwrap();

    let gate = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let providers = ProviderRegistry::builder()
        .register(
            "p1",
            Arc::new(GatedProvider {
                pending_until_release: gate.clone(),
            }),
        )
        .build();

    // Cap concurrency at 1 — only one runner may be past `acquire` at a time.
    let (engine, _rx) = TaskEngineHandle::spawn_with(
        db.clone(),
        providers,
        keyring,
        Arc::new(NoopMaterializer),
        std::path::PathBuf::new(),
        1,
        Duration::from_millis(20),
    );

    // Fire 3 tasks; only 1 should enter running, 2 should remain pending
    // waiting on the semaphore.
    let ids = vec![
        engine.submit(make_input("p1", "m1", "a1")).await.unwrap(),
        engine.submit(make_input("p1", "m1", "a1")).await.unwrap(),
        engine.submit(make_input("p1", "m1", "a1")).await.unwrap(),
    ];

    // Give the runners a moment to acquire. With the semaphore at 1 and the
    // gate held, exactly one task transitions to running; the others stay
    // pending until the in-flight one releases its permit.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut running = 0usize;
    let mut pending = 0usize;
    for id in &ids {
        let row = engine.get(id).await.unwrap();
        match row.status {
            GenerationTaskStatus::Running => running += 1,
            GenerationTaskStatus::Pending => pending += 1,
            _ => {}
        }
    }
    assert_eq!(running, 1, "expected exactly 1 running, got {running}");
    assert_eq!(pending, 2, "expected exactly 2 pending, got {pending}");

    // Release the gate so the runners can drain; we don't assert here, just
    // tidy up the tokio runtime by giving in-flight tasks a chance to finish.
    gate.store(false, std::sync::atomic::Ordering::SeqCst);
}

/// Fails the first `failures_until_success` poll calls with an Unknown error
/// paired with HTTP 503, then returns Success. Used to verify the
/// "Unknown + 5xx → retryable" special case in `pick_retry_delay`.
struct ServerErrorFlakyProvider {
    failures_until_success: u8,
    poll_calls: AtomicU8,
}

#[async_trait]
impl ModelProvider for ServerErrorFlakyProvider {
    async fn submit(&self, _params: GenerationParams) -> Result<SubmitOutcome> {
        Ok(SubmitOutcome::new("ext-503"))
    }
    async fn poll(&self, _ext: &str) -> Result<PollOutcome> {
        let n = self.poll_calls.fetch_add(1, Ordering::SeqCst);
        if n < self.failures_until_success {
            Err(CoreError::Provider(
                ProviderErrorDetail::new(ProviderErrorKind::Unknown, "internal server error")
                    .with_http_status(503),
            ))
        } else {
            Ok(PollOutcome::bare(ProviderTaskStatus::Success {
                result_url: "stub://done".into(),
            }))
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
async fn unknown_5xx_errors_are_retried() {
    let dir = tempdir().unwrap();
    let db = open_async(&dir.path().join("5xx.db")).await.unwrap();
    seed_provider_chain(&db).await;

    let keyring: Arc<dyn KeyringStore> = Arc::new(LocalKeyring::default());
    keyring.store("a1", "secret").unwrap();

    let providers = ProviderRegistry::builder()
        .register(
            "p1",
            Arc::new(ServerErrorFlakyProvider {
                failures_until_success: 2,
                poll_calls: AtomicU8::new(0),
            }),
        )
        .build();

    let (engine, mut rx) = TaskEngineHandle::spawn_full(
        db.clone(),
        providers,
        keyring,
        Arc::new(NoopMaterializer),
        std::path::PathBuf::new(),
        4,
        Duration::from_millis(20),
        vec![
            Duration::from_millis(10),
            Duration::from_millis(10),
            Duration::from_millis(10),
        ],
    );

    let task_id = engine
        .submit(make_input("p1", "m1", "a1"))
        .await
        .unwrap();

    timeout(Duration::from_secs(5), async {
        while let Some(ev) = rx.recv().await {
            if let TaskEvent::StatusChanged {
                task_id: tid,
                status,
                ..
            } = ev
                && tid == task_id
                && status == GenerationTaskStatus::Success
            {
                return;
            }
        }
        panic!("event stream closed before Success");
    })
    .await
    .expect("timeout waiting for Success after 5xx retries");

    let final_task = engine.get(&task_id).await.unwrap();
    assert_eq!(final_task.status, GenerationTaskStatus::Success);
    assert_eq!(final_task.retry_count, 2);
}

#[tokio::test]
async fn retry_exhaustion_caps_at_max_then_fails() {
    let dir = tempdir().unwrap();
    let db = open_async(&dir.path().join("exhaust.db")).await.unwrap();
    seed_provider_chain(&db).await;

    let keyring: Arc<dyn KeyringStore> = Arc::new(LocalKeyring::default());
    keyring.store("a1", "secret").unwrap();

    let providers = ProviderRegistry::builder()
        .register(
            "p1",
            Arc::new(FlakyProvider {
                failures_until_success: 100,
                poll_calls: AtomicU8::new(0),
            }),
        )
        .build();

    let (engine, mut rx) = TaskEngineHandle::spawn_full(
        db.clone(),
        providers,
        keyring,
        Arc::new(NoopMaterializer),
        std::path::PathBuf::new(),
        4,
        Duration::from_millis(20),
        vec![
            Duration::from_millis(10),
            Duration::from_millis(10),
            Duration::from_millis(10),
        ],
    );

    let task_id = engine
        .submit(make_input("p1", "m1", "a1"))
        .await
        .unwrap();

    timeout(Duration::from_secs(5), async {
        while let Some(ev) = rx.recv().await {
            if let TaskEvent::StatusChanged {
                task_id: tid,
                status,
                ..
            } = ev
                && tid == task_id
                && status == GenerationTaskStatus::Failed
            {
                return;
            }
        }
        panic!("event stream closed before Failed");
    })
    .await
    .expect("timeout waiting for terminal Failed after retries exhausted");

    let final_task = engine.get(&task_id).await.unwrap();
    assert_eq!(final_task.status, GenerationTaskStatus::Failed);
    assert_eq!(final_task.retry_count, 3);
}

#[tokio::test]
async fn set_max_concurrency_allows_new_tasks_at_higher_cap() {
    let dir = tempdir().unwrap();
    let db = open_async(&dir.path().join("resize.db")).await.unwrap();
    seed_provider_chain(&db).await;

    let keyring: Arc<dyn KeyringStore> = Arc::new(LocalKeyring::default());
    keyring.store("a1", "secret").unwrap();

    let gate = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let providers = ProviderRegistry::builder()
        .register(
            "p1",
            Arc::new(GatedProvider {
                pending_until_release: gate.clone(),
            }),
        )
        .build();

    let (engine, _rx) = TaskEngineHandle::spawn_with(
        db.clone(),
        providers,
        keyring,
        Arc::new(NoopMaterializer),
        std::path::PathBuf::new(),
        1,
        Duration::from_millis(20),
    );

    // Submit task A under cap=1 — it acquires the sole permit and runs.
    let id_a = engine.submit(make_input("p1", "m1", "a1")).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let a = engine.get(&id_a).await.unwrap();
    assert_eq!(
        a.status,
        GenerationTaskStatus::Running,
        "task A should be running under cap=1"
    );

    // Raise cap to 3. New submissions see the fresh semaphore.
    engine.set_max_concurrency(3);

    let id_b = engine.submit(make_input("p1", "m1", "a1")).await.unwrap();
    let id_c = engine.submit(make_input("p1", "m1", "a1")).await.unwrap();
    let id_d = engine.submit(make_input("p1", "m1", "a1")).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut new_running = 0usize;
    for id in [&id_b, &id_c, &id_d] {
        if engine.get(id).await.unwrap().status == GenerationTaskStatus::Running {
            new_running += 1;
        }
    }
    assert_eq!(
        new_running, 3,
        "all 3 new tasks should run after raising cap to 3"
    );

    gate.store(false, std::sync::atomic::Ordering::SeqCst);
}
