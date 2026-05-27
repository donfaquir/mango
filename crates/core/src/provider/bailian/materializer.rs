//! `BailianResultMaterializer` — downloads the generated result from DashScope
//! and persists it as a local [`Asset`], then cleans up any temporary OSS
//! objects that were uploaded as reference images.

use std::sync::Arc;

use async_trait::async_trait;

use crate::account::keyring::KeyringStore;
use crate::asset::from_remote::download_to_asset;
use crate::error::{CoreError, Result};
use crate::models::generation_task::{GenerationTask, TaskKind};
use crate::provider::asset_uploader::oss::OssUploader;
use crate::provider::asset_uploader::traits::AssetUploader;
use crate::provider::error::{ProviderErrorDetail, ProviderErrorKind};
use crate::provider::traits::ProviderCredentials;
use crate::task_engine::materializer::{CleanupOutcome, MaterializeOutcome, ResultMaterializer};

pub struct BailianResultMaterializer {
    db: tokio_rusqlite::Connection,
    keyring: Arc<dyn KeyringStore>,
}

impl BailianResultMaterializer {
    pub fn new(db: tokio_rusqlite::Connection, keyring: Arc<dyn KeyringStore>) -> Self {
        Self { db, keyring }
    }
}

#[async_trait]
impl ResultMaterializer for BailianResultMaterializer {
    async fn materialize(
        &self,
        task: &GenerationTask,
        result_url: &str,
    ) -> Result<MaterializeOutcome> {
        // Resolve project_id by traversing shot → episode → project.
        let project_id = resolve_project_id(&self.db, task).await?;

        let asset_type = match task.task_type {
            TaskKind::Image => "image",
            TaskKind::Video => "video",
            TaskKind::Audio => "audio",
            TaskKind::Text => {
                return Err(CoreError::Provider(ProviderErrorDetail::new(
                    ProviderErrorKind::InvalidRequest,
                    "百炼: 文本任务没有可下载的结果",
                )))
            }
        };

        let outcome = download_to_asset(
            &self.db,
            &project_id,
            task.shot_id.as_deref(),
            result_url,
            asset_type,
        )
        .await?;

        Ok(MaterializeOutcome {
            asset_id: outcome.asset.id,
            bytes: outcome.bytes,
            duration_ms: outcome.duration_ms,
        })
    }

    async fn cleanup(
        &self,
        task: &GenerationTask,
        credentials: Option<ProviderCredentials>,
    ) -> Result<CleanupOutcome> {
        // Parse _internal.uploaded_remote_ids from params_json.
        let params: serde_json::Value = match serde_json::from_str(&task.params_json) {
            Ok(v) => v,
            Err(_) => return Ok(CleanupOutcome::default()),
        };
        let remote_ids = match params
            .get("_internal")
            .and_then(|i| i.get("uploaded_remote_ids"))
            .and_then(|v| v.as_array())
        {
            Some(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>(),
            None => return Ok(CleanupOutcome::default()),
        };

        if remote_ids.is_empty() {
            return Ok(CleanupOutcome::default());
        }

        // Prefer pre-resolved credentials to avoid redundant keyring access.
        let extra = if let Some(creds) = credentials {
            match creds.extra_json {
                Some(json) => json,
                None => {
                    tracing::warn!("cleanup: no OSS config in pre-resolved credentials; skipping");
                    return Ok(CleanupOutcome {
                        failed_remote_ids: remote_ids,
                    });
                }
            }
        } else {
            // Fallback: resolve from keyring (backward-compat path).
            let account_id = task.account_id.clone();
            let keyring = self.keyring.clone();
            let creds_json: Option<String> = self
                .db
                .call(move |conn| {
                    Ok(crate::account::service::resolve_credentials(
                        conn,
                        keyring.as_ref(),
                        &account_id,
                    )
                    .map(|c| c.extra_json))
                })
                .await
                .map_err(|e: tokio_rusqlite::Error<rusqlite::Error>| {
                    CoreError::Provider(ProviderErrorDetail::new(
                        ProviderErrorKind::Unknown,
                        format!("清理时读取凭据失败: {e}"),
                    ))
                })?
                .map_err(|e| {
                    CoreError::Provider(ProviderErrorDetail::new(
                        ProviderErrorKind::Unknown,
                        format!("清理时解析凭据失败: {e}"),
                    ))
                })?;

            match creds_json {
                Some(json) => json,
                None => {
                    tracing::warn!("cleanup: no OSS config for account; skipping");
                    return Ok(CleanupOutcome {
                        failed_remote_ids: remote_ids,
                    });
                }
            }
        };

        let uploader = match OssUploader::from_credentials_json(&extra) {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!("cleanup: failed to build OssUploader: {e}");
                return Ok(CleanupOutcome {
                    failed_remote_ids: remote_ids,
                });
            }
        };

        let mut failed = Vec::new();
        for remote_id in &remote_ids {
            if let Err(e) = uploader.cleanup(remote_id).await {
                tracing::warn!(remote_id = %remote_id, error = %e, "oss cleanup failed");
                failed.push(remote_id.clone());
            }
        }

        Ok(CleanupOutcome {
            failed_remote_ids: failed,
        })
    }
}

/// Resolve project_id from task.
///
/// Strategy (in order):
/// 1. If task has project_id directly, use it (fast path for standalone tasks).
/// 2. If task has shot_id, traverse shot → episode → project (for shot-bound tasks).
/// 3. If neither, return Validation error.
async fn resolve_project_id(
    db: &tokio_rusqlite::Connection,
    task: &GenerationTask,
) -> Result<String> {
    // Fast path: task carries project_id directly (standalone generation tasks).
    if let Some(pid) = &task.project_id {
        return Ok(pid.clone());
    }

    // Fallback: traverse shot → episode → project (shot-bound tasks).
    let shot_id = task.shot_id.clone().ok_or_else(|| {
        CoreError::Validation(
            "task must have either project_id or shot_id to persist its result".into(),
        )
    })?;

    db.call(move |conn| {
        Ok(conn.query_row(
            "SELECT e.project_id FROM shot s \
             JOIN episode e ON s.episode_id = e.id \
             WHERE s.id = ?1",
            rusqlite::params![shot_id],
            |r| r.get::<_, String>(0),
        ))
    })
    .await
    .map_err(|e: tokio_rusqlite::Error<rusqlite::Error>| CoreError::TaskEngine(format!("db worker error: {e}")))?
    .map_err(CoreError::Sqlite)
}
