//! Remote provider trait and concrete provider implementations.
//!
//! # Architecture
//!
//! - [`Provider`] — the core trait; all providers implement this interface
//! - [`FetchPage`] — the return type for paginated sync operations
//! - Concrete providers: [`AniListProvider`], [`JikanProvider`], [`KitsuProvider`],
//!   [`TvmazeProvider`], [`ImdbProvider`]
//! - [`ProviderRegistry`] — runtime registry mapping [`SourceName`] to concrete instances
//!
//! # Adding a new provider
//!
//! 1. Create `src/provider/myprovider.rs` implementing [`Provider`]
//! 2. Add `pub use myprovider::MyProvider;` to this module
//! 3. Register it in [`default_registry`]
//!
//! No changes needed to `remote.rs`, `sync/service.rs`, or any other module.

use std::time::Duration;

use crate::error::{Error, Result};
use crate::model::{
    CanonicalEpisode, CanonicalMedia, MediaKind, SearchOptions, SourceName, SyncCursor, SyncRequest,
};

// ---------------------------------------------------------------------------
// Core trait types
// ---------------------------------------------------------------------------

/// A page of results returned by a paginated provider fetch.
#[derive(Debug, Clone)]
pub struct FetchPage {
    /// The media items on this page.
    pub items: Vec<CanonicalMedia>,
    /// Cursor for the next page. `None` when the provider has exhausted its dataset.
    pub next_cursor: Option<SyncCursor>,
}

/// Trait every remote metadata provider must implement.
///
/// Each provider is responsible for:
/// - identifying itself via [`source`](Provider::source)
/// - declaring its minimum request interval via [`min_interval`](Provider::min_interval)
/// - paginated bulk fetches via [`fetch_page`](Provider::fetch_page)
/// - keyword search via [`search`](Provider::search)
/// - single-item lookup via [`get_by_id`](Provider::get_by_id)
///
/// Providers are pure, stateless structs — they hold only the HTTP client and
/// base URL, never any in-flight state. Rate limiting between page fetches
/// is handled by [`SyncService`](crate::sync::SyncService).
pub trait Provider: Send + Sync {
    /// Returns the provider that this instance represents.
    fn source(&self) -> SourceName;

    /// Minimum wall-clock delay between successive requests to this provider.
    ///
    /// Used by [`SyncService`](crate::sync::SyncService) to respect rate limits.
    /// Return `Duration::ZERO` for providers with no explicit rate limit.
    fn min_interval(&self) -> Duration {
        Duration::ZERO
    }

    /// Fetches one page of media records for a sync request.
    ///
    /// The implementation must handle pagination internally using the `cursor`
    /// argument. When the cursor is `SyncCursor { page: 1 }` (the default), the
    /// first page of the dataset should be returned. When `next_cursor` is `None`,
    /// the provider has exhausted its dataset.
    fn fetch_page(&self, request: &SyncRequest, cursor: SyncCursor) -> Result<FetchPage>;

    /// Performs a free-text search query against the provider.
    fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<CanonicalMedia>>;

    /// Fetches a single media record by its ID on the provider.
    ///
    /// Returns `None` if the ID does not exist on the provider. Returns an error
    /// only on network or protocol failure.
    fn get_by_id(&self, media_kind: MediaKind, source_id: &str) -> Result<Option<CanonicalMedia>>;

    /// Fetches currently trending or popular media, ideal for seeding catalogs.
    ///
    /// Default implementation returns a validation error indicating unsupported.
    fn fetch_trending(&self, _media_kind: MediaKind) -> Result<Vec<CanonicalMedia>> {
        Err(Error::Validation(format!(
            "{} does not support trending",
            self.source()
        )))
    }

    /// Fetches recommendations based on a given media item.
    ///
    /// Default implementation returns a validation error indicating unsupported.
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
    ///
    /// Default implementation returns a validation error indicating unsupported.
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

    /// Fetches episode metadata for a given media item.
    ///
    /// Not all providers expose single-media episode data. Jikan, Kitsu, and TVmaze
    /// implement this method. AniList returns a validation error, and IMDb episode data
    /// is available through bulk dataset ingestion rather than this per-media endpoint.
    fn fetch_episodes(
        &self,
        _media_kind: MediaKind,
        _source_id: &str,
    ) -> Result<Vec<CanonicalEpisode>> {
        Err(Error::Validation(format!(
            "{} does not support episode metadata",
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
pub mod registry;
pub mod tvmaze;

// ---------------------------------------------------------------------------
// Public re-exports — only the provider structs, nothing internal
// ---------------------------------------------------------------------------

pub use anilist::AniListProvider;
pub use imdb::ImdbProvider;
pub use jikan::JikanProvider;
pub use kitsu::KitsuProvider;
pub use registry::{ProviderRegistry, default_registry};
pub use tvmaze::TvmazeProvider;

// Keep the old name alive as an alias so existing call sites don't break.
#[deprecated(since = "0.3.0", note = "use `Provider` instead")]
pub type RemoteProvider = dyn Provider;

/// Compatibility alias — existing code that names `RemotePage` still compiles.
#[deprecated(since = "0.3.0", note = "use `FetchPage` instead")]
pub type RemotePage = FetchPage;
