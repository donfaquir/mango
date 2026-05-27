use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Scene {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub environment_prompt: String,
    pub reference_image_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateSceneInput {
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub environment_prompt: Option<String>,
    #[serde(default)]
    pub reference_image_path: Option<String>,
}

#[derive(Debug, Deserialize, Type)]
pub struct UpdateSceneInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment_prompt: Option<String>,
    /// None = don't modify, Some(None) = clear to NULL, Some(Some(v)) = set to v
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_image_path: Option<Option<String>>,
}

#[derive(Debug, Deserialize, Type)]
pub struct ListScenesOptions {
    pub project_id: String,
    #[serde(default)]
    #[specta(type = Option<specta_typescript::Number>)]
    pub limit: Option<i64>,
    #[serde(default)]
    #[specta(type = Option<specta_typescript::Number>)]
    pub offset: Option<i64>,
}
