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

use crate::error::{Error, Result};
use crate::merge::merge_canonical_episodes_by_effective_number;
use crate::model::{
    CanonicalEpisode, CanonicalMedia, ExternalId, MediaKind, SearchOptions, SourceName,
};
use crate::provider::{
    AniListProvider, ImdbProvider, JikanProvider, KitsuProvider, Provider, ProviderRegistry,
    TvmazeProvider, default_registry,
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

/// A provider-native episode lookup derived from a media record's external IDs.
///
/// `source` is the provider that should be queried. For example, a
/// [`SourceName::MyAnimeList`] ID is queried through the Jikan provider because
/// MyAnimeList does not have a built-in provider client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeFetchCandidate {
    pub source: SourceName,
    pub source_id: String,
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

    /// Fetches episode metadata directly from this provider.
    pub fn fetch_episodes(
        &self,
        media_kind: MediaKind,
        source_id: &str,
    ) -> Result<Vec<CanonicalEpisode>> {
        self.provider.fetch_episodes(media_kind, source_id)
    }

    /// Builds the episode-capable provider attempts for a merged media record.
    ///
    /// The returned order reflects the preferred episode sources for each media kind:
    /// Jikan/MyAnimeList and Kitsu for anime, TVmaze for shows. Providers without a
    /// single-media episode endpoint, such as AniList and IMDb, are intentionally skipped.
    pub fn episode_fetch_candidates(
        media_kind: MediaKind,
        external_ids: &[ExternalId],
    ) -> Vec<EpisodeFetchCandidate> {
        let mut candidates = Vec::new();

        fn push_unique(
            candidates: &mut Vec<EpisodeFetchCandidate>,
            source: SourceName,
            source_id: &str,
        ) {
            if !candidates
                .iter()
                .any(|item| item.source == source && item.source_id == source_id)
            {
                candidates.push(EpisodeFetchCandidate {
                    source,
                    source_id: source_id.to_string(),
                });
            }
        }

        for external_id in external_ids {
            match (media_kind, external_id.source) {
                (MediaKind::Anime, SourceName::Jikan | SourceName::MyAnimeList) => {
                    push_unique(&mut candidates, SourceName::Jikan, &external_id.source_id);
                }
                (MediaKind::Anime, SourceName::Kitsu) => {
                    push_unique(&mut candidates, SourceName::Kitsu, &external_id.source_id);
                }
                (MediaKind::Show, SourceName::Tvmaze) => {
                    push_unique(&mut candidates, SourceName::Tvmaze, &external_id.source_id);
                }
                _ => {}
            }
        }

        candidates
    }

    /// Fetches episode metadata from every episode-capable source ID on a media record.
    ///
    /// Successful provider responses are concatenated as provider-normalized
    /// [`CanonicalEpisode`] records. Callers that use local SQLite should persist these as
    /// source records and run the episode merge engine to produce [`crate::StoredEpisode`] values.
    pub fn fetch_episodes_from_external_ids(
        media_kind: MediaKind,
        external_ids: &[ExternalId],
    ) -> Result<Vec<CanonicalEpisode>> {
        let candidates = Self::episode_fetch_candidates(media_kind, external_ids);
        Self::fetch_episodes_from_candidates(media_kind, &candidates)
    }

    /// Fetches and merges episode metadata from every episode-capable source ID.
    ///
    /// This is the remote-only path for applications that do not want animedb's local SQLite
    /// episode repository. Provider responses are aggregated, grouped by flat effective episode
    /// number (`absolute_number.or(episode_number)`), and merged field-by-field using animedb's
    /// episode provider priority.
    pub fn fetch_merged_episodes_from_external_ids(
        media_kind: MediaKind,
        external_ids: &[ExternalId],
    ) -> Result<Vec<CanonicalEpisode>> {
        let episodes = Self::fetch_episodes_from_external_ids(media_kind, external_ids)?;
        Ok(merge_canonical_episodes_by_effective_number(&episodes))
    }

    /// Fetches episode metadata from explicit provider-native candidates using the built-in registry.
    pub fn fetch_episodes_from_candidates(
        media_kind: MediaKind,
        candidates: &[EpisodeFetchCandidate],
    ) -> Result<Vec<CanonicalEpisode>> {
        let registry = default_registry();
        Self::fetch_episodes_from_candidates_with_registry(media_kind, candidates, &registry)
    }

    /// Fetches and merges episode metadata from explicit provider-native candidates.
    pub fn fetch_merged_episodes_from_candidates(
        media_kind: MediaKind,
        candidates: &[EpisodeFetchCandidate],
    ) -> Result<Vec<CanonicalEpisode>> {
        let episodes = Self::fetch_episodes_from_candidates(media_kind, candidates)?;
        Ok(merge_canonical_episodes_by_effective_number(&episodes))
    }

    /// Fetches episode metadata from explicit candidates using a supplied provider registry.
    ///
    /// This is useful for tests and for applications that replace built-in providers with
    /// authenticated or proxied provider instances.
    pub fn fetch_episodes_from_candidates_with_registry(
        media_kind: MediaKind,
        candidates: &[EpisodeFetchCandidate],
        registry: &ProviderRegistry,
    ) -> Result<Vec<CanonicalEpisode>> {
        let mut episodes = Vec::new();
        let mut failures = Vec::new();

        for candidate in candidates {
            match registry
                .get(candidate.source)
                .and_then(|provider| provider.fetch_episodes(media_kind, &candidate.source_id))
            {
                Ok(mut fetched) => episodes.append(&mut fetched),
                Err(err) => failures.push(format!(
                    "{}:{} failed: {err}",
                    candidate.source, candidate.source_id
                )),
            }
        }

        if episodes.is_empty() && !failures.is_empty() {
            return Err(Error::Validation(format!(
                "episode aggregation failed for all candidates: {}",
                failures.join("; ")
            )));
        }

        Ok(episodes)
    }

    /// Fetches and merges episode metadata from explicit candidates using a supplied registry.
    ///
    /// This is useful for tests and for applications that inject authenticated or proxied
    /// providers but still want animedb's remote-only episode merge behavior.
    pub fn fetch_merged_episodes_from_candidates_with_registry(
        media_kind: MediaKind,
        candidates: &[EpisodeFetchCandidate],
        registry: &ProviderRegistry,
    ) -> Result<Vec<CanonicalEpisode>> {
        let episodes =
            Self::fetch_episodes_from_candidates_with_registry(media_kind, candidates, registry)?;
        Ok(merge_canonical_episodes_by_effective_number(&episodes))
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

    /// Narrows queries to TV show records.
    pub fn show_metadata(&self) -> RemoteCollection {
        RemoteCollection::new(
            Arc::clone(&self.provider),
            SearchOptions::default().with_media_kind(MediaKind::Show),
        )
    }

    /// Narrows queries to movie records.
    pub fn tv_movie_metadata(&self) -> RemoteCollection {
        RemoteCollection::new(
            Arc::clone(&self.provider),
            SearchOptions::default().with_media_kind(MediaKind::Movie),
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

    pub fn episodes(&self, source_id: &str) -> Result<Vec<CanonicalEpisode>> {
        let kind = self.options.media_kind.unwrap_or(MediaKind::Anime);
        self.provider.fetch_episodes(kind, source_id)
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
    use crate::model::{CanonicalEpisode, SyncCursor, SyncRequest};
    use crate::provider::FetchPage;
    use std::sync::Mutex;

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

    #[test]
    fn episode_fetch_candidates_map_external_ids_to_episode_providers() {
        let external_ids = vec![
            ExternalId {
                source: SourceName::AniList,
                source_id: "1".into(),
                url: None,
            },
            ExternalId {
                source: SourceName::MyAnimeList,
                source_id: "19".into(),
                url: None,
            },
            ExternalId {
                source: SourceName::Jikan,
                source_id: "19".into(),
                url: None,
            },
            ExternalId {
                source: SourceName::Kitsu,
                source_id: "42".into(),
                url: None,
            },
        ];

        let candidates = RemoteApi::episode_fetch_candidates(MediaKind::Anime, &external_ids);

        assert_eq!(
            candidates,
            vec![
                EpisodeFetchCandidate {
                    source: SourceName::Jikan,
                    source_id: "19".into()
                },
                EpisodeFetchCandidate {
                    source: SourceName::Kitsu,
                    source_id: "42".into()
                }
            ]
        );
    }

    #[test]
    fn episode_aggregation_collects_successes_and_ignores_failed_candidates() {
        struct FakeProvider {
            source: SourceName,
            calls: Mutex<Vec<String>>,
        }

        impl Provider for FakeProvider {
            fn source(&self) -> SourceName {
                self.source
            }

            fn fetch_page(&self, _request: &SyncRequest, _cursor: SyncCursor) -> Result<FetchPage> {
                Ok(FetchPage {
                    items: Vec::new(),
                    next_cursor: None,
                })
            }

            fn search(&self, _query: &str, _options: SearchOptions) -> Result<Vec<CanonicalMedia>> {
                Ok(Vec::new())
            }

            fn get_by_id(
                &self,
                _media_kind: MediaKind,
                _source_id: &str,
            ) -> Result<Option<CanonicalMedia>> {
                Ok(None)
            }

            fn fetch_episodes(
                &self,
                media_kind: MediaKind,
                source_id: &str,
            ) -> Result<Vec<CanonicalEpisode>> {
                self.calls.lock().unwrap().push(source_id.to_string());
                if self.source == SourceName::Kitsu {
                    return Err(Error::Validation("test failure".into()));
                }
                Ok(vec![CanonicalEpisode {
                    source: self.source,
                    source_id: format!("ep-{source_id}"),
                    media_kind,
                    season_number: Some(1),
                    episode_number: Some(1),
                    absolute_number: Some(1),
                    title_display: Some("Episode 1".into()),
                    title_original: None,
                    synopsis: None,
                    air_date: None,
                    runtime_minutes: None,
                    thumbnail_url: None,
                    raw_titles_json: None,
                    raw_json: None,
                }])
            }
        }

        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(FakeProvider {
            source: SourceName::Jikan,
            calls: Mutex::new(Vec::new()),
        }));
        registry.register(Arc::new(FakeProvider {
            source: SourceName::Kitsu,
            calls: Mutex::new(Vec::new()),
        }));

        let candidates = vec![
            EpisodeFetchCandidate {
                source: SourceName::Jikan,
                source_id: "19".into(),
            },
            EpisodeFetchCandidate {
                source: SourceName::Kitsu,
                source_id: "42".into(),
            },
        ];

        let episodes = RemoteApi::fetch_episodes_from_candidates_with_registry(
            MediaKind::Anime,
            &candidates,
            &registry,
        )
        .unwrap();

        assert_eq!(episodes.len(), 1);
        assert_eq!(episodes[0].source, SourceName::Jikan);
    }

    #[test]
    fn merged_episode_fetch_deduplicates_provider_results() {
        struct FakeProvider {
            source: SourceName,
        }

        impl Provider for FakeProvider {
            fn source(&self) -> SourceName {
                self.source
            }

            fn fetch_page(&self, _request: &SyncRequest, _cursor: SyncCursor) -> Result<FetchPage> {
                Ok(FetchPage {
                    items: Vec::new(),
                    next_cursor: None,
                })
            }

            fn search(&self, _query: &str, _options: SearchOptions) -> Result<Vec<CanonicalMedia>> {
                Ok(Vec::new())
            }

            fn get_by_id(
                &self,
                _media_kind: MediaKind,
                _source_id: &str,
            ) -> Result<Option<CanonicalMedia>> {
                Ok(None)
            }

            fn fetch_episodes(
                &self,
                media_kind: MediaKind,
                source_id: &str,
            ) -> Result<Vec<CanonicalEpisode>> {
                let mut episode = CanonicalEpisode {
                    source: self.source,
                    source_id: format!("ep-{source_id}"),
                    media_kind,
                    season_number: Some(1),
                    episode_number: Some(1),
                    absolute_number: Some(1),
                    title_display: Some(format!("{} title", self.source)),
                    title_original: None,
                    synopsis: None,
                    air_date: None,
                    runtime_minutes: None,
                    thumbnail_url: None,
                    raw_titles_json: None,
                    raw_json: None,
                };

                if self.source == SourceName::Kitsu {
                    episode.runtime_minutes = Some(24);
                }

                Ok(vec![episode])
            }
        }

        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(FakeProvider {
            source: SourceName::Jikan,
        }));
        registry.register(Arc::new(FakeProvider {
            source: SourceName::Kitsu,
        }));

        let candidates = vec![
            EpisodeFetchCandidate {
                source: SourceName::Jikan,
                source_id: "19".into(),
            },
            EpisodeFetchCandidate {
                source: SourceName::Kitsu,
                source_id: "42".into(),
            },
        ];

        let episodes = RemoteApi::fetch_merged_episodes_from_candidates_with_registry(
            MediaKind::Anime,
            &candidates,
            &registry,
        )
        .unwrap();

        assert_eq!(episodes.len(), 1);
        assert_eq!(episodes[0].source, SourceName::Jikan);
        assert_eq!(episodes[0].title_display.as_deref(), Some("jikan title"));
        assert_eq!(episodes[0].runtime_minutes, Some(24));
    }
}
