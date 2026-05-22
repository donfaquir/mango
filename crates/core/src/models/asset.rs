use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum AssetType {
    Image,
    Video,
    Audio,
    Script,
}

impl AssetType {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            AssetType::Image => "image",
            AssetType::Video => "video",
            AssetType::Audio => "audio",
            AssetType::Script => "script",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "image" => Some(AssetType::Image),
            "video" => Some(AssetType::Video),
            "audio" => Some(AssetType::Audio),
            "script" => Some(AssetType::Script),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Asset {
    pub id: String,
    pub project_id: String,
    pub shot_id: Option<String>,
    pub asset_type: AssetType,
    pub original_name: String,
    /// Stored relative to the project root (e.g. `assets/{id}.png`), always
    /// `/`-separated for cross-platform stability.
    pub file_path: String,
    pub thumbnail_path: Option<String>,
    #[specta(type = specta_typescript::Number)]
    pub file_size: i64,
    pub content_hash: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct ImportAssetInput {
    pub project_id: String,
    /// Absolute path of the external source file (drag-drop or file picker).
    pub source_path: String,
    /// Optional shot binding; MS1 reference-image flows leave this unset.
    /// Empty strings are normalised to None.
    #[serde(default)]
    pub shot_id: Option<String>,
}

#[derive(Debug, Deserialize, Type)]
pub struct ListAssetsOptions {
    pub project_id: String,
    #[serde(default)]
    pub asset_type: Option<AssetType>,
    #[serde(default)]
    #[specta(type = Option<specta_typescript::Number>)]
    pub limit: Option<i64>,
    #[serde(default)]
    #[specta(type = Option<specta_typescript::Number>)]
    pub offset: Option<i64>,
}
