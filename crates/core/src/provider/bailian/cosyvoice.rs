//! CosyVoice non-realtime HTTP TTS model wrapper.
//!
//! Same pattern as wan27: the DashScope TTS HTTP API returns a JSON response
//! with an audio URL. We cache the result in memory so the engine's uniform
//! state machine works without branching.
//!
//! API endpoint: POST /services/audio/tts/SpeechSynthesizer
//! Doc: https://help.aliyun.com/zh/model-studio/cosyvoice-tts-http-api

use std::collections::HashMap;

use tokio::sync::Mutex;
use uuid::Uuid;

use super::client::BailianClient;
use super::types::CosyVoiceResponse;
use crate::error::{CoreError, Result};
use crate::provider::error::{ProviderErrorDetail, ProviderErrorKind};
use crate::provider::traits::{GenerationParams, ProviderTaskStatus};

/// In-memory cache for CosyVoice synchronous results.
#[derive(Default)]
pub(super) struct TtsCache {
    entries: HashMap<String, ProviderTaskStatus>,
}

impl TtsCache {
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

pub(super) struct SubmitResult {
    pub external_task_id: String,
    pub request_id: Option<String>,
    pub http_status: u16,
}

/// Submit a CosyVoice TTS request. Synchronous call — waits for DashScope to
/// return the audio URL, caches it, and returns a synthetic task ID.
pub(super) async fn submit(
    client: &BailianClient<'_>,
    cache: &Mutex<TtsCache>,
    params: &GenerationParams,
) -> Result<SubmitResult> {
    let body = build_cosyvoice_body(params)?;

    let (resp, meta): (CosyVoiceResponse, _) = client
        .post_json("/services/audio/tts/SpeechSynthesizer", &body)
        .await?;

    let result_url = resp
        .output
        .audio
        .and_then(|a| a.url)
        .ok_or_else(|| {
            CoreError::Provider(
                ProviderErrorDetail::new(
                    ProviderErrorKind::Malformed,
                    "cosyvoice 响应缺少 output.audio.url 字段",
                )
                .with_request_id(meta.request_id.clone())
                .with_http_status(meta.http_status),
            )
        })?;

    let task_id = match meta.request_id.as_deref() {
        Some(rid) if !rid.is_empty() => format!("cosyvoice:{rid}"),
        _ => format!("cosyvoice:{}", Uuid::new_v4()),
    };
    cache.lock().await.put(
        task_id.clone(),
        ProviderTaskStatus::Success { result_url },
    );
    Ok(SubmitResult {
        external_task_id: task_id,
        request_id: meta.request_id,
        http_status: meta.http_status,
    })
}

/// Poll the TTS cache. If the task exists, return its cached status.
pub(super) async fn poll(cache: &Mutex<TtsCache>, full_task_id: &str) -> Result<ProviderTaskStatus> {
    cache
        .lock()
        .await
        .get(full_task_id)
        .ok_or_else(|| {
            CoreError::TaskEngine(format!(
                "cosyvoice cache miss for {full_task_id}; was the task already cleared?"
            ))
        })
}

/// Cancel a CosyVoice task — just removes the cache entry.
pub(super) async fn cancel(cache: &Mutex<TtsCache>, full_task_id: &str) {
    cache.lock().await.remove(full_task_id);
}

fn build_cosyvoice_body(params: &GenerationParams) -> Result<serde_json::Value> {
    let text = params
        .provider_params
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or(&params.prompt);

    let voice_id = params
        .provider_params
        .get("voice_id")
        .and_then(|v| v.as_str())
        .unwrap_or("longanyang");

    let format = params
        .provider_params
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("mp3");

    let sample_rate = params
        .provider_params
        .get("sample_rate")
        .and_then(|v| v.as_u64())
        .unwrap_or(24000);

    let rate = params
        .provider_params
        .get("rate")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);

    let volume = params
        .provider_params
        .get("volume")
        .and_then(|v| v.as_i64())
        .unwrap_or(50);

    let pitch = params
        .provider_params
        .get("pitch")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);

    // CosyVoice HTTP API: all params go inside "input", no separate "parameters"
    Ok(serde_json::json!({
        "model": params.model_id,
        "input": {
            "text": text,
            "voice": voice_id,
            "format": format,
            "sample_rate": sample_rate,
            "rate": rate,
            "volume": volume,
            "pitch": pitch
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_cache() -> Arc<Mutex<TtsCache>> {
        Arc::new(Mutex::new(TtsCache::default()))
    }

    #[test]
    fn build_body_correct_structure() {
        let params = GenerationParams {
            model_id: "cosyvoice-v3-flash".to_string(),
            prompt: "hello world".to_string(),
            provider_params: serde_json::json!({
                "voice_id": "longshu",
                "rate": 1.2,
                "volume": 80,
                "pitch": 10,
                "format": "wav",
                "sample_rate": 16000
            }),
            credentials: crate::provider::traits::ProviderCredentials {
                api_key: "k".into(),
                extra_json: None,
            },
        };
        let body = build_cosyvoice_body(&params).unwrap();
        assert_eq!(body["model"], "cosyvoice-v3-flash");
        assert_eq!(body["input"]["text"], "hello world");
        assert_eq!(body["input"]["voice"], "longshu");
        assert_eq!(body["input"]["rate"], 1.2);
        assert_eq!(body["input"]["volume"], 80);
        assert_eq!(body["input"]["format"], "wav");
        assert_eq!(body["input"]["sample_rate"], 16000);
    }

    #[test]
    fn build_body_uses_text_from_provider_params() {
        let params = GenerationParams {
            model_id: "cosyvoice-v3-flash".to_string(),
            prompt: "fallback".to_string(),
            provider_params: serde_json::json!({
                "text": "primary text",
                "voice_id": "longxiaochun"
            }),
            credentials: crate::provider::traits::ProviderCredentials {
                api_key: "k".into(),
                extra_json: None,
            },
        };
        let body = build_cosyvoice_body(&params).unwrap();
        assert_eq!(body["input"]["text"], "primary text");
    }

    #[test]
    fn build_body_falls_back_to_prompt() {
        let params = GenerationParams {
            model_id: "cosyvoice-v3-flash".to_string(),
            prompt: "fallback prompt".to_string(),
            provider_params: serde_json::json!({ "voice_id": "longxiaochun" }),
            credentials: crate::provider::traits::ProviderCredentials {
                api_key: "k".into(),
                extra_json: None,
            },
        };
        let body = build_cosyvoice_body(&params).unwrap();
        assert_eq!(body["input"]["text"], "fallback prompt");
    }

    #[tokio::test]
    async fn poll_returns_cached_success() {
        let cache = make_cache();
        cache.lock().await.put(
            "cosyvoice:abc123".to_string(),
            ProviderTaskStatus::Success {
                result_url: "https://example.com/audio.mp3".to_string(),
            },
        );
        let status = poll(&cache, "cosyvoice:abc123").await.unwrap();
        assert!(matches!(status, ProviderTaskStatus::Success { .. }));
    }

    #[tokio::test]
    async fn poll_missing_returns_error() {
        let cache = make_cache();
        let result = poll(&cache, "cosyvoice:nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn cancel_clears_cache() {
        let cache = make_cache();
        cache.lock().await.put(
            "cosyvoice:abc".to_string(),
            ProviderTaskStatus::Success {
                result_url: "url".to_string(),
            },
        );
        cancel(&cache, "cosyvoice:abc").await;
        assert!(cache.lock().await.get("cosyvoice:abc").is_none());
    }
}
