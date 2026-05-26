//! Test-only `ModelProvider` impl. Drives the engine through its full
//! state machine without an external API. Real providers live in spec-17.

#![cfg(test)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use super::traits::{GenerationParams, ModelProvider, ProviderTaskStatus};
use crate::error::Result;

/// Polls 3 times: Running(30%) → Running(60%) → Success.
#[derive(Default)]
pub struct StubProvider {
    polls: Arc<AtomicU8>,
}

#[async_trait]
impl ModelProvider for StubProvider {
    async fn submit(&self, _params: GenerationParams) -> Result<String> {
        Ok(format!("stub-{}", uuid::Uuid::new_v4()))
    }

    async fn poll(&self, _external_task_id: &str) -> Result<ProviderTaskStatus> {
        let n = self.polls.fetch_add(1, Ordering::SeqCst);
        if n < 2 {
            Ok(ProviderTaskStatus::Running {
                progress: Some((n + 1) * 30),
            })
        } else {
            Ok(ProviderTaskStatus::Success {
                result_url: "stub://ok".into(),
            })
        }
    }

    async fn cancel(&self, _external_task_id: &str) -> Result<()> {
        Ok(())
    }

    async fn download(&self, _external_task_id: &str, dest: &Path) -> Result<PathBuf> {
        // 1×1 transparent PNG, 67 bytes.
        const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\x0dIDATx\x9cc\xf8\xff\xff?\x00\x05\xfe\x02\xfe\xdcMb\xa3\x00\x00\x00\x00IEND\xaeB`\x82";
        std::fs::write(dest, PNG)?;
        Ok(dest.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn fake_params() -> GenerationParams {
        GenerationParams {
            model_id: "m".into(),
            prompt: "p".into(),
            provider_params: serde_json::json!({}),
            credentials: super::super::traits::ProviderCredentials {
                api_key: "k".into(),
                extra_json: None,
            },
        }
    }

    #[tokio::test]
    async fn poll_sequence_is_running_running_success() {
        let p = StubProvider::default();
        let _ = p.submit(fake_params()).await.unwrap();

        match p.poll("x").await.unwrap() {
            ProviderTaskStatus::Running { progress } => assert_eq!(progress, Some(30)),
            other => panic!("expected Running(30), got {other:?}"),
        }
        match p.poll("x").await.unwrap() {
            ProviderTaskStatus::Running { progress } => assert_eq!(progress, Some(60)),
            other => panic!("expected Running(60), got {other:?}"),
        }
        match p.poll("x").await.unwrap() {
            ProviderTaskStatus::Success { result_url } => assert_eq!(result_url, "stub://ok"),
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn download_writes_nonempty_file() {
        let p = StubProvider::default();
        let dir = tempdir().unwrap();
        let dest = dir.path().join("out.png");
        let written = p.download("x", &dest).await.unwrap();
        assert_eq!(written, dest);
        let bytes = std::fs::read(&dest).unwrap();
        assert!(!bytes.is_empty());
    }
}
