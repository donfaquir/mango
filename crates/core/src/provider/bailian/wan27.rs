//! wan2.7-image-pro synchronous model wrapper.
//!
//! The DashScope API for this model returns the result directly in the response
//! (no task_id polling). We wrap it to look async by caching the result in
//! memory: `submit` → call API → cache result → return a `wan27:{uuid}` id.
//! `poll` → read from cache → return `Success`. This lets the engine's uniform
//! state machine work without branching.

use std::collections::HashMap;

use tokio::sync::Mutex;
use uuid::Uuid;

use super::client::BailianClient;
use super::types::Wan27Response;
use crate::error::{CoreError, Result};
use crate::provider::traits::{GenerationParams, ProviderTaskStatus};

/// In-memory cache for wan27 synchronous results.
#[derive(Default)]
pub(super) struct Wan27Cache {
    entries: HashMap<String, ProviderTaskStatus>,
}

impl Wan27Cache {
    fn put(&mut self, task_id: String, status: ProviderTaskStatus) {
        self.entries.insert(task_id, status);
    }
    fn get(&self, task_id: &str) -> Option<ProviderTaskStatus> {
        self.entries.get(task_id).cloned()
    }
    pub(super) fn remove(&mut self, task_id: &str) {
        self.entries.remove(task_id);
    }
}

/// Submit a wan2.7-image-pro request. This is a blocking call that actually
/// waits for the DashScope API to return the generated image URL, then caches
/// it and returns a synthetic task ID.
pub(super) async fn submit(
    client: &BailianClient<'_>,
    cache: &Mutex<Wan27Cache>,
    params: &GenerationParams,
) -> Result<String> {
    let body = build_wan27_body(params)?;

    let resp: Wan27Response = client
        .post_json("/services/aigc/multimodal-generation/generation", &body)
        .await?;

    let result_url = resp
        .output
        .choices
        .first()
        .and_then(|c| c.message.content.first())
        .and_then(|c| c.image.as_deref())
        .ok_or_else(|| {
            CoreError::Provider(
                "wan27 response missing output.choices[0].message.content[0].image".into(),
            )
        })?
        .to_string();

    let task_id = format!("wan27:{}", Uuid::new_v4());
    cache.lock().await.put(
        task_id.clone(),
        ProviderTaskStatus::Success { result_url },
    );
    Ok(task_id)
}

/// Poll the wan27 cache. If the task exists, return its cached status.
pub(super) async fn poll(cache: &Mutex<Wan27Cache>, full_task_id: &str) -> Result<ProviderTaskStatus> {
    cache
        .lock()
        .await
        .get(full_task_id)
        .ok_or_else(|| {
            CoreError::TaskEngine(format!(
                "wan27 cache miss for {full_task_id}; was the task already cleared?"
            ))
        })
}

/// Cancel a wan27 task — just removes the cache entry.
pub(super) async fn cancel(cache: &Mutex<Wan27Cache>, full_task_id: &str) {
    cache.lock().await.remove(full_task_id);
}

fn build_wan27_body(params: &GenerationParams) -> Result<serde_json::Value> {
    let n = params
        .provider_params
        .get("n")
        .and_then(|v| v.as_u64())
        .unwrap_or(1);
    let size = params
        .provider_params
        .get("size")
        .and_then(|v| v.as_str())
        .unwrap_or("2K");
    let enable_sequential = params
        .provider_params
        .get("enable_sequential")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let negative_prompt = params
        .provider_params
        .get("negative_prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    Ok(serde_json::json!({
        "model": "wan2.7-image-pro",
        "input": {
            "messages": [{
                "role": "user",
                "content": [{ "text": params.prompt }]
            }]
        },
        "parameters": {
            "n": n,
            "size": size,
            "enable_sequential": enable_sequential,
            "negative_prompt": negative_prompt
        }
    }))
}
