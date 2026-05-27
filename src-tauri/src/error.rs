use mango_core::error::CoreError;
use mango_core::provider::error::{ProviderErrorDetail, ProviderErrorKind};
use serde::Serialize;
use specta::Type;

/// IPC error payload — must implement `Serialize` to cross the IPC boundary,
/// and `Type` so tauri-specta emits a matching TypeScript definition.
///
/// `request_id` / `http_status` / `kind` are populated when the underlying
/// failure originated from a structured `ProviderErrorDetail` (provider call
/// or OSS upload), so the diagnostics UI can surface "复制 request_id" /
/// HTTP code without a second IPC round-trip.
#[derive(Debug, Serialize, Type)]
pub struct IpcError {
    pub message: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = Option<specta_typescript::Number>)]
    pub http_status: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<ProviderErrorKind>,
}

impl IpcError {
    pub fn internal(message: impl Into<String>) -> Self {
        IpcError {
            message: message.into(),
            code: "INTERNAL".into(),
            request_id: None,
            http_status: None,
            kind: None,
        }
    }

    fn from_detail(code: &'static str, detail: &ProviderErrorDetail) -> Self {
        IpcError {
            message: detail.message.clone(),
            code: code.into(),
            request_id: detail.request_id.clone(),
            http_status: detail.http_status,
            kind: Some(detail.kind),
        }
    }
}

impl From<CoreError> for IpcError {
    fn from(err: CoreError) -> Self {
        match &err {
            CoreError::NotFound { entity, id } => IpcError {
                message: format!("{entity} '{id}' not found"),
                code: "NOT_FOUND".into(),
                request_id: None,
                http_status: None,
                kind: None,
            },
            CoreError::Validation(msg) => IpcError {
                message: msg.clone(),
                code: "VALIDATION_ERROR".into(),
                request_id: None,
                http_status: None,
                kind: None,
            },
            CoreError::Sqlite(e) => IpcError {
                message: format!("Database error: {e}"),
                code: "DB_ERROR".into(),
                request_id: None,
                http_status: None,
                kind: None,
            },
            CoreError::Io(e) => IpcError {
                message: format!("Filesystem error: {e}"),
                code: "IO_ERROR".into(),
                request_id: None,
                http_status: None,
                kind: None,
            },
            CoreError::Keyring(msg) => IpcError {
                message: format!("Keyring error: {msg}"),
                code: "KEYRING_ERROR".into(),
                request_id: None,
                http_status: None,
                kind: None,
            },
            CoreError::Provider(detail) => IpcError::from_detail("PROVIDER_ERROR", detail),
            CoreError::TaskEngine(msg) => IpcError {
                message: format!("Task engine error: {msg}"),
                code: "TASK_ENGINE_ERROR".into(),
                request_id: None,
                http_status: None,
                kind: None,
            },
            CoreError::Cancelled => IpcError {
                message: "task cancelled".into(),
                code: "TASK_CANCELLED".into(),
                request_id: None,
                http_status: None,
                kind: None,
            },
            CoreError::Upload(detail) => IpcError::from_detail("UPLOAD_ERROR", detail),
        }
    }
}

impl From<tokio_rusqlite::Error> for IpcError {
    fn from(err: tokio_rusqlite::Error) -> Self {
        IpcError {
            message: format!("Database connection error: {err}"),
            code: "DB_ERROR".into(),
            request_id: None,
            http_status: None,
            kind: None,
        }
    }
}

// Note: Tauri 2 provides a blanket `impl<T: Serialize> From<T> for InvokeError`,
// so deriving `Serialize` on IpcError is enough — no manual From impl needed.
