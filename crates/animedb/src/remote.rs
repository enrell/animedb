//! Remote-first facade over the supported metadata providers.
//!
//! # Design
//!
//! [`RemoteApi`] wraps a `Box<dyn Provider>` so that adding a new provider
//! requires **zero changes** to this file — only the provider struct and a
//! new constructor on `RemoteApi`.
//!
//! The old `match self.source { … }` style required editing `remote.rs` every
//! time a provider was added or removed.  By storing the trait object we
//! decouple the dispatch entirely.
use std::sync::Arc;

use crate::error::Result;
use crate::model::{CanonicalMedia, MediaKind, SearchOptions};
use crate::provider::{
    AniListProvider, ImdbProvider, JikanProvider, KitsuProvider, Provider, TvmazeProvider,
};

// ---------------------------------------------------------------------------
// RemoteSource — kept for compatibility; maps to concrete providers
// ---------------------------------------------------------------------------

/// Named variants for the built-in providers.
///
/// Using [`RemoteApi::with_provider`] with any type implementing [`Provider`] is preferred for new
/// code; this enum exists for ergonomic constructors and serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteSource {
    AniList,
    Jikan,
    Kitsu,
    Tvmaze,
    Imdb,
}

impl RemoteSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AniList => "anilist",
            Self::Jikan => "jikan",
            Self::Kitsu => "kitsu",
            Self::Tvmaze => "tvmaze",
            Self::Imdb => "imdb",
        }
    }
}

// ---------------------------------------------------------------------------
// RemoteApi
// ---------------------------------------------------------------------------

/// Remote-first facade over a single metadata provider.
///
/// Construct it with one of the named constructors (`::anilist()`, etc.) or
/// wrap any `Provider` impl with `RemoteApi::with_provider(my_provider)`.
pub struct RemoteApi {
    provider: Arc<dyn Provider>,
}

impl Default for RemoteApi {
    fn default() -> Self {
        Self::anilist()
    }
}

impl RemoteApi {
    // -- Named constructors -------------------------------------------------

    pub fn anilist() -> Self {
        Self::with_provider(AniListProvider::new())
    }

    pub fn jikan() -> Self {
        Self::with_provider(JikanProvider::new())
    }

    pub fn kitsu() -> Self {
        Self::with_provider(KitsuProvider::new())
    }

    pub fn tvmaze() -> Self {
        Self::with_provider(TvmazeProvider::new())
    }

    pub fn imdb() -> Self {
        Self::with_provider(ImdbProvider::new())
    }

    // -- Generic constructor ------------------------------------------------

    /// Wraps any type that implements [`Provider`].
    pub fn with_provider<P: Provider + 'static>(provider: P) -> Self {
        Self {
            provider: Arc::new(provider),
        }
    }

    // -- Accessors ----------------------------------------------------------

    /// Returns a handle to the underlying provider (for introspection / tests).
    pub fn provider(&self) -> &dyn Provider {
        self.provider.as_ref()
    }

    // -- Query API ----------------------------------------------------------

    /// Searches the selected provider directly.
    pub fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<CanonicalMedia>> {
        self.provider.search(query, options)
    }

    /// Fetches trending media for the given media kind directly from the provider.
    pub fn fetch_trending(&self, media_kind: MediaKind) -> Result<Vec<CanonicalMedia>> {
        self.provider.fetch_trending(media_kind)
    }

    /// Fetches recommendations for the given source ID directly from the provider.
    pub fn fetch_recommendations(
        &self,
        media_kind: MediaKind,
        source_id: &str,
    ) -> Result<Vec<CanonicalMedia>> {
        self.provider.fetch_recommendations(media_kind, source_id)
    }

    /// Fetches related media for the given source ID directly from the provider.
    pub fn fetch_related(
        &self,
        media_kind: MediaKind,
        source_id: &str,
    ) -> Result<Vec<CanonicalMedia>> {
        self.provider.fetch_related(media_kind, source_id)
    }

    /// Narrows queries to anime records.
    pub fn anime_metadata(&self) -> RemoteCollection {
        RemoteCollection::new(
            Arc::clone(&self.provider),
            SearchOptions::default().with_media_kind(MediaKind::Anime),
        )
    }

    /// Narrows queries to manga records.
    pub fn manga_metadata(&self) -> RemoteCollection {
        RemoteCollection::new(
            Arc::clone(&self.provider),
            SearchOptions::default().with_media_kind(MediaKind::Manga),
        )
    }

    /// Narrows queries to anime movies.
    pub fn movie_metadata(&self) -> RemoteCollection {
        RemoteCollection::new(
            Arc::clone(&self.provider),
            SearchOptions::default()
                .with_media_kind(MediaKind::Anime)
                .with_format("MOVIE"),
        )
    }
}

// ---------------------------------------------------------------------------
// RemoteCollection
// ---------------------------------------------------------------------------

/// Filtered view over one provider and one media slice.
pub struct RemoteCollection {
    provider: Arc<dyn Provider>,
    options: SearchOptions,
}

impl RemoteCollection {
    fn new(provider: Arc<dyn Provider>, options: SearchOptions) -> Self {
        Self { provider, options }
    }

    pub fn options(&self) -> &SearchOptions {
        &self.options
    }

    pub fn search(&self, query: &str) -> Result<Vec<CanonicalMedia>> {
        self.provider.search(query, self.options.clone())
    }

    pub fn by_id(&self, source_id: &str) -> Result<Option<CanonicalMedia>> {
        let kind = self.options.media_kind.unwrap_or(MediaKind::Anime);
        let item = self.provider.get_by_id(kind, source_id)?;

        Ok(item.filter(|m| {
            self.options
                .format
                .as_ref()
                .map(|fmt| m.format.as_ref().map(|v| v.eq_ignore_ascii_case(fmt)) == Some(true))
                .unwrap_or(true)
        }))
    }

    pub fn trending(&self) -> Result<Vec<CanonicalMedia>> {
        let kind = self.options.media_kind.unwrap_or(MediaKind::Anime);
        self.provider.fetch_trending(kind)
    }

    pub fn recommendations(&self, source_id: &str) -> Result<Vec<CanonicalMedia>> {
        let kind = self.options.media_kind.unwrap_or(MediaKind::Anime);
        self.provider.fetch_recommendations(kind, source_id)
    }

    pub fn related(&self, source_id: &str) -> Result<Vec<CanonicalMedia>> {
        let kind = self.options.media_kind.unwrap_or(MediaKind::Anime);
        self.provider.fetch_related(kind, source_id)
    }
}

// ---------------------------------------------------------------------------
// Compatibility: keep RemoteSource-based constructors via From
// ---------------------------------------------------------------------------

impl From<RemoteSource> for RemoteApi {
    fn from(source: RemoteSource) -> Self {
        match source {
            RemoteSource::AniList => Self::anilist(),
            RemoteSource::Jikan => Self::jikan(),
            RemoteSource::Kitsu => Self::kitsu(),
            RemoteSource::Tvmaze => Self::tvmaze(),
            RemoteSource::Imdb => Self::imdb(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SourceName;

    #[test]
    fn default_remote_api_uses_anilist() {
        let api = RemoteApi::default();
        assert_eq!(api.provider().source(), SourceName::AniList);
    }

    #[test]
    fn movie_collection_defaults_to_anime_movie_format() {
        let col = RemoteApi::jikan().movie_metadata();
        assert_eq!(col.options().media_kind, Some(MediaKind::Anime));
        assert_eq!(col.options().format.as_deref(), Some("MOVIE"));
    }

    #[test]
    fn kitsu_provider_has_correct_source() {
        let api = RemoteApi::kitsu();
        assert_eq!(api.provider().source(), SourceName::Kitsu);
    }

    #[test]
    fn custom_provider_via_with_provider() {
        let api = RemoteApi::with_provider(TvmazeProvider::new());
        assert_eq!(api.provider().source(), SourceName::Tvmaze);
    }
}
