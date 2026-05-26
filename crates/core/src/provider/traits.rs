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

#[async_trait]
pub trait ModelProvider: Send + Sync + 'static {
    /// Kick off a job. For synchronous models, providers should still return
    /// an external id (a random uuid is fine) so the engine has a stable
    /// handle for `poll`/`cancel`/`download`.
    async fn submit(&self, params: GenerationParams) -> Result<String>;

    /// Best-effort status query. Sync providers may return `Success` on the
    /// first poll if they cached the response from `submit`.
    async fn poll(&self, external_task_id: &str) -> Result<ProviderTaskStatus>;

    /// Best-effort cancel. Providers without server-side cancel return Ok(()).
    async fn cancel(&self, external_task_id: &str) -> Result<()>;

    /// Download the result to `dest`. Returns the actual path written
    /// (provider may append an extension). The engine decides `dest`; this
    /// trait only writes bytes.
    async fn download(&self, external_task_id: &str, dest: &Path) -> Result<PathBuf>;
}
