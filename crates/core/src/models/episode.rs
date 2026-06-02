use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Episode {
    pub id: String,
    pub project_id: String,
    pub title: String,
    #[specta(type = specta_typescript::Number)]
    pub order_index: i64,
    pub script_text: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateEpisodeInput {
    pub project_id: String,
    pub title: String,
    #[serde(default)]
    pub script_text: Option<String>,
}

#[derive(Debug, Deserialize, Type)]
pub struct UpdateEpisodeInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_text: Option<String>,
}

#[derive(Debug, Deserialize, Type)]
pub struct ListEpisodesOptions {
    pub project_id: String,
}
