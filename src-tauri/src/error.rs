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
