//! DashScope request/response serialization types for BailianProvider.

use serde::{Deserialize, Serialize};

// ─── wan2.7 chat-messages style response ─────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(super) struct Wan27Response {
    pub output: Wan27Output,
    #[serde(default)]
    #[allow(dead_code)]
    pub usage: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Wan27Output {
    pub choices: Vec<Wan27Choice>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Wan27Choice {
    pub message: Wan27Message,
}

#[derive(Debug, Deserialize)]
pub(super) struct Wan27Message {
    pub content: Vec<Wan27Content>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Wan27Content {
    pub image: Option<String>,
}

// ─── happyhorse async submit ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(super) struct AsyncSubmitResponse {
    pub output: AsyncSubmitOutput,
}

#[derive(Debug, Deserialize)]
pub(super) struct AsyncSubmitOutput {
    pub task_id: String,
}

// ─── happyhorse async poll ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(super) struct AsyncPollResponse {
    pub output: AsyncPollOutput,
}

#[derive(Debug, Deserialize)]
pub(super) struct AsyncPollOutput {
    pub task_status: String,
    pub video_url: Option<String>,
    pub message: Option<String>,
}

// ─── shared internal types ───────────────────────────────────────────────────

/// Represents a media entry in happyhorse `params_json.media[]`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct MediaEntry {
    pub asset_id: String,
    /// Local absolute path, resolved by the provider before upload.
    /// Not persisted to DB — only lives in memory during submit.
    #[serde(default)]
    pub local_path: String,
    #[serde(rename = "type")]
    pub media_type: String,
}
