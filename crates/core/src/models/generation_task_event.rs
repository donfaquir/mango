use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventPhase {
    SubmitUpload,
    SubmitCall,
    Poll,
    Download,
    Persist,
    Cleanup,
}

impl EventPhase {
    /// String form persisted in `generation_task_event.phase`. Must match the
    /// CHECK constraint values in `006_add_generation_task_event.sql`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SubmitUpload => "submit_upload",
            Self::SubmitCall => "submit_call",
            Self::Poll => "poll",
            Self::Download => "download",
            Self::Persist => "persist",
            Self::Cleanup => "cleanup",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "submit_upload" => Some(Self::SubmitUpload),
            "submit_call" => Some(Self::SubmitCall),
            "poll" => Some(Self::Poll),
            "download" => Some(Self::Download),
            "persist" => Some(Self::Persist),
            "cleanup" => Some(Self::Cleanup),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EventSeverity {
    Info,
    Warn,
    Error,
}

impl EventSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "info" => Some(Self::Info),
            "warn" => Some(Self::Warn),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GenerationTaskEvent {
    #[specta(type = specta_typescript::Number)]
    pub id: i64,
    pub task_id: String,
    pub occurred_at: String,
    pub phase: EventPhase,
    pub severity: EventSeverity,
    pub request_id: Option<String>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub http_status: Option<i64>,
    pub details_json: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_string_roundtrip_matches_db_check_values() {
        let cases = [
            (EventPhase::SubmitUpload, "submit_upload"),
            (EventPhase::SubmitCall, "submit_call"),
            (EventPhase::Poll, "poll"),
            (EventPhase::Download, "download"),
            (EventPhase::Persist, "persist"),
            (EventPhase::Cleanup, "cleanup"),
        ];
        for (variant, s) in cases {
            assert_eq!(variant.as_str(), s);
            assert_eq!(EventPhase::from_db_str(s), Some(variant));
        }
        assert_eq!(EventPhase::from_db_str("bogus"), None);
    }

    #[test]
    fn severity_string_roundtrip_matches_db_check_values() {
        let cases = [
            (EventSeverity::Info, "info"),
            (EventSeverity::Warn, "warn"),
            (EventSeverity::Error, "error"),
        ];
        for (variant, s) in cases {
            assert_eq!(variant.as_str(), s);
            assert_eq!(EventSeverity::from_db_str(s), Some(variant));
        }
        assert_eq!(EventSeverity::from_db_str("bogus"), None);
    }
}
