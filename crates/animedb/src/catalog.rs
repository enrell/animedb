//! Remote-first provider wrappers without local SQLite storage.
//!
//! [`RemoteCatalog`] and [`RemoteMetadataCollection`] expose provider functionality
//! without requiring the `local-db` feature or any SQLite dependency.

use crate::error::Result;
use crate::model::{CanonicalMedia, MediaKind, SearchOptions};
use crate::provider::{AniListProvider, Provider};

/// Remote-first facade over a single metadata provider.
///
/// Construct with a concrete provider:
///
/// ```ignore
/// let remote = RemoteCatalog::new(AniListProvider::default());
/// let results = remote.anime_metadata().search("monster")?;
/// ```
///
/// Or use the type-erasured form via [`RemoteApi`](crate::remote::RemoteApi)
/// when the provider variant is chosen dynamically.
pub struct RemoteCatalog<P = AniListProvider> {
    provider: P,
}

impl Default for RemoteCatalog<AniListProvider> {
    fn default() -> Self {
        Self::new(AniListProvider::default())
    }
}

impl<P: Provider> RemoteCatalog<P> {
    /// Creates a new remote catalog wrapping the given provider.
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    /// Returns a reference to the underlying provider for introspection.
    pub fn provider(&self) -> &P {
        &self.provider
    }

    /// Free-text search across the provider's full catalog.
    pub fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<CanonicalMedia>> {
        self.provider.search(query, options)
    }

    /// Returns a collection filtered to anime records.
    pub fn anime_metadata(&self) -> RemoteMetadataCollection<'_, P> {
        RemoteMetadataCollection::new(
            &self.provider,
            SearchOptions::default().with_media_kind(MediaKind::Anime),
        )
    }

    /// Returns a collection filtered to manga records.
    pub fn manga_metadata(&self) -> RemoteMetadataCollection<'_, P> {
        RemoteMetadataCollection::new(
            &self.provider,
            SearchOptions::default().with_media_kind(MediaKind::Manga),
        )
    }

    /// Returns a collection filtered to anime movies (`format = "MOVIE"`).
    pub fn movie_metadata(&self) -> RemoteMetadataCollection<'_, P> {
        RemoteMetadataCollection::new(
            &self.provider,
            SearchOptions::default()
                .with_media_kind(MediaKind::Anime)
                .with_format("MOVIE"),
        )
    }

    /// Returns a collection filtered to TV show records.
    pub fn show_metadata(&self) -> RemoteMetadataCollection<'_, P> {
        RemoteMetadataCollection::new(
            &self.provider,
            SearchOptions::default().with_media_kind(MediaKind::Show),
        )
    }

    /// Returns a collection filtered to movie records (IMDb-style).
    pub fn tv_movie_metadata(&self) -> RemoteMetadataCollection<'_, P> {
        RemoteMetadataCollection::new(
            &self.provider,
            SearchOptions::default().with_media_kind(MediaKind::Movie),
        )
    }
}

/// A filtered view over a provider's catalog for one media kind.
///
/// Obtain from [`RemoteCatalog::anime_metadata`], [`RemoteCatalog::manga_metadata`],
/// [`RemoteCatalog::movie_metadata`], etc.
pub struct RemoteMetadataCollection<'a, P> {
    provider: &'a P,
    options: SearchOptions,
}

impl<'a, P: Provider> RemoteMetadataCollection<'a, P> {
    fn new(provider: &'a P, options: SearchOptions) -> Self {
        Self { provider, options }
    }

    /// Returns the effective search options for this collection.
    pub fn options(&self) -> &SearchOptions {
        &self.options
    }

    /// Free-text search scoped to this collection's media kind.
    pub fn search(&self, query: &str) -> Result<Vec<CanonicalMedia>> {
        self.provider.search(query, self.options.clone())
    }

    /// Direct ID lookup (provider-native ID).
    ///
    /// Returns `None` if the ID exists but the format does not match the collection.
    pub fn by_id(&self, source_id: &str) -> Result<Option<CanonicalMedia>> {
        let media = self.provider.get_by_id(
            self.options.media_kind.unwrap_or(MediaKind::Anime),
            source_id,
        )?;

        Ok(media.filter(|item| {
            self.options
                .format
                .as_ref()
                .map(|format| {
                    item.format
                        .as_ref()
                        .map(|value| value.eq_ignore_ascii_case(format))
                        == Some(true)
                })
                .unwrap_or(true)
        }))
    }
}
