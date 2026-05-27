//! Result materialization abstraction. After a provider reports `Success`,
//! the runner calls [`ResultMaterializer::materialize`] to download the result
//! URL and persist it as a local [`Asset`]. The trait is provider-agnostic;
//! `BailianResultMaterializer` (spec-17) is the first real implementation.
//!
//! [`NoopMaterializer`] is provided for tests and scenarios where no download
//! is needed (e.g. stub providers in the e2e suite).

use async_trait::async_trait;

use crate::error::Result;
use crate::models::generation_task::GenerationTask;
use crate::provider::traits::ProviderCredentials;

/// Outcome of a successful [`ResultMaterializer::materialize`] call.
///
/// The runner records `download` + `persist` events from this so the
/// diagnostics UI can show "下载结果 (3.2 MB, 0.8s)" + "已存入素材库" lines.
#[derive(Debug, Clone)]
pub struct MaterializeOutcome {
    pub asset_id: String,
    pub bytes: u64,
    pub duration_ms: u64,
}

/// Outcome of a [`ResultMaterializer::cleanup`] call. Lists `remote_id`s the
/// implementation failed to delete; the runner emits one `cleanup` warn event
/// per id so the diagnostics UI surfaces lingering OSS objects.
#[derive(Debug, Clone, Default)]
pub struct CleanupOutcome {
    pub failed_remote_ids: Vec<String>,
}

/// Turns a remote result URL into a persisted local Asset.
#[async_trait]
pub trait ResultMaterializer: Send + Sync + 'static {
    /// Called by the runner when a task reaches `Success`. Downloads the result
    /// and inserts an `asset` row. Returns the new `asset_id` plus download
    /// stats so the runner can log `download` + `persist` events.
    async fn materialize(
        &self,
        task: &GenerationTask,
        result_url: &str,
    ) -> Result<MaterializeOutcome>;

    /// Called after the task reaches a terminal state (success or failure).
    /// Allows the provider to clean up remote temporary resources (e.g.
    /// uploaded reference images on OSS). Best-effort — failures are surfaced
    /// via [`CleanupOutcome::failed_remote_ids`] for the runner to log as
    /// `cleanup` warn events, but never bubble as `Err`.
    ///
    /// `credentials` — if supplied, reuse the already-resolved credentials
    /// instead of hitting the keyring again (avoids repeated macOS Keychain
    /// prompts within the same task lifecycle).
    async fn cleanup(
        &self,
        task: &GenerationTask,
        credentials: Option<ProviderCredentials>,
    ) -> Result<CleanupOutcome> {
        let _ = (task, credentials);
        Ok(CleanupOutcome::default())
    }
}

/// No-op materializer for tests and providers that don't produce downloadable
/// results. `materialize` returns an empty asset_id (no asset created).
pub struct NoopMaterializer;

#[async_trait]
impl ResultMaterializer for NoopMaterializer {
    async fn materialize(
        &self,
        _task: &GenerationTask,
        _result_url: &str,
    ) -> Result<MaterializeOutcome> {
        Ok(MaterializeOutcome {
            asset_id: String::new(),
            bytes: 0,
            duration_ms: 0,
        })
    }
}
