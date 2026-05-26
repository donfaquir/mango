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
use crate::task_engine::materializer::ResultMaterializer;

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
    async fn materialize(&self, task: &GenerationTask, result_url: &str) -> Result<String> {
        // Resolve project_id by traversing shot → episode → project.
        let project_id = resolve_project_id(&self.db, task).await?;

        let asset_type = match task.task_type {
            TaskKind::Image => "image",
            TaskKind::Video => "video",
            TaskKind::Audio => "audio",
            TaskKind::Text => {
                return Err(CoreError::Provider(
                    "bailian: text tasks have no downloadable result".into(),
                ))
            }
        };

        let asset = download_to_asset(
            &self.db,
            &project_id,
            task.shot_id.as_deref(),
            result_url,
            asset_type,
        )
        .await?;

        Ok(asset.id)
    }

    async fn cleanup(&self, task: &GenerationTask) -> Result<()> {
        // Parse _internal.uploaded_remote_ids from params_json.
        let params: serde_json::Value = match serde_json::from_str(&task.params_json) {
            Ok(v) => v,
            Err(_) => return Ok(()),
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
            None => return Ok(()),
        };

        if remote_ids.is_empty() {
            return Ok(());
        }

        // Resolve OSS credentials from the account.
        let account_id = task.account_id.clone();
        let keyring = self.keyring.clone();
        let creds_json: Option<String> = self
            .db
            .call(move |conn| {
                Ok(crate::account::service::resolve_credentials(
                    conn,
                    keyring.as_ref(),
                    &account_id,
                ).map(|c| c.extra_json))
            })
            .await
            .map_err(|e: tokio_rusqlite::Error<rusqlite::Error>| CoreError::Provider(format!("cleanup creds: {e}")))?
            .map_err(|e| CoreError::Provider(format!("cleanup creds: {e}")))?;

        let extra = match creds_json {
            Some(json) => json,
            None => {
                tracing::warn!("cleanup: no OSS config for account; skipping");
                return Ok(());
            }
        };

        let uploader = match OssUploader::from_credentials_json(&extra) {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!("cleanup: failed to build OssUploader: {e}");
                return Ok(());
            }
        };

        for remote_id in &remote_ids {
            let _ = uploader.cleanup(remote_id).await;
        }

        Ok(())
    }
}

/// Resolve project_id from task. If task has a shot_id, traverse
/// shot → episode → project. If no shot_id, return Validation error
/// (MS2 tasks must have a shot).
async fn resolve_project_id(
    db: &tokio_rusqlite::Connection,
    task: &GenerationTask,
) -> Result<String> {
    let shot_id = task.shot_id.clone().ok_or_else(|| {
        CoreError::Validation("task without shot_id cannot persist its result".into())
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
