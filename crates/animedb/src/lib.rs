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
pub use provider::{AniListProvider, JikanProvider, KitsuProvider, RemotePage, RemoteProvider};
pub use remote::{RemoteApi, RemoteCollection, RemoteSource};
