use serde::{Deserialize, Serialize};
use specta::Type;

// i64 fields below are tagged `#[specta(type = Number)]` so they emit as TS
// `number` instead of `bigint`. All values are bounded well under 2^53:
// pagination limit/offset never approach that, and RNG seeds are user-supplied
// integers, typically i32 range. SQLite stores them as INTEGER (i64) regardless.

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    pub style_prompt: String,
    /// Workspace-relative POSIX path to the project root (e.g.
    /// `projects/ep4-the-rain`). Resolved to an absolute filesystem path at
    /// the IPC boundary by `paths::resolve_project_root(workspace, ...)`, so
    /// frontend consumers see an absolute path. The relative form keeps the
    /// whole workspace portable across machines.
    pub root_path: String,
    #[specta(type = Option<specta_typescript::Number>)]
    pub global_seed: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateProjectInput {
    pub name: String,
    /// User-chosen subdirectory name under `<workspace>/projects/`. None
    /// falls back to a slug derived from `name`. Must be a single path
    /// segment — no separators, no `..`, no control chars.
    #[serde(default)]
    pub subdir: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub style_prompt: Option<String>,
    #[serde(default)]
    #[specta(type = Option<specta_typescript::Number>)]
    pub global_seed: Option<i64>,
}

#[derive(Debug, Deserialize, Type)]
pub struct UpdateProjectInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_prompt: Option<String>,
    /// None = don't modify, Some(None) = clear to NULL, Some(Some(v)) = set to v
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[specta(type = Option<Option<specta_typescript::Number>>)]
    pub global_seed: Option<Option<i64>>,
}

#[derive(Debug, Default, Deserialize, Type)]
pub struct ListProjectsOptions {
    #[serde(default)]
    #[specta(type = Option<specta_typescript::Number>)]
    pub limit: Option<i64>,
    #[serde(default)]
    #[specta(type = Option<specta_typescript::Number>)]
    pub offset: Option<i64>,
}
