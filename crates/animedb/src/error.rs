//! Errors returned by the animedb library.
//!
//! All errors are in `animedb::Error` form. No raw SQLite, HTTP, or JSON errors
//! leak through the public API — they are always wrapped as one of the variants
//! below.

use thiserror::Error;

/// The library's unified error type.
///
/// Operations that can fail expose `animedb::Result<T>` (alias for
/// `std::result::Result<T, Error>`) as their return type.
#[derive(Debug, Error)]
pub enum Error {
    /// SQLite error (connection, statement, or transaction failure).
    #[cfg(feature = "local-db")]
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// JSON serialization or deserialization failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP request failure (network error, TLS error, non-2xx status).
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// Input validation failure — returned when required fields are missing,
    /// enum variants cannot be parsed, or a sync source mismatch is detected.
    #[error("validation error: {0}")]
    Validation(String),

    /// A requested media record was not found in the local database.
    #[error("not found")]
    NotFound,

    /// An external ID resolves to different media records depending on the
    /// media kind, and the kind was not specified. Use
    /// [`crate::repository::MediaRepository::get_by_external_id_and_kind`] with an explicit kind.
    #[error("conflicting external id for provider {provider} and id {source_id}")]
    ConflictingExternalId {
        /// Provider name that caused the conflict.
        provider: String,
        /// Source-specific ID that is ambiguous.
        source_id: String,
    },

    /// A mutex was poisoned (internal synchronization error).
    #[error("internal sync error")]
    Sync(String),
}

pub type Result<T> = std::result::Result<T, Error>;
