use mango_core::error::CoreError;
use serde::Serialize;
use specta::Type;

/// IPC error payload — must implement `Serialize` to cross the IPC boundary,
/// and `Type` so tauri-specta emits a matching TypeScript definition.
#[derive(Debug, Serialize, Type)]
pub struct IpcError {
    pub message: String,
    pub code: String,
}

impl IpcError {
    pub fn internal(message: impl Into<String>) -> Self {
        IpcError {
            message: message.into(),
            code: "INTERNAL".into(),
        }
    }
}

impl From<CoreError> for IpcError {
    fn from(err: CoreError) -> Self {
        match &err {
            CoreError::NotFound { entity, id } => IpcError {
                message: format!("{entity} '{id}' not found"),
                code: "NOT_FOUND".into(),
            },
            CoreError::Validation(msg) => IpcError {
                message: msg.clone(),
                code: "VALIDATION_ERROR".into(),
            },
            CoreError::Sqlite(e) => IpcError {
                message: format!("Database error: {e}"),
                code: "DB_ERROR".into(),
            },
            CoreError::Io(e) => IpcError {
                message: format!("Filesystem error: {e}"),
                code: "IO_ERROR".into(),
            },
            CoreError::Keyring(msg) => IpcError {
                message: format!("Keyring error: {msg}"),
                code: "KEYRING_ERROR".into(),
            },
            CoreError::Provider(msg) => IpcError {
                message: format!("Provider error: {msg}"),
                code: "PROVIDER_ERROR".into(),
            },
            CoreError::TaskEngine(msg) => IpcError {
                message: format!("Task engine error: {msg}"),
                code: "TASK_ENGINE_ERROR".into(),
            },
            CoreError::Cancelled => IpcError {
                message: "task cancelled".into(),
                code: "TASK_CANCELLED".into(),
            },
            CoreError::Upload(msg) => IpcError {
                message: format!("Upload error: {msg}"),
                code: "UPLOAD_ERROR".into(),
            },
        }
    }
}

impl From<tokio_rusqlite::Error> for IpcError {
    fn from(err: tokio_rusqlite::Error) -> Self {
        IpcError {
            message: format!("Database connection error: {err}"),
            code: "DB_ERROR".into(),
        }
    }
}

// Note: Tauri 2 provides a blanket `impl<T: Serialize> From<T> for InvokeError`,
// so deriving `Serialize` on IpcError is enough — no manual From impl needed.
