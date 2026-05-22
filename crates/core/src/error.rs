use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    // Keyring errors are kept as Strings because the underlying `keyring_core::Error`
    // has different variants per platform store, which would leak through the IPC
    // boundary if exposed directly.
    #[error("keyring error: {0}")]
    Keyring(String),

    #[error("not found: {entity} with id '{id}'")]
    NotFound { entity: &'static str, id: String },

    #[error("validation error: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
