//! BailianProvider — DashScope-backed provider covering three models:
//! - `wan2.7-image-pro`: synchronous text-to-image (chat-messages style)
//! - `happyhorse-1.0-r2v`: asynchronous reference-image-to-video
//! - `cosyvoice-v3-flash`: non-realtime HTTP text-to-speech (CosyVoice TTS)
//!
//! Routes by `model_id` in submit/poll/cancel/download.

pub(crate) mod client;
pub(crate) mod cosyvoice;
pub(crate) mod happyhorse;
pub mod materializer;
pub(crate) mod types;
pub(crate) mod validate;
pub(crate) mod wan27;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::error::{CoreError, Result};
use crate::provider::asset_uploader::oss::OssUploader;
use crate::provider::error::{ProviderErrorDetail, ProviderErrorKind};
use crate::provider::traits::{
    GenerationParams, ModelProvider, PollOutcome, ProviderCredentials, ProviderTaskStatus,
    SubmitOutcome, UploadSummary,
};
use client::BailianClient;
use cosyvoice::TtsCache;
use wan27::Wan27Cache;

/// One BailianProvider instance covers all DashScope models. Credentials are
/// injected per-call via `GenerationParams::credentials` — the struct holds no secrets.
pub struct BailianProvider {
    /// HTTP client shared across calls. Timeout = 120s (wan27 sync can take ~60s).
    http: reqwest::Client,
    /// In-memory cache for wan27 synchronous results. Keyed by "wan27:{uuid}".
    wan_cache: Arc<Mutex<Wan27Cache>>,
    /// In-memory cache for cosyvoice TTS synchronous results. Keyed by "cosyvoice:{uuid}".
    tts_cache: Arc<Mutex<TtsCache>>,
    /// Credentials cache for happyhorse poll/cancel: external_task_id → api_key.
    /// Populated at submit time, cleared when task reaches terminal state.
    creds_cache: Arc<Mutex<HashMap<String, String>>>,
}

impl BailianProvider {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .gzip(true)
            .build()
            .expect("reqwest builder default config never fails");
        Self {
            http,
            wan_cache: Arc::new(Mutex::new(Wan27Cache::default())),
            tts_cache: Arc::new(Mutex::new(TtsCache::default())),
            creds_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for BailianProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ModelProvider for BailianProvider {
    async fn submit(&self, params: GenerationParams) -> Result<SubmitOutcome> {
        validate::validate_params(&params)?;
        let client = BailianClient::new(&self.http, &params.credentials)?;
        match params.model_id.as_str() {
            "wan2.7-image-pro" => {
                let result = wan27::submit(&client, &self.wan_cache, &params).await?;
                Ok(SubmitOutcome {
                    external_task_id: result.external_task_id,
                    request_id: result.request_id,
                    http_status: Some(result.http_status as i64),
                    upload: None,
                })
            }
            "cosyvoice-v3-flash" => {
                let result = cosyvoice::submit(&client, &self.tts_cache, &params).await?;
                Ok(SubmitOutcome {
                    external_task_id: result.external_task_id,
                    request_id: result.request_id,
                    http_status: Some(result.http_status as i64),
                    upload: None,
                })
            }
            "happyhorse-1.0-r2v" => {
                let uploader = build_uploader(&params)?;
                let result = happyhorse::submit(&client, &params, uploader).await?;
                // Cache the api_key for subsequent poll/cancel calls.
                self.creds_cache.lock().await.insert(
                    result.external_task_id.clone(),
                    params.credentials.api_key.clone(),
                );
                Ok(SubmitOutcome {
                    external_task_id: result.external_task_id,
                    request_id: result.request_id,
                    http_status: Some(result.http_status as i64),
                    upload: Some(UploadSummary {
                        count: result.upload_count,
                        total_bytes: result.upload_total_bytes,
                        duration_ms: result.upload_duration_ms,
                        remote_ids: result.uploaded_remote_ids,
                    }),
                })
            }
            other => Err(CoreError::Provider(ProviderErrorDetail::new(
                ProviderErrorKind::InvalidRequest,
                format!("不支持的百炼模型: {other}"),
            ))),
        }
    }

    async fn poll(&self, external_task_id: &str) -> Result<PollOutcome> {
        if external_task_id.starts_with("wan27:") {
            let status = wan27::poll(&self.wan_cache, external_task_id).await?;
            Ok(PollOutcome::bare(status))
        } else if external_task_id.starts_with("cosyvoice:") {
            let status = cosyvoice::poll(&self.tts_cache, external_task_id).await?;
            Ok(PollOutcome::bare(status))
        } else {
            // happyhorse — retrieve cached api_key.
            let api_key = self
                .creds_cache
                .lock()
                .await
                .get(external_task_id)
                .cloned()
                .ok_or_else(|| {
                    CoreError::TaskEngine(format!(
                        "no cached credentials for happyhorse task {external_task_id}"
                    ))
                })?;
            let creds = ProviderCredentials {
                api_key,
                extra_json: None,
            };
            let client = BailianClient::new(&self.http, &creds)?;
            let (status, meta) = happyhorse::poll(&client, external_task_id).await?;
            // Clean up creds cache when task reaches terminal state.
            if matches!(
                status,
                ProviderTaskStatus::Success { .. } | ProviderTaskStatus::Failed { .. }
            ) {
                self.creds_cache.lock().await.remove(external_task_id);
            }
            Ok(PollOutcome {
                status,
                request_id: meta.request_id,
                http_status: Some(meta.http_status as i64),
            })
        }
    }

    async fn cancel(&self, external_task_id: &str) -> Result<()> {
        if external_task_id.starts_with("wan27:") {
            wan27::cancel(&self.wan_cache, external_task_id).await;
            Ok(())
        } else if external_task_id.starts_with("cosyvoice:") {
            cosyvoice::cancel(&self.tts_cache, external_task_id).await;
            Ok(())
        } else {
            // Best-effort cancel for happyhorse.
            if let Some(api_key) = self.creds_cache.lock().await.get(external_task_id).cloned() {
                let creds = ProviderCredentials {
                    api_key,
                    extra_json: None,
                };
                if let Ok(client) = BailianClient::new(&self.http, &creds) {
                    let _ = happyhorse::cancel(&client, external_task_id).await;
                }
            }
            // Remove from creds cache.
            self.creds_cache.lock().await.remove(external_task_id);
            Ok(())
        }
    }

    async fn download(&self, _external_task_id: &str, dest: &Path) -> Result<PathBuf> {
        // Download is now handled by ResultMaterializer, not by the provider.
        // This method exists for trait completeness. Return dest as-is.
        Ok(dest.to_path_buf())
    }
}

fn build_uploader(params: &GenerationParams) -> Result<Arc<dyn crate::provider::asset_uploader::traits::AssetUploader>> {
    let extra = params.credentials.extra_json.as_deref().ok_or_else(|| {
        CoreError::Validation(
            "happyhorse requires OSS credentials configured on the account".into(),
        )
    })?;
    let uploader = OssUploader::from_credentials_json(extra)?;
    Ok(Arc::new(uploader))
}
