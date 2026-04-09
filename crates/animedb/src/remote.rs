use crate::error::Result;
use crate::model::{CanonicalMedia, MediaKind, SearchOptions};
use crate::provider::{AniListProvider, JikanProvider, KitsuProvider, RemoteProvider};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteSource {
    AniList,
    Jikan,
    Kitsu,
}

impl RemoteSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AniList => "anilist",
            Self::Jikan => "jikan",
            Self::Kitsu => "kitsu",
        }
    }
}

pub struct RemoteApi {
    source: RemoteSource,
}

impl Default for RemoteApi {
    fn default() -> Self {
        Self::anilist()
    }
}

impl RemoteApi {
    pub fn new(source: RemoteSource) -> Self {
        Self { source }
    }

    pub fn anilist() -> Self {
        Self::new(RemoteSource::AniList)
    }

    pub fn jikan() -> Self {
        Self::new(RemoteSource::Jikan)
    }

    pub fn kitsu() -> Self {
        Self::new(RemoteSource::Kitsu)
    }

    pub fn source(&self) -> RemoteSource {
        self.source
    }

    pub fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<CanonicalMedia>> {
        match self.source {
            RemoteSource::AniList => AniListProvider::default().search(query, options),
            RemoteSource::Jikan => JikanProvider::default().search(query, options),
            RemoteSource::Kitsu => KitsuProvider::default().search(query, options),
        }
    }

    pub fn anime_metadata(&self) -> RemoteCollection {
        RemoteCollection::new(
            self.source,
            SearchOptions::default().with_media_kind(MediaKind::Anime),
        )
    }

    pub fn manga_metadata(&self) -> RemoteCollection {
        RemoteCollection::new(
            self.source,
            SearchOptions::default().with_media_kind(MediaKind::Manga),
        )
    }

    pub fn movie_metadata(&self) -> RemoteCollection {
        RemoteCollection::new(
            self.source,
            SearchOptions::default()
                .with_media_kind(MediaKind::Anime)
                .with_format("MOVIE"),
        )
    }
}

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
        }
    }

    pub fn by_id(&self, source_id: &str) -> Result<Option<CanonicalMedia>> {
        let media_kind = self.options.media_kind.unwrap_or(MediaKind::Anime);
        let item = match self.source {
            RemoteSource::AniList => AniListProvider::default().get_by_id(media_kind, source_id)?,
            RemoteSource::Jikan => JikanProvider::default().get_by_id(media_kind, source_id)?,
            RemoteSource::Kitsu => KitsuProvider::default().get_by_id(media_kind, source_id)?,
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
