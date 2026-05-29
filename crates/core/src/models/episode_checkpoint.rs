use serde::{Deserialize, Serialize};
use specta::Type;

/// A point-in-time snapshot of a single episode's canvas layout. The schema
/// reserves space for additional MS4 fields (`script_text`, `shots_json`,
/// `change_summary`) that this MS3 placeholder does not surface yet — the
/// minimal command writes the schema defaults for those columns.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EpisodeCheckpoint {
    pub id: String,
    pub episode_id: String,
    #[specta(type = specta_typescript::Number)]
    pub version_number: i64,
    pub label: Option<String>,
    pub trigger_type: String,
    pub canvas_nodes_json: String,
    pub canvas_edges_json: String,
    pub canvas_viewport_json: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateCheckpointInput {
    pub episode_id: String,
    /// Optional user-supplied label. Empty / missing maps to NULL in DB.
    pub label: Option<String>,
}
