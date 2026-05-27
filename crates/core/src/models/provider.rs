use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub auth_type: String,
    pub docs_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Model {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    /// One of: 'text' | 'image' | 'video' | 'audio'. Enforced by a CHECK
    /// constraint in `001_initial.sql`.
    pub model_type: String,
}
