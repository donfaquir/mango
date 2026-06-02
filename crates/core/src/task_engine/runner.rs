//! Per-task driver. One coroutine per submitted task: acquire concurrency slot,
//! resolve credentials, drive the provider through `submit → poll* → terminal`,
//! and persist transitions through [`super::state`]. Cancellation is observed
//! by re-reading the row before each poll.

use std::path::Path;

use tokio::time::sleep;

use crate::db::queries::{asset as asset_q, project as project_q};
use crate::error::{CoreError, Result};
use crate::models::generation_task::{GenerationTask, GenerationTaskStatus};
use crate::models::generation_task_event::{EventPhase, EventSeverity};
use crate::provider::error::{ProviderErrorDetail, ProviderErrorKind};
use crate::provider::traits::{
    GenerationParams, ModelProvider, PollOutcome, ProviderCredentials, ProviderTaskStatus,
    SubmitOutcome,
};

use super::handle::TaskEngineHandle;
use super::state::{self, EventBuilder};

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
        Err(e) => {
            return fail_with_detail(
                &engine,
                &task_id,
                EventPhase::SubmitCall,
                ProviderErrorDetail::new(ProviderErrorKind::Unknown, e.to_string()),
            )
            .await
        }
    };

    let credentials = match resolve_credentials(&engine, &task.account_id).await {
        Ok(c) => c,
        Err(e) => {
            return fail_with_detail(
                &engine,
                &task_id,
                EventPhase::SubmitUpload,
                detail_from_error(&e),
            )
            .await
        }
    };

    // Keep a clone for the cleanup phase so the materializer can reuse
    // already-resolved credentials without hitting the keyring again.
    let credentials_for_cleanup = credentials.clone();

    let mut params = match build_generation_params(&task, credentials) {
        Ok(p) => p,
        Err(e) => {
            return fail_with_detail(
                &engine,
                &task_id,
                EventPhase::SubmitCall,
                detail_from_error(&e),
            )
            .await
        }
    };

    // Resolve asset_id references in media[] to absolute local paths.
    if let Err(e) = resolve_media_paths(&engine, &task, &mut params).await {
        return fail_with_detail(
            &engine,
            &task_id,
            EventPhase::SubmitUpload,
            detail_from_error(&e),
        )
        .await;
    }

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

    let outcome: SubmitOutcome = match provider.submit(params).await {
        Ok(o) => o,
        Err(e) => {
            // CoreError variant tells us which phase to attribute the failure
            // to. Upload errors live in `Upload`, provider HTTP/protocol errors
            // in `Provider`. Anything else collapses to `submit_call`.
            let phase = match &e {
                CoreError::Upload(_) => EventPhase::SubmitUpload,
                _ => EventPhase::SubmitCall,
            };
            return fail_with_detail(&engine, &task_id, phase, detail_from_error(&e)).await;
        }
    };

    // Persist uploaded_remote_ids so the materializer can clean them up after
    // the task reaches a terminal state, and emit a `submit_upload` info
    // event with aggregate stats.
    if let Some(upload) = &outcome.upload {
        if !upload.remote_ids.is_empty()
            && let Err(e) = persist_uploaded_remote_ids(&engine, &task_id, &upload.remote_ids).await
        {
            tracing::warn!(task_id = %task_id, error = %e, "failed to persist uploaded_remote_ids");
        }
        let mb = (upload.total_bytes as f64) / 1_048_576.0;
        let secs = (upload.duration_ms as f64) / 1000.0;
        let message = format!(
            "上传参考图 ({n} 张, {mb:.2} MB, {secs:.1}s)",
            n = upload.count,
        );
        let _ = state::record_event(
            &engine.db,
            &engine.event_tx,
            &task_id,
            EventPhase::SubmitUpload,
            EventSeverity::Info,
            message,
            EventBuilder::new().details(serde_json::json!({
                "count": upload.count,
                "total_bytes": upload.total_bytes,
                "duration_ms": upload.duration_ms,
            })),
        )
        .await;
    }

    // submit_call info — DashScope accepted the job. Always emitted.
    let _ = state::record_event(
        &engine.db,
        &engine.event_tx,
        &task_id,
        EventPhase::SubmitCall,
        EventSeverity::Info,
        "提交到云端".to_string(),
        EventBuilder::new()
            .maybe_request_id(outcome.request_id.clone())
            .maybe_http_status(outcome.http_status)
            .details(serde_json::json!({
                "external_task_id": outcome.external_task_id,
            })),
    )
    .await;

    if let Err(e) = state::set_external_id(&engine.db, &task_id, &outcome.external_task_id).await {
        tracing::error!(task_id = %task_id, error = %e, "set_external_id failed");
        return fail_with_detail(
            &engine,
            &task_id,
            EventPhase::SubmitCall,
            detail_from_error(&e),
        )
        .await;
    }

    poll_loop(
        &engine,
        &task_id,
        provider.as_ref(),
        &outcome.external_task_id,
        credentials_for_cleanup,
    )
    .await;
}

async fn poll_loop(
    engine: &TaskEngineHandle,
    task_id: &str,
    provider: &dyn ModelProvider,
    external_id: &str,
    credentials: ProviderCredentials,
) {
    // Track whether we've already emitted "云端开始生成" for this task so the
    // poll matrix only fires it on the *first* transition into Running.
    let mut running_announced = false;

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

        let poll_result: Result<PollOutcome> = provider.poll(external_id).await;
        let poll_outcome = match poll_result {
            Ok(o) => o,
            Err(e) => {
                // Transient failure inside poll. Emit a `poll` warn event but
                // do NOT terminate the task — the next tick may succeed.
                let detail = detail_from_error(&e);
                let _ = state::record_event(
                    &engine.db,
                    &engine.event_tx,
                    task_id,
                    EventPhase::Poll,
                    EventSeverity::Warn,
                    format!("轮询失败: {}", detail.message),
                    EventBuilder::from_detail(&detail),
                )
                .await;
                continue;
            }
        };

        match poll_outcome.status {
            ProviderTaskStatus::Pending => continue,
            ProviderTaskStatus::Running { progress } => {
                // First transition into Running: emit "云端开始生成" once.
                if !running_announced {
                    running_announced = true;
                    let _ = state::record_event(
                        &engine.db,
                        &engine.event_tx,
                        task_id,
                        EventPhase::Poll,
                        EventSeverity::Info,
                        "云端开始生成".to_string(),
                        EventBuilder::new()
                            .maybe_request_id(poll_outcome.request_id.clone())
                            .maybe_http_status(poll_outcome.http_status),
                    )
                    .await;
                }
                if let Some(p) = progress {
                    let _ = engine
                        .event_tx
                        .send(super::events::TaskEvent::progress_tick(task_id, p));
                }
                // Re-emit StatusChanged with updated progress; the DB row stays
                // in running state so transition_status would reject. We send
                // the event directly so the UI can update its progress hint.
                let _ = engine.event_tx.send(super::events::TaskEvent::status_changed(
                    task_id,
                    GenerationTaskStatus::Running,
                    progress,
                    None,
                ));
                continue;
            }
            ProviderTaskStatus::Success { result_url } => {
                let _ = state::record_event(
                    &engine.db,
                    &engine.event_tx,
                    task_id,
                    EventPhase::Poll,
                    EventSeverity::Info,
                    "生成完成".to_string(),
                    EventBuilder::new()
                        .maybe_request_id(poll_outcome.request_id.clone())
                        .maybe_http_status(poll_outcome.http_status)
                        .details(serde_json::json!({ "result_url": result_url })),
                )
                .await;

                // Materialize the result: download + persist as Asset.
                let task_for_mat = match state::load(&engine.db, task_id).await {
                    Ok(t) => t,
                    Err(e) => {
                        return fail_with_detail(
                            engine,
                            task_id,
                            EventPhase::Persist,
                            detail_from_error(&e),
                        )
                        .await
                    }
                };
                let mat_outcome = match engine
                    .materializer
                    .materialize(&task_for_mat, &result_url)
                    .await
                {
                    Ok(o) => o,
                    Err(e) => {
                        tracing::error!(task_id = %task_id, error = %e, "materialize failed");
                        let detail = detail_from_error(&e);
                        let phase = if matches!(e, CoreError::Provider(_)) {
                            EventPhase::Download
                        } else {
                            EventPhase::Persist
                        };
                        return fail_with_detail(engine, task_id, phase, detail).await;
                    }
                };

                if mat_outcome.bytes > 0 {
                    let mb = (mat_outcome.bytes as f64) / 1_048_576.0;
                    let secs = (mat_outcome.duration_ms as f64) / 1000.0;
                    let _ = state::record_event(
                        &engine.db,
                        &engine.event_tx,
                        task_id,
                        EventPhase::Download,
                        EventSeverity::Info,
                        format!("下载结果 ({mb:.2} MB, {secs:.1}s)"),
                        EventBuilder::new().details(serde_json::json!({
                            "bytes": mat_outcome.bytes,
                            "duration_ms": mat_outcome.duration_ms,
                        })),
                    )
                    .await;
                }

                // Write result_asset_id if materializer produced one.
                if !mat_outcome.asset_id.is_empty() {
                    if let Err(e) =
                        state::set_result_asset_id(&engine.db, task_id, &mat_outcome.asset_id).await
                    {
                        tracing::error!(task_id = %task_id, error = %e, "set_result_asset_id failed");
                    }
                    let _ = state::record_event(
                        &engine.db,
                        &engine.event_tx,
                        task_id,
                        EventPhase::Persist,
                        EventSeverity::Info,
                        "已存入素材库".to_string(),
                        EventBuilder::new()
                            .details(serde_json::json!({ "asset_id": mat_outcome.asset_id })),
                    )
                    .await;
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

                // Best-effort cleanup (e.g. remove uploaded reference images
                // from OSS). Cleanup runs in a detached task so terminal
                // status returns to the user immediately. The runner's event
                // channel is held by the engine for as long as it lives,
                // which outlives this task, so cleanup events still reach the
                // UI/CLI even after the task row has settled.
                let materializer = engine.materializer.clone();
                let task_clone = task_for_mat;
                let event_tx = engine.event_tx.clone();
                let db = engine.db.clone();
                let task_id_for_cleanup = task_id.to_string();
                tokio::spawn(async move {
                    match materializer.cleanup(&task_clone, Some(credentials)).await {
                        Ok(outcome) => {
                            for remote_id in &outcome.failed_remote_ids {
                                let _ = state::record_event(
                                    &db,
                                    &event_tx,
                                    &task_id_for_cleanup,
                                    EventPhase::Cleanup,
                                    EventSeverity::Warn,
                                    "临时文件清理失败".to_string(),
                                    EventBuilder::new()
                                        .details(serde_json::json!({ "remote_id": remote_id })),
                                )
                                .await;
                            }
                        }
                        Err(e) => {
                            tracing::warn!(task_id = %task_clone.id, error = %e, "materializer cleanup failed");
                            let _ = state::record_event(
                                &db,
                                &event_tx,
                                &task_id_for_cleanup,
                                EventPhase::Cleanup,
                                EventSeverity::Warn,
                                format!("清理失败: {e}"),
                                EventBuilder::new(),
                            )
                            .await;
                        }
                    }
                });
                return;
            }
            ProviderTaskStatus::Failed { message } => {
                let detail = ProviderErrorDetail::new(ProviderErrorKind::Unknown, message)
                    .with_request_id(poll_outcome.request_id.clone());
                let detail = if let Some(s) = poll_outcome.http_status {
                    detail.with_http_status(s as u16)
                } else {
                    detail
                };
                return fail_with_detail(engine, task_id, EventPhase::Poll, detail).await;
            }
        }
    }
}

/// Convert any `CoreError` into a `ProviderErrorDetail`. When the variant is
/// already structured (`Provider` / `Upload`) the inner detail is reused so we
/// don't lose `request_id` / `http_status`. Any other variant collapses to
/// `ProviderErrorKind::Unknown` with the error's `Display` as the message.
fn detail_from_error(e: &CoreError) -> ProviderErrorDetail {
    match e {
        CoreError::Provider(d) | CoreError::Upload(d) => d.clone(),
        other => ProviderErrorDetail::new(ProviderErrorKind::Unknown, other.to_string()),
    }
}

/// Records a structured error event then transitions the task to `Failed`.
/// Replaces the old `fail(message)` helper — every failure now carries a
/// `phase` for the diagnostics UI and a `ProviderErrorDetail` so the
/// `request_id` / `http_status` / `kind` make it into the event row.
async fn fail_with_detail(
    engine: &TaskEngineHandle,
    task_id: &str,
    phase: EventPhase,
    detail: ProviderErrorDetail,
) {
    let _ = state::record_event(
        &engine.db,
        &engine.event_tx,
        task_id,
        phase,
        EventSeverity::Error,
        detail.message.clone(),
        EventBuilder::from_detail(&detail),
    )
    .await;
    if let Err(e) = state::transition(
        &engine.db,
        &engine.event_tx,
        task_id,
        GenerationTaskStatus::Failed,
        None,
        Some(detail.to_string()),
    )
    .await
    {
        tracing::error!(task_id = %task_id, error = %e, "transition→failed failed");
    }
}

/// Merge `remote_ids` into the task's `params_json` under
/// `_internal.uploaded_remote_ids`. The materializer's cleanup path reads this
/// column verbatim, so the runner must persist them before reaching a
/// terminal state. The merge is additive: any existing `_internal` keys are
/// preserved, and existing `uploaded_remote_ids` are deduped against the new
/// list.
async fn persist_uploaded_remote_ids(
    engine: &TaskEngineHandle,
    task_id: &str,
    remote_ids: &[String],
) -> Result<()> {
    if remote_ids.is_empty() {
        return Ok(());
    }
    let id = task_id.to_string();
    let new_ids: Vec<String> = remote_ids.to_vec();
    engine
        .db
        .call(move |conn| {
            let task = match crate::db::queries::generation_task::get_by_id(conn, &id) {
                Ok(t) => t,
                Err(e) => return Ok(Err(e)),
            };
            let mut value: serde_json::Value =
                serde_json::from_str(&task.params_json).unwrap_or_else(|_| {
                    serde_json::Value::Object(serde_json::Map::new())
                });
            if !value.is_object() {
                value = serde_json::Value::Object(serde_json::Map::new());
            }
            let obj = value.as_object_mut().unwrap();
            let internal = obj
                .entry("_internal".to_string())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            if !internal.is_object() {
                *internal = serde_json::Value::Object(serde_json::Map::new());
            }
            let internal_obj = internal.as_object_mut().unwrap();
            let mut merged: Vec<String> = match internal_obj.get("uploaded_remote_ids") {
                Some(serde_json::Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
                _ => Vec::new(),
            };
            for rid in &new_ids {
                if !merged.contains(rid) {
                    merged.push(rid.clone());
                }
            }
            internal_obj.insert(
                "uploaded_remote_ids".to_string(),
                serde_json::Value::Array(
                    merged.into_iter().map(serde_json::Value::String).collect(),
                ),
            );
            let serialized = value.to_string();
            Ok(crate::db::queries::generation_task::set_params_json(
                conn,
                &id,
                &serialized,
            ))
        })
        .await
        .map_err(|e| match e {
            tokio_rusqlite::Error::Error(inner) => CoreError::Sqlite(inner),
            other => CoreError::TaskEngine(format!("db worker error: {other}")),
        })?
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

/// For models that upload local files (e.g. happyhorse reference images),
/// resolve asset_id references in provider_params["media"] to absolute local paths.
async fn resolve_media_paths(
    engine: &TaskEngineHandle,
    task: &GenerationTask,
    params: &mut GenerationParams,
) -> Result<()> {
    // Only process if media array exists in provider_params.
    let media = match params.provider_params.get_mut("media") {
        Some(serde_json::Value::Array(arr)) => arr,
        _ => return Ok(()),
    };

    if media.is_empty() {
        return Ok(());
    }

    let project_id = task.project_id.clone().ok_or_else(|| {
        CoreError::Validation("task with media references must have project_id".into())
    })?;

    // Collect asset_ids that need resolution (entries missing local_path).
    let asset_ids: Vec<String> = media
        .iter()
        .filter_map(|entry| {
            let has_local = entry
                .get("local_path")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty());
            if has_local {
                return None;
            }
            entry.get("asset_id").and_then(|v| v.as_str()).map(|s| s.to_string())
        })
        .collect();

    if asset_ids.is_empty() {
        return Ok(());
    }

    // Resolve project root and all asset file_paths in a single DB call.
    let ids = asset_ids.clone();
    let pid = project_id.clone();
    let workspace = engine.workspace_root.clone();
    let resolved: Vec<(String, String)> = engine
        .db
        .call(move |conn| {
            Ok(resolve_asset_paths_sync(conn, &workspace, &pid, &ids))
        })
        .await
        .map_err(|e: tokio_rusqlite::Error| CoreError::TaskEngine(format!("failed to resolve media paths: {e}")))??;

    // Verify files exist and inject local_path into each media entry.
    let path_map: std::collections::HashMap<String, String> = resolved.into_iter().collect();
    for entry in media.iter_mut() {
        if let Some(aid) = entry.get("asset_id").and_then(|v| v.as_str())
            && let Some(abs_path) = path_map.get(aid)
        {
            let p = Path::new(abs_path);
            if !p.exists() {
                return Err(CoreError::Validation(format!(
                    "reference image not found: {}",
                    p.display()
                )));
            }
            entry
                .as_object_mut()
                .unwrap()
                .insert("local_path".to_string(), serde_json::Value::String(abs_path.clone()));
        }
    }

    Ok(())
}

/// Synchronous helper for resolve_media_paths: runs inside db.call(). The
/// stored `project.root_path` is workspace-relative; we join it under
/// `workspace_root` before composing the per-asset absolute path.
fn resolve_asset_paths_sync(
    conn: &rusqlite::Connection,
    workspace_root: &Path,
    project_id: &str,
    asset_ids: &[String],
) -> Result<Vec<(String, String)>> {
    let project = project_q::get_by_id(conn, project_id)?;
    let project_root = crate::paths::resolve_project_root(workspace_root, &project.root_path)?;
    let mut results = Vec::with_capacity(asset_ids.len());
    for aid in asset_ids {
        let asset = asset_q::get_by_id(conn, aid)?;
        let abs = project_root.join(&asset.file_path);
        results.push((aid.clone(), abs.to_string_lossy().to_string()));
    }
    Ok(results)
}
