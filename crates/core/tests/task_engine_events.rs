//! Failure-path coverage for the diagnostic event stream.
//!
//! Two stub providers exercise the runner's error attribution:
//!  * `FailAtSubmitProvider` raises `CoreError::Provider` from `submit`, with
//!    a populated `ProviderErrorDetail` (request_id + http_status). The runner
//!    should record one `submit_call` error event carrying those fields, then
//!    transition the task to `Failed`.
//!  * `FailAtPollProvider` accepts the submit, returns `Running` once, and
//!    then yields `ProviderTaskStatus::Failed` on the next poll. The runner
//!    should emit `submit_call` info, `poll` info ("云端开始生成"), and a
//!    terminal `poll` error.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use mango_core::account::keyring::KeyringStore;
use mango_core::db::open_async;
use mango_core::db::queries::generation_task_event as event_queries;
use mango_core::error::{CoreError, Result};
use mango_core::models::generation_task::{
    CreateGenerationTaskInput, GenerationTaskStatus, TaskKind,
};
use mango_core::models::generation_task_event::{EventPhase, EventSeverity, GenerationTaskEvent};
use mango_core::provider::error::{ProviderErrorDetail, ProviderErrorKind};
use mango_core::provider::{
    GenerationParams, ModelProvider, PollOutcome, ProviderRegistry, ProviderTaskStatus,
    SubmitOutcome,
};
use mango_core::task_engine::{NoopMaterializer, TaskEngineHandle, TaskEvent};
use tempfile::tempdir;
use tokio::time::timeout;
use tokio_rusqlite::Connection as AsyncConnection;

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

/// Provider whose `submit` always returns a structured DashScope-style error.
struct FailAtSubmitProvider;

#[async_trait]
impl ModelProvider for FailAtSubmitProvider {
    async fn submit(&self, _params: GenerationParams) -> Result<SubmitOutcome> {
        let detail = ProviderErrorDetail::new(ProviderErrorKind::Auth, "认证失败")
            .with_http_status(401)
            .with_request_id(Some("rid-submit-fail".into()))
            .with_body_excerpt("{\"code\":\"InvalidApiKey\"}");
        Err(CoreError::Provider(detail))
    }
    async fn poll(&self, _ext: &str) -> Result<PollOutcome> {
        unreachable!("submit fails before poll is reached")
    }
    async fn cancel(&self, _ext: &str) -> Result<()> {
        Ok(())
    }
    async fn download(&self, _ext: &str, dest: &Path) -> Result<PathBuf> {
        Ok(dest.to_path_buf())
    }
}

/// Provider that submits cleanly, reports `Running` once, then `Failed`.
#[derive(Default)]
struct FailAtPollProvider {
    polls: AtomicU8,
}

#[async_trait]
impl ModelProvider for FailAtPollProvider {
    async fn submit(&self, _params: GenerationParams) -> Result<SubmitOutcome> {
        Ok(SubmitOutcome {
            external_task_id: "ext-pollfail".into(),
            request_id: Some("rid-submit-ok".into()),
            http_status: Some(200),
            upload: None,
        })
    }
    async fn poll(&self, _ext: &str) -> Result<PollOutcome> {
        let n = self.polls.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            Ok(PollOutcome {
                status: ProviderTaskStatus::Running { progress: Some(50) },
                request_id: Some("rid-poll-running".into()),
                http_status: Some(200),
            })
        } else {
            Ok(PollOutcome {
                status: ProviderTaskStatus::Failed {
                    message: "content policy 违规".into(),
                },
                request_id: Some("rid-poll-fail".into()),
                http_status: Some(200),
            })
        }
    }
    async fn cancel(&self, _ext: &str) -> Result<()> {
        Ok(())
    }
    async fn download(&self, _ext: &str, dest: &Path) -> Result<PathBuf> {
        Ok(dest.to_path_buf())
    }
}

async fn seed_db(db_path: &Path) -> AsyncConnection {
    let db = open_async(db_path).await.unwrap();
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
    db
}

async fn load_events(db: &AsyncConnection, task_id: &str) -> Vec<GenerationTaskEvent> {
    let id = task_id.to_string();
    db.call(move |conn| Ok::<_, rusqlite::Error>(event_queries::list_by_task(conn, &id)))
        .await
        .unwrap()
        .unwrap()
}

async fn await_failed(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<TaskEvent>,
    task_id: &str,
) {
    timeout(Duration::from_secs(5), async {
        while let Some(ev) = rx.recv().await {
            if let TaskEvent::StatusChanged { status, task_id: tid, .. } = ev
                && tid == task_id
                && status == GenerationTaskStatus::Failed
            {
                return;
            }
        }
        panic!("event stream closed before Failed");
    })
    .await
    .expect("timeout waiting for Failed event");
}

#[tokio::test]
async fn submit_failure_records_structured_error_event() {
    let dir = tempdir().unwrap();
    let db = seed_db(&dir.path().join("submit_fail.db")).await;

    let keyring: Arc<dyn KeyringStore> = Arc::new(LocalKeyring::default());
    keyring.store("a1", "secret").unwrap();

    let providers = ProviderRegistry::builder()
        .register("p1", Arc::new(FailAtSubmitProvider))
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

    let task_id = engine
        .submit(CreateGenerationTaskInput {
            project_id: None,
            shot_id: None,
            provider_id: "p1".into(),
            model_id: "m1".into(),
            account_id: "a1".into(),
            task_type: TaskKind::Image,
            params_json: Some("{\"prompt\":\"x\"}".into()),
        })
        .await
        .unwrap();

    await_failed(&mut rx, &task_id).await;

    let final_task = engine.get(&task_id).await.unwrap();
    assert_eq!(final_task.status, GenerationTaskStatus::Failed);

    let events = load_events(&db, &task_id).await;
    let submit_errors: Vec<&GenerationTaskEvent> = events
        .iter()
        .filter(|e| e.phase == EventPhase::SubmitCall && e.severity == EventSeverity::Error)
        .collect();
    assert_eq!(
        submit_errors.len(),
        1,
        "expected exactly 1 submit_call error event; got {events:?}"
    );
    let err = submit_errors[0];
    assert_eq!(err.request_id.as_deref(), Some("rid-submit-fail"));
    assert_eq!(err.http_status, Some(401));
    assert!(
        err.message.contains("认证失败"),
        "expected message to carry detail.message; got {:?}",
        err.message
    );
    // No info events should sneak in past the failure boundary.
    assert!(
        !events
            .iter()
            .any(|e| e.phase == EventPhase::Poll || e.phase == EventPhase::Persist),
        "no poll/persist events expected on submit-fail path; got {events:?}"
    );
}

#[tokio::test]
async fn poll_failure_records_phase_attribution_and_request_id() {
    let dir = tempdir().unwrap();
    let db = seed_db(&dir.path().join("poll_fail.db")).await;

    let keyring: Arc<dyn KeyringStore> = Arc::new(LocalKeyring::default());
    keyring.store("a1", "secret").unwrap();

    let providers = ProviderRegistry::builder()
        .register("p1", Arc::new(FailAtPollProvider::default()))
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

    let task_id = engine
        .submit(CreateGenerationTaskInput {
            project_id: None,
            shot_id: None,
            provider_id: "p1".into(),
            model_id: "m1".into(),
            account_id: "a1".into(),
            task_type: TaskKind::Image,
            params_json: Some("{\"prompt\":\"x\"}".into()),
        })
        .await
        .unwrap();

    await_failed(&mut rx, &task_id).await;

    let final_task = engine.get(&task_id).await.unwrap();
    assert_eq!(final_task.status, GenerationTaskStatus::Failed);
    assert_eq!(final_task.external_task_id.as_deref(), Some("ext-pollfail"));

    let events = load_events(&db, &task_id).await;

    // Submit call info should fire before polling kicks off.
    assert!(
        events
            .iter()
            .any(|e| e.phase == EventPhase::SubmitCall && e.severity == EventSeverity::Info),
        "expected submit_call info event; got {events:?}"
    );

    // First Running transition should emit a poll info ("云端开始生成").
    let running_info = events
        .iter()
        .find(|e| e.phase == EventPhase::Poll && e.severity == EventSeverity::Info);
    assert!(
        running_info.is_some(),
        "expected poll info on first Running transition; got {events:?}"
    );

    // Terminal failure must record a poll error carrying the upstream request_id.
    let poll_errors: Vec<&GenerationTaskEvent> = events
        .iter()
        .filter(|e| e.phase == EventPhase::Poll && e.severity == EventSeverity::Error)
        .collect();
    assert_eq!(
        poll_errors.len(),
        1,
        "expected exactly 1 poll error event; got {events:?}"
    );
    let poll_err = poll_errors[0];
    assert_eq!(poll_err.request_id.as_deref(), Some("rid-poll-fail"));
    assert_eq!(poll_err.http_status, Some(200));
    assert!(
        poll_err.message.contains("content policy"),
        "expected upstream message to surface; got {:?}",
        poll_err.message
    );
}
