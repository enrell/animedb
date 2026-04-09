//! `animedb` is a Rust-first anime and manga metadata crate for local media servers.
//!
//! It exposes two integration modes:
//!
//! - local-first with SQLite schema management, sync, merge, provenance, and FTS search
//! - remote-first with normalized access to supported providers without local persistence
//!
//! The easiest entry points are [`AnimeDb`] for local catalogs and [`RemoteApi`] for direct
//! provider access.

mod catalog;
mod db;
mod error;
mod merge;
mod model;
mod provider;
mod remote;

pub use catalog::{RemoteCatalog, RemoteMetadataCollection};
pub use db::{AnimeDb, MetadataCollection};
pub use error::{Error, Result};
pub use model::{
    CanonicalMedia, ExternalId, FieldProvenance, MediaKind, PersistedSyncState, SearchHit,
    SearchOptions, SourceName, SourcePayload, StoredMedia, SyncCursor, SyncMode, SyncOutcome,
    SyncReport, SyncRequest,
};
pub use provider::{
    AniListProvider, ImdbProvider, JikanProvider, KitsuProvider, RemotePage, RemoteProvider,
    TvmazeProvider,
};
pub use remote::{RemoteApi, RemoteCollection, RemoteSource};
