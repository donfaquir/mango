//! happyhorse-1.0-r2v asynchronous model (reference image → video).
//!
//! Submit: upload reference images to OSS → POST DashScope with
//! `X-DashScope-Async: enable` → return DashScope's task_id.
//! Poll: GET /tasks/{id} → map `task_status` to [`ProviderTaskStatus`].
//! Cancel: DELETE /tasks/{id} (best-effort).

use std::sync::Arc;

use super::client::BailianClient;
use super::types::{AsyncPollResponse, AsyncSubmitResponse, MediaEntry};
use crate::error::{CoreError, Result};
use crate::provider::asset_uploader::traits::{AssetUploader, UploadedAsset};
use crate::provider::traits::{GenerationParams, ProviderTaskStatus};

/// Submit a happyhorse task. Before calling DashScope, uploads all reference
/// images to OSS via the provided `uploader`.
///
/// Returns the DashScope task_id (used directly as external_task_id).
pub(super) async fn submit(
    client: &BailianClient<'_>,
    params: &GenerationParams,
    uploader: Arc<dyn AssetUploader>,
) -> Result<SubmitResult> {
    // 1) Parse media[] from provider_params.
    let media_entries: Vec<MediaEntry> = serde_json::from_value(
        params
            .provider_params
            .get("media")
            .cloned()
            .ok_or_else(|| {
                CoreError::Validation("happyhorse params missing 'media[]'".into())
            })?,
    )
    .map_err(|e| CoreError::Validation(format!("media[] schema: {e}")))?;

    if media_entries.is_empty() {
        return Err(CoreError::Validation(
            "happyhorse requires at least one reference image".into(),
        ));
    }

    // 2) Upload all reference images in parallel.
    let upload_started = std::time::Instant::now();
    let upload_futures = media_entries.iter().map(|m| {
        let path = std::path::PathBuf::from(&m.local_path);
        let up = uploader.clone();
        async move { up.upload(&path).await }
    });
    let uploads: Vec<UploadedAsset> = futures::future::try_join_all(upload_futures).await?;
    let upload_duration_ms = upload_started.elapsed().as_millis() as u64;

    // 3) Build DashScope request body.
    let body = build_happyhorse_body(params, &uploads)?;

    // 4) POST with async header.
    let (task_resp, meta): (AsyncSubmitResponse, _) = match client
        .post_json_async("/services/aigc/video-generation/video-synthesis", &body)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            // If submit fails, best-effort cleanup already-uploaded files.
            for u in &uploads {
                let _ = uploader.cleanup(&u.remote_id).await;
            }
            return Err(e);
        }
    };

    // 5) Collect remote_ids and aggregate byte counts for the runner's
    //    `submit_upload` event.
    let remote_ids: Vec<String> = uploads.iter().map(|u| u.remote_id.clone()).collect();
    let upload_total_bytes: u64 = uploads.iter().map(|u| u.bytes).sum();

    Ok(SubmitResult {
        external_task_id: task_resp.output.task_id,
        uploaded_remote_ids: remote_ids,
        upload_count: uploads.len(),
        upload_total_bytes,
        upload_duration_ms,
        request_id: meta.request_id,
        http_status: meta.http_status,
    })
}

/// Result from happyhorse submit — caller persists `uploaded_remote_ids` to DB
/// and writes `submit_upload`/`submit_call` events using the timing + meta.
pub(super) struct SubmitResult {
    pub external_task_id: String,
    pub uploaded_remote_ids: Vec<String>,
    pub upload_count: usize,
    pub upload_total_bytes: u64,
    pub upload_duration_ms: u64,
    pub request_id: Option<String>,
    pub http_status: u16,
}

/// Poll DashScope for task status. The returned tuple's second element carries
/// the upstream `request_id`/`http_status` so the runner can attribute warn
/// events (e.g. transient 429 during poll) to the correct upstream call.
pub(super) async fn poll(
    client: &BailianClient<'_>,
    external_task_id: &str,
) -> Result<(ProviderTaskStatus, super::client::ResponseMeta)> {
    let (resp, meta): (AsyncPollResponse, _) = client
        .get_json(&format!("/tasks/{external_task_id}"))
        .await?;

    let status = match resp.output.task_status.as_str() {
        "PENDING" | "QUEUED" => ProviderTaskStatus::Pending,
        "RUNNING" => ProviderTaskStatus::Running { progress: None },
        "SUCCEEDED" => ProviderTaskStatus::Success {
            result_url: resp.output.video_url.unwrap_or_default(),
        },
        "FAILED" | "UNKNOWN" => ProviderTaskStatus::Failed {
            message: resp
                .output
                .message
                .unwrap_or_else(|| "task failed".into()),
        },
        other => ProviderTaskStatus::Failed {
            message: format!("unknown DashScope task_status '{other}'"),
        },
    };
    Ok((status, meta))
}

/// Best-effort cancel via DashScope DELETE.
pub(super) async fn cancel(client: &BailianClient<'_>, external_task_id: &str) -> Result<()> {
    let _ = client
        .delete(&format!("/tasks/{external_task_id}"))
        .await;
    Ok(())
}

fn build_happyhorse_body(
    params: &GenerationParams,
    uploads: &[UploadedAsset],
) -> Result<serde_json::Value> {
    let resolution = params
        .provider_params
        .get("resolution")
        .and_then(|v| v.as_str())
        .unwrap_or("720P");
    let ratio = params
        .provider_params
        .get("ratio")
        .and_then(|v| v.as_str())
        .unwrap_or("16:9");
    let duration = params
        .provider_params
        .get("duration")
        .and_then(|v| v.as_u64())
        .unwrap_or(5);

    Ok(serde_json::json!({
        "model": "happyhorse-1.0-r2v",
        "input": {
            "prompt": params.prompt,
            "media": uploads.iter().map(|u| serde_json::json!({
                "type": "reference_image",
                "url": u.url,
            })).collect::<Vec<_>>()
        },
        "parameters": {
            "resolution": resolution,
            "ratio": ratio,
            "duration": duration,
        }
    }))
}
