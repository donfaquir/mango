//! `ModelProvider` trait + value types it consumes/produces.
//!
//! The task engine talks to providers exclusively through this surface — no
//! provider-specific types leak into `task_engine`. Concrete provider impls
//! (e.g. spec-17 `BailianProvider`) wrap their HTTP client and translate
//! provider responses into [`ProviderTaskStatus`].

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::Result;

/// Submit-time parameters. `provider_params` carries provider-private JSON
/// (e.g. wan2.7 `size`, happyhorse `media[]`) that the engine decodes from
/// `generation_task.params_json`.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GenerationParams {
    pub model_id: String,
    pub prompt: String,
    pub provider_params: serde_json::Value,
    pub credentials: ProviderCredentials,
}

/// Plaintext credentials, materialized in memory only. The engine resolves
/// them from `keyring` per task and never persists them outside the keyring.
/// `extra_json` is reserved for spec-16 (OSS endpoint/bucket).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProviderCredentials {
    pub api_key: String,
    #[serde(default)]
    pub extra_json: Option<String>,
}

/// Provider-side status. The task engine maps this to
/// [`crate::models::generation_task::GenerationTaskStatus`].
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderTaskStatus {
    Pending,
    Running {
        #[specta(type = Option<specta_typescript::Number>)]
        progress: Option<u8>,
    },
    Success {
        result_url: String,
    },
    Failed {
        message: String,
    },
}

/// Telemetry from a successful upload group (e.g. happyhorse's reference
/// images going to OSS) — surfaced so the runner can write a single
/// `submit_upload` info event with aggregate stats.
#[derive(Debug, Clone, Default)]
pub struct UploadSummary {
    pub count: usize,
    pub total_bytes: u64,
    pub duration_ms: u64,
    /// Backend handles to be cleaned up when the task reaches a terminal
    /// state (passed through to the materializer via `_internal` in
    /// `params_json`).
    pub remote_ids: Vec<String>,
}

/// Result of a successful [`ModelProvider::submit`]. Carries the upstream
/// `request_id`/`http_status` so the runner can attribute events to the
/// correct DashScope call.
#[derive(Debug, Clone, Default)]
pub struct SubmitOutcome {
    pub external_task_id: String,
    pub request_id: Option<String>,
    pub http_status: Option<i64>,
    pub upload: Option<UploadSummary>,
}

impl SubmitOutcome {
    pub fn new(external_task_id: impl Into<String>) -> Self {
        Self {
            external_task_id: external_task_id.into(),
            request_id: None,
            http_status: None,
            upload: None,
        }
    }
}

/// Result of a [`ModelProvider::poll`]. Carries optional upstream metadata so
/// transient `poll` warn events (e.g. 429) can record the request_id.
#[derive(Debug, Clone)]
pub struct PollOutcome {
    pub status: ProviderTaskStatus,
    pub request_id: Option<String>,
    pub http_status: Option<i64>,
}

impl PollOutcome {
    pub fn bare(status: ProviderTaskStatus) -> Self {
        Self {
            status,
            request_id: None,
            http_status: None,
        }
    }
}

#[async_trait]
pub trait ModelProvider: Send + Sync + 'static {
    /// Kick off a job. For synchronous models, providers should still return
    /// an external id (a random uuid is fine) so the engine has a stable
    /// handle for `poll`/`cancel`/`download`. Returns a [`SubmitOutcome`]
    /// carrying request_id/http_status/upload metadata so the runner can
    /// emit structured `submit_upload` and `submit_call` events.
    async fn submit(&self, params: GenerationParams) -> Result<SubmitOutcome>;

    /// Best-effort status query. Sync providers may return `Success` on the
    /// first poll if they cached the response from `submit`.
    async fn poll(&self, external_task_id: &str) -> Result<PollOutcome>;

    /// Best-effort cancel. Providers without server-side cancel return Ok(()).
    async fn cancel(&self, external_task_id: &str) -> Result<()>;

    /// Download the result to `dest`. Returns the actual path written
    /// (provider may append an extension). The engine decides `dest`; this
    /// trait only writes bytes.
    async fn download(&self, external_task_id: &str, dest: &Path) -> Result<PathBuf>;
}
