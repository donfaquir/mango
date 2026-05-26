//! Per-task driver. One coroutine per submitted task: acquire concurrency slot,
//! resolve credentials, drive the provider through `submit → poll* → terminal`,
//! and persist transitions through [`super::state`]. Cancellation is observed
//! by re-reading the row before each poll.

use tokio::time::sleep;

use crate::error::{CoreError, Result};
use crate::models::generation_task::{GenerationTask, GenerationTaskStatus};
use crate::provider::traits::{
    GenerationParams, ModelProvider, ProviderCredentials, ProviderTaskStatus,
};

use super::handle::TaskEngineHandle;
use super::state;

pub async fn run(engine: TaskEngineHandle, task_id: String) {
    let _permit = match engine.semaphore.clone().acquire_owned().await {
        Ok(p) => p,
        Err(_) => {
            tracing::error!(task_id = %task_id, "semaphore closed; runner aborting");
            return;
        }
    };

    let task = match state::load(&engine.db, &task_id).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(task_id = %task_id, error = %e, "load failed; runner aborting");
            return;
        }
    };

    // The user may have cancelled while the permit was queued.
    if matches!(task.status, GenerationTaskStatus::Cancelled) {
        return;
    }

    let provider = match engine.providers.get(&task.provider_id) {
        Ok(p) => p,
        Err(e) => return fail(&engine, &task_id, &e.to_string()).await,
    };

    let credentials = match resolve_credentials(&engine, &task.account_id).await {
        Ok(c) => c,
        Err(e) => return fail(&engine, &task_id, &e.to_string()).await,
    };

    let params = match build_generation_params(&task, credentials) {
        Ok(p) => p,
        Err(e) => return fail(&engine, &task_id, &e.to_string()).await,
    };

    if let Err(e) = state::transition(
        &engine.db,
        &engine.event_tx,
        &task_id,
        GenerationTaskStatus::Running,
        None,
        None,
    )
    .await
    {
        tracing::error!(task_id = %task_id, error = %e, "transition→running failed");
        return;
    }

    let external_id = match provider.submit(params).await {
        Ok(id) => id,
        Err(e) => return fail(&engine, &task_id, &e.to_string()).await,
    };
    if let Err(e) = state::set_external_id(&engine.db, &task_id, &external_id).await {
        tracing::error!(task_id = %task_id, error = %e, "set_external_id failed");
        return fail(&engine, &task_id, &e.to_string()).await;
    }

    poll_loop(&engine, &task_id, provider.as_ref(), &external_id).await;
}

async fn poll_loop(
    engine: &TaskEngineHandle,
    task_id: &str,
    provider: &dyn ModelProvider,
    external_id: &str,
) {
    loop {
        sleep(engine.poll_interval).await;

        match state::is_cancelled(&engine.db, task_id).await {
            Ok(true) => {
                let _ = provider.cancel(external_id).await;
                return;
            }
            Ok(false) => {}
            Err(e) => {
                tracing::error!(task_id = %task_id, error = %e, "is_cancelled check failed");
                return;
            }
        }

        match provider.poll(external_id).await {
            Ok(ProviderTaskStatus::Pending) => continue,
            Ok(ProviderTaskStatus::Running { progress }) => {
                // Re-emit Running with updated progress; the DB row stays in
                // running state so transition_status would reject. We send the
                // event directly.
                let _ = engine.event_tx.send(super::events::TaskEvent::status_changed(
                    task_id,
                    GenerationTaskStatus::Running,
                    progress,
                    None,
                ));
                continue;
            }
            Ok(ProviderTaskStatus::Success { result_url }) => {
                // Materialize the result: download + persist as Asset.
                let task_for_mat = match state::load(&engine.db, task_id).await {
                    Ok(t) => t,
                    Err(e) => return fail(engine, task_id, &e.to_string()).await,
                };
                let asset_id = match engine.materializer.materialize(&task_for_mat, &result_url).await {
                    Ok(id) => id,
                    Err(e) => {
                        tracing::error!(task_id = %task_id, error = %e, "materialize failed");
                        return fail(engine, task_id, &format!("result download failed: {e}")).await;
                    }
                };

                // Write result_asset_id if materializer produced one.
                if !asset_id.is_empty()
                    && let Err(e) = state::set_result_asset_id(&engine.db, task_id, &asset_id).await
                {
                    tracing::error!(task_id = %task_id, error = %e, "set_result_asset_id failed");
                }

                if let Err(e) = state::transition(
                    &engine.db,
                    &engine.event_tx,
                    task_id,
                    GenerationTaskStatus::Success,
                    Some(100),
                    None,
                )
                .await
                {
                    tracing::error!(task_id = %task_id, error = %e, "transition→success failed");
                }
                tracing::info!(task_id = %task_id, %result_url, "task succeeded");

                // Best-effort cleanup (e.g. remove uploaded reference images from OSS).
                let materializer = engine.materializer.clone();
                let task_clone = task_for_mat;
                tokio::spawn(async move {
                    if let Err(e) = materializer.cleanup(&task_clone).await {
                        tracing::warn!(task_id = %task_clone.id, error = %e, "materializer cleanup failed");
                    }
                });
                return;
            }
            Ok(ProviderTaskStatus::Failed { message }) => {
                return fail(engine, task_id, &message).await;
            }
            Err(e) => return fail(engine, task_id, &e.to_string()).await,
        }
    }
}

async fn fail(engine: &TaskEngineHandle, task_id: &str, message: &str) {
    if let Err(e) = state::transition(
        &engine.db,
        &engine.event_tx,
        task_id,
        GenerationTaskStatus::Failed,
        None,
        Some(message.to_string()),
    )
    .await
    {
        tracing::error!(task_id = %task_id, error = %e, "transition→failed failed");
    }
}

async fn resolve_credentials(
    engine: &TaskEngineHandle,
    account_id: &str,
) -> Result<ProviderCredentials> {
    // spec-16: account::service::resolve_credentials assembles api_key plus
    // (optional) OSS extra_json from the keyring + params_json column. Run on
    // the DB worker thread so the sync `Connection` API stays in its lane.
    let id = account_id.to_string();
    let keyring = engine.keyring.clone();
    engine
        .db
        .call(move |conn| {
            Ok(crate::account::service::resolve_credentials(
                conn,
                keyring.as_ref(),
                &id,
            ))
        })
        .await
        .map_err(|e| match e {
            tokio_rusqlite::Error::Error(inner) => CoreError::Sqlite(inner),
            other => CoreError::TaskEngine(format!("db worker error: {other}")),
        })?
}

fn build_generation_params(
    task: &GenerationTask,
    credentials: ProviderCredentials,
) -> Result<GenerationParams> {
    let provider_params: serde_json::Value = serde_json::from_str(&task.params_json)
        .map_err(|e| CoreError::Validation(format!("params_json is not valid JSON: {e}")))?;
    let prompt = provider_params
        .get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Ok(GenerationParams {
        model_id: task.model_id.clone(),
        prompt,
        provider_params,
        credentials,
    })
}
