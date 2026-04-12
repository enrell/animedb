use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[cfg(feature = "local-db")]
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("not found")]
    NotFound,
    #[error("conflicting external id for provider {provider} and id {source_id}")]
    ConflictingExternalId { provider: String, source_id: String },
}

pub type Result<T> = std::result::Result<T, Error>;
