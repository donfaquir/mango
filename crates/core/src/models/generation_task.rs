use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GenerationTaskStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
}

impl GenerationTaskStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Success | Self::Failed | Self::Cancelled)
    }

    /// String form persisted in `generation_task.status`. Must match the
    /// CHECK constraint values in `001_initial.sql`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "success" => Some(Self::Success),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskKind {
    Text,
    Image,
    Video,
    Audio,
}

impl TaskKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "text" => Some(Self::Text),
            "image" => Some(Self::Image),
            "video" => Some(Self::Video),
            "audio" => Some(Self::Audio),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GenerationTask {
    pub id: String,
    pub project_id: Option<String>,
    pub shot_id: Option<String>,
    pub provider_id: String,
    pub model_id: String,
    pub account_id: String,
    pub task_type: TaskKind,
    pub params_json: String,
    pub status: GenerationTaskStatus,
    pub result_asset_id: Option<String>,
    pub external_task_id: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error_message: Option<String>,
    #[specta(type = specta_typescript::Number)]
    pub retry_count: i64,
    pub created_at: String,
    /// Groups multiple tasks created from a single batch submission. NULL for
    /// single-task submits (the MS2 path). Set by [`task_engine::submit_batch`]
    /// to a UUID v4 shared by every row in the batch.
    pub batch_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Type)]
pub struct CreateGenerationTaskInput {
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub shot_id: Option<String>,
    pub provider_id: String,
    pub model_id: String,
    pub account_id: String,
    pub task_type: TaskKind,
    /// JSON-encoded provider-specific parameters. Validated by the chosen
    /// provider, not this layer; core only asserts it parses as JSON.
    #[serde(default)]
    pub params_json: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_terminal_marks_only_terminal_states() {
        assert!(!GenerationTaskStatus::Pending.is_terminal());
        assert!(!GenerationTaskStatus::Running.is_terminal());
        assert!(GenerationTaskStatus::Success.is_terminal());
        assert!(GenerationTaskStatus::Failed.is_terminal());
        assert!(GenerationTaskStatus::Cancelled.is_terminal());
    }

    #[test]
    fn status_string_roundtrip_matches_db_check_values() {
        let cases = [
            (GenerationTaskStatus::Pending, "pending"),
            (GenerationTaskStatus::Running, "running"),
            (GenerationTaskStatus::Success, "success"),
            (GenerationTaskStatus::Failed, "failed"),
            (GenerationTaskStatus::Cancelled, "cancelled"),
        ];
        for (variant, s) in cases {
            assert_eq!(variant.as_str(), s);
            assert_eq!(GenerationTaskStatus::from_db_str(s), Some(variant));
        }
        assert_eq!(GenerationTaskStatus::from_db_str("bogus"), None);
    }

    #[test]
    fn task_kind_string_roundtrip() {
        let cases = [
            (TaskKind::Text, "text"),
            (TaskKind::Image, "image"),
            (TaskKind::Video, "video"),
            (TaskKind::Audio, "audio"),
        ];
        for (variant, s) in cases {
            assert_eq!(variant.as_str(), s);
            assert_eq!(TaskKind::from_db_str(s), Some(variant));
        }
    }
}
