use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EpisodeCheckpoint {
    pub id: String,
    pub episode_id: String,
    #[specta(type = specta_typescript::Number)]
    pub version_number: i64,
    pub label: Option<String>,
    pub trigger_type: String,
    pub script_text: String,
    pub shots_json: String,
    pub canvas_nodes_json: String,
    pub canvas_edges_json: String,
    pub canvas_viewport_json: String,
    pub change_summary: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EpisodeCheckpointListItem {
    pub id: String,
    pub episode_id: String,
    #[specta(type = specta_typescript::Number)]
    pub version_number: i64,
    pub trigger_type: String,
    pub label: Option<String>,
    pub change_summary: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateCheckpointInput {
    pub episode_id: String,
    pub label: Option<String>,
    #[serde(default)]
    pub trigger_type: Option<String>,
}
