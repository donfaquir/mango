use thiserror::Error;

use crate::provider::error::ProviderErrorDetail;

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

    // Wraps any error a `ModelProvider` implementation surfaces (HTTP, auth,
    // remote business). Concrete implementations populate `ProviderErrorDetail`
    // with `kind`/`request_id`/`http_status` so the diagnostics UI and CLI can
    // render rich failure context.
    #[error("provider error: {0}")]
    Provider(ProviderErrorDetail),

    // Task engine internal failures: registry misses, illegal state-machine
    // transitions, and other invariants the engine must enforce.
    #[error("task engine error: {0}")]
    TaskEngine(String),

    // Distinct from Failed: the UI must not render this as an error.
    #[error("task cancelled")]
    Cancelled,

    // OSS upload / cleanup failure (spec-16). Distinct from `Provider` so the
    // UI can tell "OSS misconfigured" apart from "Bailian rejected the
    // request"; same structured shape so diagnostics can render request_id /
    // http_status uniformly.
    #[error("upload error: {0}")]
    Upload(ProviderErrorDetail),
}

pub type Result<T> = std::result::Result<T, CoreError>;
