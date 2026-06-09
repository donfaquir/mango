use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Character {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub appearance_prompt: String,
    pub reference_image_path: Option<String>,
    pub voice_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateCharacterInput {
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub appearance_prompt: Option<String>,
    #[serde(default)]
    pub reference_image_path: Option<String>,
    #[serde(default)]
    pub voice_id: Option<String>,
}

#[derive(Debug, Deserialize, Type)]
pub struct UpdateCharacterInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance_prompt: Option<String>,
    /// None = don't modify, Some(None) = clear to NULL, Some(Some(v)) = set to v
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_image_path: Option<Option<String>>,
    /// None = don't modify, Some(None) = clear to NULL, Some(Some(v)) = set to v
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice_id: Option<Option<String>>,
}

#[derive(Debug, Deserialize, Type)]
pub struct ListCharactersOptions {
    pub project_id: String,
    #[serde(default)]
    #[specta(type = Option<specta_typescript::Number>)]
    pub limit: Option<i64>,
    #[serde(default)]
    #[specta(type = Option<specta_typescript::Number>)]
    pub offset: Option<i64>,
}
