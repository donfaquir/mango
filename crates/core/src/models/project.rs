use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    pub style_prompt: String,
    pub global_seed: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct CreateProjectInput {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub style_prompt: Option<String>,
    #[serde(default)]
    pub global_seed: Option<i64>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct UpdateProjectInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_prompt: Option<String>,
    /// None = don't modify, Some(None) = clear to NULL, Some(Some(v)) = set to v
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global_seed: Option<Option<i64>>,
}

#[derive(Debug, Default, Deserialize, TS)]
#[ts(export)]
pub struct ListProjectsOptions {
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}
