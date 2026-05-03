use crate::error::Result;
use crate::model::{CanonicalMedia, MediaKind, SearchOptions};
use crate::provider::{AniListProvider, Provider};

pub struct RemoteCatalog<P = AniListProvider> {
    provider: P,
}

impl Default for RemoteCatalog<AniListProvider> {
    fn default() -> Self {
        Self::new(AniListProvider::default())
    }
}

impl<P: Provider> RemoteCatalog<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    pub fn provider(&self) -> &P {
        &self.provider
    }

    pub fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<CanonicalMedia>> {
        self.provider.search(query, options)
    }

    pub fn anime_metadata(&self) -> RemoteMetadataCollection<'_, P> {
        RemoteMetadataCollection::new(
            &self.provider,
            SearchOptions::default().with_media_kind(MediaKind::Anime),
        )
    }

    pub fn manga_metadata(&self) -> RemoteMetadataCollection<'_, P> {
        RemoteMetadataCollection::new(
            &self.provider,
            SearchOptions::default().with_media_kind(MediaKind::Manga),
        )
    }

    pub fn movie_metadata(&self) -> RemoteMetadataCollection<'_, P> {
        RemoteMetadataCollection::new(
            &self.provider,
            SearchOptions::default()
                .with_media_kind(MediaKind::Anime)
                .with_format("MOVIE"),
        )
    }

    pub fn show_metadata(&self) -> RemoteMetadataCollection<'_, P> {
        RemoteMetadataCollection::new(
            &self.provider,
            SearchOptions::default().with_media_kind(MediaKind::Show),
        )
    }

    pub fn tv_movie_metadata(&self) -> RemoteMetadataCollection<'_, P> {
        RemoteMetadataCollection::new(
            &self.provider,
            SearchOptions::default().with_media_kind(MediaKind::Movie),
        )
    }
}

pub struct RemoteMetadataCollection<'a, P> {
    provider: &'a P,
    options: SearchOptions,
}

impl<'a, P: Provider> RemoteMetadataCollection<'a, P> {
    fn new(provider: &'a P, options: SearchOptions) -> Self {
        Self { provider, options }
    }

    pub fn search(&self, query: &str) -> Result<Vec<CanonicalMedia>> {
        self.provider.search(query, self.options.clone())
    }

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
