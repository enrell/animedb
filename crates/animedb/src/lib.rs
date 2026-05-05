//! `animedb` is a Rust-first anime and manga metadata crate for local media servers.
//!
//! It exposes two integration modes:
//!
//! - local-first with SQLite schema management, sync, merge, provenance, and FTS search
//!   (enabled by default with `local-db` feature; requires rusqlite)
//! - remote-first with normalized access to supported providers without local persistence
//!   (enabled by default with `remote` feature)
//!
//! The easiest entry points are [`AnimeDb`] for local catalogs and [`RemoteApi`] for direct
//! provider access.
//!
//! # Feature flags
//!
//! - `local-db` (default): local SQLite storage, sync state persistence, and the
//!   [`AnimeDb`] type. This feature pulls in `rusqlite` with a bundled SQLite.
//! - `remote` (default): remote provider clients ([`AniListProvider`], [`TvmazeProvider`], etc.)
//!   and the normalized data model ([`CanonicalMedia`], [`SearchOptions`], etc.).
//!
//! To use animedb as a pure remote-only client (no SQLite dependency), disable both
//! default features and enable only `remote`:
//! ```toml
//! animedb = { version = "0.1", default-features = false, features = ["remote"] }
//! ```

#[cfg(feature = "local-db")]
mod catalog;
#[cfg(feature = "local-db")]
mod db;
mod error;
mod merge;
#[cfg(feature = "local-db")]
pub use merge::make_provenance;
pub use merge::{MergeDecision, merge_episode_source_records, merge_media, provider_weight};
mod model;
pub mod provider;
mod remote;
pub mod repository;
#[cfg(feature = "local-db")]
mod schema;
#[cfg(feature = "local-db")]
pub mod sync;

// ---------------------------------------------------------------------------
// Pure data types: always available (no SQLite dependency)
// ---------------------------------------------------------------------------

pub use error::{Error, Result};
pub use model::{
    CanonicalEpisode, CanonicalMedia, EpisodeSourceRecord, ExternalId, FieldProvenance,
    MediaDocument, MediaKind, SearchHit, SearchOptions, SourceName, SourcePayload, StoredEpisode,
    StoredMedia,
};

// Provider trait and concrete provider structs.
pub use provider::{
    AniListProvider, FetchPage, ImdbProvider, JikanProvider, KitsuProvider, Provider,
    TvmazeProvider,
};

pub use remote::{RemoteApi, RemoteCollection, RemoteSource};

// Re-export sync-related types — they are pure data, no SQLite needed.
pub use model::{PersistedSyncState, SyncCursor, SyncMode, SyncOutcome, SyncReport, SyncRequest};

// ---------------------------------------------------------------------------
// local-db only: requires rusqlite
// ---------------------------------------------------------------------------

#[cfg(feature = "local-db")]
pub use catalog::{RemoteCatalog, RemoteMetadataCollection};

#[cfg(feature = "local-db")]
pub use db::{AnimeDb, MetadataCollection};
