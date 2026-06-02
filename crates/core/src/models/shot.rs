use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ShotStatus {
    Draft,
    Ready,
    Generating,
    Done,
}

impl ShotStatus {
    pub fn as_db_str(self) -> &'static str {
        match self {
            ShotStatus::Draft => "draft",
            ShotStatus::Ready => "ready",
            ShotStatus::Generating => "generating",
            ShotStatus::Done => "done",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(ShotStatus::Draft),
            "ready" => Some(ShotStatus::Ready),
            "generating" => Some(ShotStatus::Generating),
            "done" => Some(ShotStatus::Done),
            _ => None,
        }
    }
}

/// Which side-table a subject points to. Used by `link_shot_subject` /
/// `unlink_shot_subject` to dispatch to the right join table without
/// exploding the IPC surface into three near-identical commands.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SubjectKind {
    Character,
    Scene,
    Prop,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Shot {
    pub id: String,
    pub episode_id: String,
    #[specta(type = specta_typescript::Number)]
    pub order_index: i64,
    pub summary: String,
    pub duration_sec: Option<f64>,
    pub camera_angle: String,
    pub shot_type: String,
    pub mood: String,
    pub dialogue: String,
    pub video_prompt: String,
    pub image_prompt: String,
    pub status: ShotStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateShotInput {
    pub episode_id: String,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize, Type, Default)]
pub struct UpdateShotInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// None = don't modify, Some(None) = clear, Some(Some(v)) = set
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_sec: Option<Option<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_angle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shot_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mood: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialogue: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ShotStatus>,
}

#[derive(Debug, Deserialize, Type)]
pub struct ListShotsOptions {
    pub episode_id: String,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct ShotLinks {
    pub character_ids: Vec<String>,
    pub scene_ids: Vec<String>,
    pub prop_ids: Vec<String>,
}
