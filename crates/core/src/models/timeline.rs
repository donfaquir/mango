use serde::{Deserialize, Serialize};
use specta::Type;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum TrackType {
    Video,
    Audio,
    Text,
    Overlay,
}

impl TrackType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Text => "text",
            Self::Overlay => "overlay",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "video" => Some(Self::Video),
            "audio" => Some(Self::Audio),
            "text" => Some(Self::Text),
            "overlay" => Some(Self::Overlay),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ItemType {
    Clip,
    Text,
    Sticker,
    Transition,
    Effect,
}

impl ItemType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Clip => "clip",
            Self::Text => "text",
            Self::Sticker => "sticker",
            Self::Transition => "transition",
            Self::Effect => "effect",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "clip" => Some(Self::Clip),
            "text" => Some(Self::Text),
            "sticker" => Some(Self::Sticker),
            "transition" => Some(Self::Transition),
            "effect" => Some(Self::Effect),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct TimelineTrack {
    pub id: String,
    pub episode_id: String,
    pub track_type: TrackType,
    pub label: String,
    pub order_index: i32,
    pub muted: bool,
    pub locked: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Type, TS)]
#[ts(export)]
pub struct CreateTimelineTrackInput {
    pub episode_id: String,
    pub track_type: TrackType,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct TimelineItem {
    pub id: String,
    pub track_id: String,
    pub asset_id: Option<String>,
    pub item_type: ItemType,
    #[specta(type = specta_typescript::Number)]
    pub position_ms: i64,
    #[specta(type = specta_typescript::Number)]
    pub duration_ms: i64,
    #[specta(type = specta_typescript::Number)]
    pub in_point_ms: i64,
    #[specta(type = specta_typescript::Number)]
    pub out_point_ms: i64,
    pub params_json: String,
    pub order_index: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Type, TS)]
#[ts(export)]
pub struct CreateTimelineItemInput {
    pub track_id: String,
    pub asset_id: Option<String>,
    pub item_type: ItemType,
    #[specta(type = specta_typescript::Number)]
    pub position_ms: i64,
    #[specta(type = specta_typescript::Number)]
    pub duration_ms: i64,
    #[specta(type = Option<specta_typescript::Number>)]
    pub in_point_ms: Option<i64>,
    #[specta(type = specta_typescript::Number)]
    pub out_point_ms: i64,
    pub params_json: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Type, TS)]
#[ts(export)]
pub struct UpdateTimelineItemInput {
    #[specta(type = Option<specta_typescript::Number>)]
    pub position_ms: Option<i64>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub duration_ms: Option<i64>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub in_point_ms: Option<i64>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub out_point_ms: Option<i64>,
    pub params_json: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Type, TS)]
#[ts(export)]
pub struct MoveTimelineItemInput {
    pub track_id: String,
    #[specta(type = specta_typescript::Number)]
    pub position_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct TimelineKeyframe {
    pub id: String,
    pub item_id: String,
    pub property: String,
    #[specta(type = specta_typescript::Number)]
    pub time_ms: i64,
    pub value: f64,
    pub easing: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Type, TS)]
#[ts(export)]
pub struct CreateTimelineKeyframeInput {
    pub item_id: String,
    pub property: String,
    #[specta(type = specta_typescript::Number)]
    pub time_ms: i64,
    pub value: f64,
    #[serde(default = "default_easing")]
    pub easing: String,
}

fn default_easing() -> String {
    "linear".into()
}
