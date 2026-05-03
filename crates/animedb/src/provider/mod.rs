use std::time::Duration;

use crate::error::{Error, Result};
use crate::model::{
    CanonicalMedia, MediaKind, SearchOptions, SourceName, SyncCursor, SyncRequest,
};

// ---------------------------------------------------------------------------
// Core trait types
// ---------------------------------------------------------------------------

/// A page of results returned by a paginated provider fetch.
#[derive(Debug, Clone)]
pub struct FetchPage {
    pub items: Vec<CanonicalMedia>,
    /// `None` when the provider has no more pages.
    pub next_cursor: Option<SyncCursor>,
}

/// Trait every remote provider must implement.
///
/// Each provider is responsible for:
/// - identifying itself via `source()`
/// - declaring its minimum request interval via `min_interval()`
/// - paginated bulk fetches via `fetch_page()`
/// - keyword search via `search()`
/// - single-item lookup via `get_by_id()`
///
/// Providers are pure, stateless structs — they hold only the HTTP client and
/// the base URL, never any in-flight state.
pub trait Provider: Send + Sync {
    fn source(&self) -> SourceName;

    /// Minimum wall-clock delay between successive requests to this provider.
    fn min_interval(&self) -> Duration {
        Duration::ZERO
    }

    fn fetch_page(&self, request: &SyncRequest, cursor: SyncCursor) -> Result<FetchPage>;

    fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<CanonicalMedia>>;

    fn get_by_id(&self, media_kind: MediaKind, source_id: &str) -> Result<Option<CanonicalMedia>>;

    /// Fetches currently trending or popular media, ideal for seeding catalogs.
    fn fetch_trending(&self, _media_kind: MediaKind) -> Result<Vec<CanonicalMedia>> {
        Err(Error::Validation(format!(
            "{} does not support trending",
            self.source()
        )))
    }

    /// Fetches recommendations based on a given media item.
    fn fetch_recommendations(
        &self,
        _media_kind: MediaKind,
        _source_id: &str,
    ) -> Result<Vec<CanonicalMedia>> {
        Err(Error::Validation(format!(
            "{} does not support recommendations",
            self.source()
        )))
    }

    /// Fetches related media (sequels, prequels, spin-offs, etc.) for a given media item.
    fn fetch_related(
        &self,
        _media_kind: MediaKind,
        _source_id: &str,
    ) -> Result<Vec<CanonicalMedia>> {
        Err(Error::Validation(format!(
            "{} does not support relations",
            self.source()
        )))
    }
}

// ---------------------------------------------------------------------------
// Submodules — one file per provider, shared utilities isolated in `http`
// ---------------------------------------------------------------------------

pub mod anilist;
pub mod http;
pub mod imdb;
pub mod jikan;
pub mod kitsu;
pub mod tvmaze;

// ---------------------------------------------------------------------------
// Public re-exports — only the provider structs, nothing internal
// ---------------------------------------------------------------------------

pub use anilist::AniListProvider;
pub use imdb::ImdbProvider;
pub use jikan::JikanProvider;
pub use kitsu::KitsuProvider;
pub use tvmaze::TvmazeProvider;

// Keep the old name alive as an alias so existing call sites don't break.
#[deprecated(since = "0.3.0", note = "use `Provider` instead")]
pub type RemoteProvider = dyn Provider;

/// Compatibility alias — existing code that names `RemotePage` still compiles.
#[deprecated(since = "0.3.0", note = "use `FetchPage` instead")]
pub type RemotePage = FetchPage;