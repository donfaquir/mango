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
    /// Public-side OSS configuration. Only populated for accounts with an
    /// `oss` block in `params_json`. The access_key_secret never appears here
    /// — it lives in the keyring under entry `api_account:<id>:oss_secret`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oss_config: Option<OssConfigPublic>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub usage_quota: Option<i64>,
    #[specta(type = specta_typescript::Number)]
    pub usage_used: i64,
    pub last_used_at: Option<String>,
    pub created_at: String,
}

/// OSS configuration as exposed back to the frontend / IPC. Mirrors
/// `OssConfigInput` minus `access_key_secret`.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OssConfigPublic {
    pub endpoint: String,
    pub bucket: String,
    /// AccessKey ID is fine to surface — it's the public half of the credential
    /// pair. Pairs with the secret stored in keyring.
    pub access_key_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// Validity (seconds) for presigned GET URLs minted by this account's
    /// uploader. Defaults to 3600.
    #[specta(type = specta_typescript::Number)]
    pub url_expires_seconds: u32,
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateApiAccountInput {
    pub provider_id: String,
    pub label: String,
    /// Plaintext API key. Written to the system keyring immediately and never
    /// persisted to SQLite or returned to the caller.
    pub api_key: String,
    /// Required for providers that need to push local assets to a public URL
    /// (MS2: only `bailian`); ignored for others. Validation lives in
    /// `account::service::validate_oss`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oss: Option<OssConfigInput>,
}

#[derive(Debug, Deserialize, Type)]
pub struct OssConfigInput {
    pub endpoint: String,
    pub bucket: String,
    pub access_key_id: String,
    /// Plaintext OSS secret. Persisted to the keyring immediately and dropped
    /// from memory; never written to SQLite.
    pub access_key_secret: String,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default = "default_expires")]
    #[specta(type = specta_typescript::Number)]
    pub url_expires_seconds: u32,
}

fn default_expires() -> u32 {
    3600
}

#[derive(Debug, Deserialize, Type)]
pub struct UpdateApiAccountInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Some(new_key) replaces the keyring entry; None leaves it untouched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Some(_) replaces the entire OSS config (all 4+ fields together); None
    /// leaves it untouched. Sub-field updates are intentionally not supported
    /// — the frontend always re-submits the whole block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oss: Option<OssConfigInput>,
}
