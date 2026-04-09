use crate::error::Result;
use crate::model::{CanonicalMedia, MediaKind, SearchOptions};
use crate::provider::{
    AniListProvider, JikanProvider, KitsuProvider, RemoteProvider, TvmazeProvider,
};

/// Remote providers supported by the simplified facade.
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

/// Remote-first facade over the supported metadata providers.
///
/// This type is intended for clients that want normalized metadata access without creating
/// or syncing a local SQLite catalog.
pub struct RemoteApi {
    source: RemoteSource,
}

impl Default for RemoteApi {
    fn default() -> Self {
        Self::anilist()
    }
}

impl RemoteApi {
    /// Creates a remote API facade for the selected provider.
    pub fn new(source: RemoteSource) -> Self {
        Self { source }
    }

    /// Creates an AniList facade.
    pub fn anilist() -> Self {
        Self::new(RemoteSource::AniList)
    }

    /// Creates a Jikan facade.
    pub fn jikan() -> Self {
        Self::new(RemoteSource::Jikan)
    }

    /// Creates a Kitsu facade.
    pub fn kitsu() -> Self {
        Self::new(RemoteSource::Kitsu)
    }

    /// Creates a TVmaze facade.
    pub fn tvmaze() -> Self {
        Self::new(RemoteSource::Tvmaze)
    }

    /// Creates an IMDb facade.
    pub fn imdb() -> Self {
        Self::new(RemoteSource::Imdb)
    }

    pub fn source(&self) -> RemoteSource {
        self.source
    }

    /// Searches the selected provider directly.
    pub fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<CanonicalMedia>> {
        match self.source {
            RemoteSource::AniList => AniListProvider::default().search(query, options),
            RemoteSource::Jikan => JikanProvider::default().search(query, options),
            RemoteSource::Kitsu => KitsuProvider::default().search(query, options),
            RemoteSource::Tvmaze => TvmazeProvider::default().search(query, options),
            RemoteSource::Imdb => Err(crate::error::Error::Validation(
                "IMDb remote search requires downloading the full dataset; use sync instead".into(),
            )),
        }
    }

    /// Narrows queries to anime records.
    pub fn anime_metadata(&self) -> RemoteCollection {
        RemoteCollection::new(
            self.source,
            SearchOptions::default().with_media_kind(MediaKind::Anime),
        )
    }

    /// Narrows queries to manga records.
    pub fn manga_metadata(&self) -> RemoteCollection {
        RemoteCollection::new(
            self.source,
            SearchOptions::default().with_media_kind(MediaKind::Manga),
        )
    }

    /// Narrows queries to anime movies.
    pub fn movie_metadata(&self) -> RemoteCollection {
        RemoteCollection::new(
            self.source,
            SearchOptions::default()
                .with_media_kind(MediaKind::Anime)
                .with_format("MOVIE"),
        )
    }
}

/// Filtered view over one remote provider and one media slice.
pub struct RemoteCollection {
    source: RemoteSource,
    options: SearchOptions,
}

impl RemoteCollection {
    fn new(source: RemoteSource, options: SearchOptions) -> Self {
        Self { source, options }
    }

    pub fn source(&self) -> RemoteSource {
        self.source
    }

    pub fn options(&self) -> &SearchOptions {
        &self.options
    }

    pub fn search(&self, query: &str) -> Result<Vec<CanonicalMedia>> {
        match self.source {
            RemoteSource::AniList => AniListProvider::default().search(query, self.options.clone()),
            RemoteSource::Jikan => JikanProvider::default().search(query, self.options.clone()),
            RemoteSource::Kitsu => KitsuProvider::default().search(query, self.options.clone()),
            RemoteSource::Tvmaze => TvmazeProvider::default().search(query, self.options.clone()),
            RemoteSource::Imdb => Err(crate::error::Error::Validation(
                "IMDb remote search requires downloading the full dataset; use sync instead".into(),
            )),
        }
    }

    pub fn by_id(&self, source_id: &str) -> Result<Option<CanonicalMedia>> {
        let media_kind = self.options.media_kind.unwrap_or(MediaKind::Anime);
        let item = match self.source {
            RemoteSource::AniList => AniListProvider::default().get_by_id(media_kind, source_id)?,
            RemoteSource::Jikan => JikanProvider::default().get_by_id(media_kind, source_id)?,
            RemoteSource::Kitsu => KitsuProvider::default().get_by_id(media_kind, source_id)?,
            RemoteSource::Tvmaze => TvmazeProvider::default().get_by_id(media_kind, source_id)?,
            RemoteSource::Imdb => {
                return Err(crate::error::Error::Validation(
                    "IMDb remote lookup requires downloading the full dataset; use sync instead"
                        .into(),
                ));
            }
        };

        Ok(item.filter(|media| {
            self.options
                .format
                .as_ref()
                .map(|format| {
                    media
                        .format
                        .as_ref()
                        .map(|value| value.eq_ignore_ascii_case(format))
                        == Some(true)
                })
                .unwrap_or(true)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_remote_api_uses_anilist() {
        let api = RemoteApi::default();
        assert_eq!(api.source(), RemoteSource::AniList);
    }

    #[test]
    fn movie_collection_is_anime_with_movie_format() {
        let collection = RemoteApi::jikan().movie_metadata();
        assert_eq!(collection.source(), RemoteSource::Jikan);
        assert_eq!(collection.options().media_kind, Some(MediaKind::Anime));
        assert_eq!(collection.options().format.as_deref(), Some("MOVIE"));
    }

    #[test]
    fn kitsu_constructor_uses_kitsu() {
        let api = RemoteApi::kitsu();
        assert_eq!(api.source(), RemoteSource::Kitsu);
    }
}
