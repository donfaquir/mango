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

    // Wraps any error a `ModelProvider` implementation surfaces (HTTP, auth,
    // remote business). Concrete implementations use `thiserror` internally and
    // collapse to String at the core boundary so trait-object variance does not
    // leak through.
    #[error("provider error: {0}")]
    Provider(String),

    // Task engine internal failures: registry misses, illegal state-machine
    // transitions, and other invariants the engine must enforce.
    #[error("task engine error: {0}")]
    TaskEngine(String),

    // Distinct from Failed: the UI must not render this as an error.
    #[error("task cancelled")]
    Cancelled,

    // OSS upload / cleanup failure (spec-16). The asset_uploader layer
    // collapses SDK / network / signature errors to String here so the IPC
    // boundary stays free of provider-specific variants. Distinct from
    // `Provider`: this lets the UI tell "OSS misconfigured" apart from
    // "Bailian rejected the request".
    #[error("upload error: {0}")]
    Upload(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
