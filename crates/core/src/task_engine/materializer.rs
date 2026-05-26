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

/// Turns a remote result URL into a persisted local Asset.
#[async_trait]
pub trait ResultMaterializer: Send + Sync + 'static {
    /// Called by the runner when a task reaches `Success`. Downloads the result
    /// and inserts an `asset` row. Returns `result_asset_id` which is then
    /// written back to `generation_task.result_asset_id`.
    async fn materialize(&self, task: &GenerationTask, result_url: &str) -> Result<String>;

    /// Called after the task reaches a terminal state (success or failure).
    /// Allows the provider to clean up remote temporary resources (e.g.
    /// uploaded reference images on OSS). Best-effort — failures are logged,
    /// not propagated.
    async fn cleanup(&self, task: &GenerationTask) -> Result<()> {
        let _ = task;
        Ok(())
    }
}

/// No-op materializer for tests and providers that don't produce downloadable
/// results. `materialize` returns an empty string (no asset created).
pub struct NoopMaterializer;

#[async_trait]
impl ResultMaterializer for NoopMaterializer {
    async fn materialize(&self, _task: &GenerationTask, _result_url: &str) -> Result<String> {
        // No asset created; return empty string to signal "no result_asset_id".
        Ok(String::new())
    }
}
