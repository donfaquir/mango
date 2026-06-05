use serde::{Deserialize, Serialize};
use specta::Type;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct VideoClip {
    pub id: String,
    pub project_id: String,
    pub episode_id: Option<String>,
    pub source_asset_id: String,
    pub label: Option<String>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub trim_start_ms: Option<i64>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub trim_end_ms: Option<i64>,
    pub order_index: i32,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Type, TS)]
#[ts(export)]
pub struct CreateVideoClipInput {
    pub project_id: String,
    pub episode_id: Option<String>,
    pub source_asset_id: String,
    pub label: Option<String>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub trim_start_ms: Option<i64>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub trim_end_ms: Option<i64>,
}

#[derive(Debug, Deserialize, Type, TS)]
#[ts(export)]
pub struct UpdateVideoClipInput {
    pub label: Option<String>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub trim_start_ms: Option<i64>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub trim_end_ms: Option<i64>,
}
