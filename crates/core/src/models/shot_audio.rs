use serde::{Deserialize, Serialize};
use specta::Type;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum AudioRole {
    Voice,
    Sfx,
    Bgm,
}

impl AudioRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Voice => "voice",
            Self::Sfx => "sfx",
            Self::Bgm => "bgm",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "voice" => Some(Self::Voice),
            "sfx" => Some(Self::Sfx),
            "bgm" => Some(Self::Bgm),
            _ => None,
        }
    }

    pub fn is_unique(self) -> bool {
        matches!(self, Self::Voice | Self::Bgm)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct ShotAudio {
    pub id: String,
    pub shot_id: String,
    pub asset_id: String,
    pub audio_role: AudioRole,
    pub volume: f64,
    #[specta(type = specta_typescript::Number)]
    pub offset_ms: i64,
    #[specta(type = specta_typescript::Number)]
    pub order_index: i64,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Type, TS)]
#[ts(export)]
pub struct CreateShotAudioInput {
    pub shot_id: String,
    pub asset_id: String,
    pub audio_role: AudioRole,
    #[serde(default = "default_volume")]
    pub volume: f64,
    #[serde(default)]
    #[specta(type = specta_typescript::Number)]
    pub offset_ms: i64,
}

#[derive(Debug, Deserialize, Type, TS)]
#[ts(export)]
pub struct UpdateShotAudioInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[specta(type = Option<specta_typescript::Number>)]
    pub offset_ms: Option<i64>,
}

fn default_volume() -> f64 {
    1.0
}
