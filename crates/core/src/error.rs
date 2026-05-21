use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("not found: {entity} with id '{id}'")]
    NotFound { entity: &'static str, id: String },

    #[error("validation error: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
