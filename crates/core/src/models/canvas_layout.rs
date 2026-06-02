use serde::{Deserialize, Serialize};
use specta::Type;

/// Canvas layout for a single episode. `nodes_json` / `edges_json` /
/// `viewport_json` are opaque JSON blobs owned by the frontend's React Flow
/// state — core never parses their inner structure, only that they are
/// well-formed JSON. See spec-21 for rationale.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CanvasLayout {
    pub id: String,
    pub episode_id: String,
    pub nodes_json: String,
    pub edges_json: String,
    pub viewport_json: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct UpsertCanvasLayoutInput {
    pub episode_id: String,
    pub nodes_json: String,
    pub edges_json: String,
    pub viewport_json: String,
}
