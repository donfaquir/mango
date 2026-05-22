use serde::{Deserialize, Serialize};
use specta::Type;

/// IPC-exposed account row. Crucially excludes `api_key_ref`: the frontend has
/// no business reading the keyring entry name, and not exporting it makes
/// accidental misuse a type error.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ApiAccount {
    pub id: String,
    pub provider_id: String,
    pub label: String,
    pub key_last4: String,
    #[specta(type = Option<specta_typescript::Number>)]
    pub usage_quota: Option<i64>,
    #[specta(type = specta_typescript::Number)]
    pub usage_used: i64,
    pub last_used_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateApiAccountInput {
    pub provider_id: String,
    pub label: String,
    /// Plaintext API key. Written to the system keyring immediately and never
    /// persisted to SQLite or returned to the caller.
    pub api_key: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct UpdateApiAccountInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Some(new_key) replaces the keyring entry; None leaves it untouched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}
