use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::io::BufRead;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::model::{
    CanonicalMedia, ExternalId, MediaKind, SearchOptions, SourceName, SourcePayload, SyncCursor,
    SyncRequest,
};

#[derive(Debug, Clone)]
pub struct RemotePage {
    pub items: Vec<CanonicalMedia>,
    pub next_cursor: Option<SyncCursor>,
}

pub trait RemoteProvider {
    fn source(&self) -> SourceName;

    fn min_interval(&self) -> Duration {
        Duration::ZERO
    }

    fn fetch_page(&self, request: &SyncRequest, cursor: SyncCursor) -> Result<RemotePage>;

    fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<CanonicalMedia>>;

    fn get_by_id(&self, media_kind: MediaKind, source_id: &str) -> Result<Option<CanonicalMedia>>;
}

#[derive(Debug, Clone)]
pub struct AniListProvider {
    client: Client,
    endpoint: String,
}

#[derive(Debug, Clone)]
pub struct JikanProvider {
    client: Client,
    endpoint: String,
}

#[derive(Debug, Clone)]
pub struct KitsuProvider {
    client: Client,
    endpoint: String,
}

impl Default for AniListProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AniListProvider {
    pub const DEFAULT_ENDPOINT: &'static str = "https://graphql.anilist.co";

    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("animedb/0.1")
            .build()
            .expect("reqwest blocking client should build");

        Self {
            client,
            endpoint: Self::DEFAULT_ENDPOINT.to_string(),
        }
    }

    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        let mut provider = Self::new();
        provider.endpoint = endpoint.into();
        provider
    }
}

impl Default for JikanProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl JikanProvider {
    pub const DEFAULT_ENDPOINT: &'static str = "https://api.jikan.moe/v4";

    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("animedb/0.1")
            .build()
            .expect("reqwest blocking client should build");

        Self {
            client,
            endpoint: Self::DEFAULT_ENDPOINT.to_string(),
        }
    }

    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        let mut provider = Self::new();
        provider.endpoint = endpoint.into();
        provider
    }
}

impl Default for KitsuProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KitsuProvider {
    pub const DEFAULT_ENDPOINT: &'static str = "https://kitsu.io/api/edge";

    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("animedb/0.1")
            .build()
            .expect("reqwest blocking client should build");

        Self {
            client,
            endpoint: Self::DEFAULT_ENDPOINT.to_string(),
        }
    }

    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        let mut provider = Self::new();
        provider.endpoint = endpoint.into();
        provider
    }
}

impl RemoteProvider for AniListProvider {
    fn source(&self) -> SourceName {
        SourceName::AniList
    }

    fn min_interval(&self) -> Duration {
        Duration::from_millis(700)
    }

    fn fetch_page(&self, request: &SyncRequest, cursor: SyncCursor) -> Result<RemotePage> {
        let media_kind = request.media_kind.unwrap_or(MediaKind::Anime);
        let page_size = request.page_size.clamp(1, 50);

        let payload = json!({
            "query": ANILIST_PAGE_QUERY,
            "variables": {
                "page": cursor.page as i64,
                "perPage": page_size as i64,
                "type": anilist_kind(media_kind),
                "sort": ["ID"]
            }
        });

        let response = self
            .client
            .post(&self.endpoint)
            .json(&payload)
            .send()?
            .error_for_status()?
            .json::<AniListGraphQlResponse>()?;

        if !response.errors.is_empty() {
            return Err(Error::Validation(format!(
                "AniList returned errors: {}",
                response
                    .errors
                    .iter()
                    .map(|item| item.message.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }

        let page = response
            .data
            .ok_or_else(|| Error::Validation("AniList response missing data".into()))?
            .page;

        let items = page
            .media
            .into_iter()
            .map(|item| map_anilist_media(item, media_kind))
            .collect::<Result<Vec<_>>>()?;

        let next_cursor = page.page_info.has_next_page.then_some(SyncCursor {
            page: cursor.page + 1,
        });

        Ok(RemotePage { items, next_cursor })
    }

    fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<CanonicalMedia>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let media_kind = options.media_kind.unwrap_or(MediaKind::Anime);
        let limit = options.limit.clamp(1, 50);

        let payload = json!({
            "query": ANILIST_SEARCH_QUERY,
            "variables": {
                "page": 1,
                "perPage": limit as i64,
                "type": anilist_kind(media_kind),
                "search": query,
            }
        });

        let response = self
            .client
            .post(&self.endpoint)
            .json(&payload)
            .send()?
            .error_for_status()?
            .json::<AniListGraphQlResponse>()?;

        if !response.errors.is_empty() {
            return Err(Error::Validation(format!(
                "AniList returned errors: {}",
                response
                    .errors
                    .iter()
                    .map(|item| item.message.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }

        let page = response
            .data
            .ok_or_else(|| Error::Validation("AniList response missing data".into()))?
            .page;

        let mut media = page
            .media
            .into_iter()
            .map(|item| map_anilist_media(item, media_kind))
            .collect::<Result<Vec<_>>>()?;

        if let Some(format) = options.format {
            media.retain(|item| {
                item.format
                    .as_ref()
                    .map(|value| value.eq_ignore_ascii_case(&format))
                    == Some(true)
            });
        }

        Ok(media)
    }

    fn get_by_id(&self, media_kind: MediaKind, source_id: &str) -> Result<Option<CanonicalMedia>> {
        let media_id: i64 = source_id
            .parse()
            .map_err(|_| Error::Validation(format!("invalid AniList id: {source_id}")))?;

        let payload = json!({
            "query": ANILIST_BY_ID_QUERY,
            "variables": {
                "id": media_id,
                "type": anilist_kind(media_kind),
            }
        });

        let response = self
            .client
            .post(&self.endpoint)
            .json(&payload)
            .send()?
            .error_for_status()?
            .json::<AniListSingleMediaResponse>()?;

        if !response.errors.is_empty() {
            return Err(Error::Validation(format!(
                "AniList returned errors: {}",
                response
                    .errors
                    .iter()
                    .map(|item| item.message.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }

        let Some(data) = response.data else {
            return Ok(None);
        };
        let Some(media) = data.media else {
            return Ok(None);
        };

        Ok(Some(map_anilist_media(media, media_kind)?))
    }
}

fn anilist_kind(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Anime => "ANIME",
        MediaKind::Manga => "MANGA",
        MediaKind::Show | MediaKind::Movie => "ANIME",
    }
}

fn map_anilist_media(item: AniListMedia, media_kind: MediaKind) -> Result<CanonicalMedia> {
    let raw_json = serde_json::to_value(&item)?;
    let mut external_ids = vec![ExternalId {
        source: SourceName::AniList,
        source_id: item.id.to_string(),
        url: item.site_url.clone(),
    }];

    if let Some(id_mal) = item.id_mal {
        external_ids.push(ExternalId {
            source: SourceName::MyAnimeList,
            source_id: id_mal.to_string(),
            url: None,
        });
    }

    let title_display = item
        .title
        .english
        .clone()
        .or(item.title.romaji.clone())
        .or(item.title.native.clone())
        .ok_or_else(|| Error::Validation(format!("AniList media {} has no title", item.id)))?;

    Ok(CanonicalMedia {
        media_kind,
        title_display,
        title_romaji: item.title.romaji.clone(),
        title_english: item.title.english.clone(),
        title_native: item.title.native.clone(),
        synopsis: item.description.clone(),
        format: item.format.clone(),
        status: item.status.clone(),
        season: item.season.map(|season| season.to_ascii_lowercase()),
        season_year: item.season_year,
        episodes: item.episodes,
        chapters: item.chapters,
        volumes: item.volumes,
        country_of_origin: item.country_of_origin.clone(),
        cover_image: item.cover_image.as_ref().and_then(|cover| {
            cover
                .extra_large
                .clone()
                .or(cover.large.clone())
                .or(cover.medium.clone())
        }),
        banner_image: item.banner_image.clone(),
        provider_rating: item
            .average_score
            .map(|value| (value / 100.0).clamp(0.0, 1.0)),
        nsfw: item.is_adult.unwrap_or(false),
        aliases: item.synonyms.clone(),
        genres: item.genres.clone(),
        tags: item.tags.iter().map(|tag| tag.name.clone()).collect(),
        external_ids,
        source_payloads: vec![SourcePayload {
            source: SourceName::AniList,
            source_id: item.id.to_string(),
            url: item.site_url.clone(),
            remote_updated_at: item.updated_at.map(|value| value.to_string()),
            raw_json: Some(raw_json),
        }],
        field_provenance: Vec::new(),
    })
}

impl RemoteProvider for JikanProvider {
    fn source(&self) -> SourceName {
        SourceName::Jikan
    }

    fn min_interval(&self) -> Duration {
        Duration::from_millis(1_100)
    }

    fn fetch_page(&self, request: &SyncRequest, cursor: SyncCursor) -> Result<RemotePage> {
        let media_kind = request.media_kind.unwrap_or(MediaKind::Anime);
        let page_size = request.page_size.clamp(1, 25);
        let path = match media_kind {
            MediaKind::Anime | MediaKind::Show => "anime",
            MediaKind::Manga | MediaKind::Movie => "manga",
        };

        let response = self
            .client
            .get(format!("{}/{path}", self.endpoint))
            .query(&[
                ("page", cursor.page.to_string()),
                ("limit", page_size.to_string()),
                ("sfw", "false".to_string()),
            ])
            .send()?
            .error_for_status()?
            .json::<JikanListResponse>()?;

        let items = response
            .data
            .into_iter()
            .map(|item| map_jikan_media(item, media_kind))
            .collect::<Result<Vec<_>>>()?;

        let next_cursor = response.pagination.and_then(|pagination| {
            pagination
                .has_next_page
                .unwrap_or(false)
                .then_some(SyncCursor {
                    page: cursor.page + 1,
                })
        });

        Ok(RemotePage { items, next_cursor })
    }

    fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<CanonicalMedia>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let media_kind = options.media_kind.unwrap_or(MediaKind::Anime);
        let limit = options.limit.clamp(1, 25);
        let path = match media_kind {
            MediaKind::Anime | MediaKind::Show => "anime",
            MediaKind::Manga | MediaKind::Movie => "manga",
        };

        let response = self
            .client
            .get(format!("{}/{path}", self.endpoint))
            .query(&[
                ("q", query.to_string()),
                ("limit", limit.to_string()),
                ("sfw", "false".to_string()),
            ])
            .send()?
            .error_for_status()?
            .json::<JikanListResponse>()?;

        let mut items = response
            .data
            .into_iter()
            .map(|item| map_jikan_media(item, media_kind))
            .collect::<Result<Vec<_>>>()?;

        if let Some(format) = options.format {
            items.retain(|item| {
                item.format
                    .as_ref()
                    .map(|value| value.eq_ignore_ascii_case(&format))
                    == Some(true)
            });
        }

        Ok(items)
    }

    fn get_by_id(&self, media_kind: MediaKind, source_id: &str) -> Result<Option<CanonicalMedia>> {
        let mal_id: i64 = source_id
            .parse()
            .map_err(|_| Error::Validation(format!("invalid Jikan/MAL id: {source_id}")))?;
        let path = match media_kind {
            MediaKind::Anime | MediaKind::Show => "anime",
            MediaKind::Manga | MediaKind::Movie => "manga",
        };

        let response = self
            .client
            .get(format!("{}/{path}/{mal_id}", self.endpoint))
            .send()?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let response = response.error_for_status()?.json::<JikanItemResponse>()?;
        let Some(item) = response.data else {
            return Ok(None);
        };

        Ok(Some(map_jikan_media(item, media_kind)?))
    }
}

impl RemoteProvider for KitsuProvider {
    fn source(&self) -> SourceName {
        SourceName::Kitsu
    }

    fn min_interval(&self) -> Duration {
        Duration::from_millis(900)
    }

    fn fetch_page(&self, request: &SyncRequest, cursor: SyncCursor) -> Result<RemotePage> {
        let media_kind = request.media_kind.unwrap_or(MediaKind::Anime);
        let page_size = request.page_size.clamp(1, 20);
        let offset = cursor.page.saturating_sub(1) * page_size;
        let path = kitsu_kind_path(media_kind);

        let response = self
            .client
            .get(format!("{}/{path}", self.endpoint))
            .header("Accept", "application/vnd.api+json")
            .query(&[
                ("page[limit]", page_size.to_string()),
                ("page[offset]", offset.to_string()),
                ("sort", "id".to_string()),
                ("include", "categories,mappings".to_string()),
            ])
            .send()?
            .error_for_status()?
            .json::<KitsuCollectionResponse>()?;

        let items = response
            .data
            .iter()
            .map(|item| map_kitsu_media(item, &response.included, media_kind))
            .collect::<Result<Vec<_>>>()?;

        let next_cursor = response.links.next.as_ref().map(|_| SyncCursor {
            page: cursor.page + 1,
        });

        Ok(RemotePage { items, next_cursor })
    }

    fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<CanonicalMedia>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let media_kind = options.media_kind.unwrap_or(MediaKind::Anime);
        let limit = options.limit.clamp(1, 20);
        let path = kitsu_kind_path(media_kind);

        let response = self
            .client
            .get(format!("{}/{path}", self.endpoint))
            .header("Accept", "application/vnd.api+json")
            .query(&[
                ("filter[text]", query.to_string()),
                ("page[limit]", limit.to_string()),
                ("page[offset]", options.offset.to_string()),
                ("include", "categories,mappings".to_string()),
            ])
            .send()?
            .error_for_status()?
            .json::<KitsuCollectionResponse>()?;

        let mut items = response
            .data
            .iter()
            .map(|item| map_kitsu_media(item, &response.included, media_kind))
            .collect::<Result<Vec<_>>>()?;

        if let Some(format) = options.format {
            items.retain(|item| {
                item.format
                    .as_ref()
                    .map(|value| value.eq_ignore_ascii_case(&format))
                    == Some(true)
            });
        }

        Ok(items)
    }

    fn get_by_id(&self, media_kind: MediaKind, source_id: &str) -> Result<Option<CanonicalMedia>> {
        let path = kitsu_kind_path(media_kind);
        let response = self
            .client
            .get(format!("{}/{path}/{source_id}", self.endpoint))
            .header("Accept", "application/vnd.api+json")
            .query(&[("include", "categories,mappings")])
            .send()?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let response = response.error_for_status()?.json::<KitsuItemResponse>()?;
        let Some(item) = response.data else {
            return Ok(None);
        };

        Ok(Some(map_kitsu_media(
            &item,
            &response.included,
            media_kind,
        )?))
    }
}

fn map_jikan_media(item: JikanMedia, media_kind: MediaKind) -> Result<CanonicalMedia> {
    let raw_json = serde_json::to_value(&item)?;
    let title_display = item
        .title_english
        .clone()
        .or(item.title.clone())
        .or(item.title_japanese.clone())
        .ok_or_else(|| Error::Validation(format!("Jikan media {} has no title", item.mal_id)))?;

    let mut aliases = item
        .titles
        .iter()
        .filter_map(|title| title.title.clone())
        .collect::<Vec<_>>();
    aliases.extend(item.title_synonyms.clone());
    if let Some(default_title) = item.title.clone() {
        aliases.push(default_title);
    }

    let external_ids = vec![
        ExternalId {
            source: SourceName::Jikan,
            source_id: item.mal_id.to_string(),
            url: item.url.clone(),
        },
        ExternalId {
            source: SourceName::MyAnimeList,
            source_id: item.mal_id.to_string(),
            url: item.url.clone(),
        },
    ];

    Ok(CanonicalMedia {
        media_kind,
        title_display,
        title_romaji: item.title.clone(),
        title_english: item.title_english.clone(),
        title_native: item.title_japanese.clone(),
        synopsis: item.synopsis.clone(),
        format: item.media_type.clone(),
        status: item.status.clone(),
        season: item.season.clone(),
        season_year: item.year,
        episodes: item.episodes,
        chapters: item.chapters,
        volumes: item.volumes,
        country_of_origin: None,
        cover_image: item
            .images
            .jpg
            .large_image_url
            .clone()
            .or(item.images.jpg.image_url.clone()),
        banner_image: item
            .trailer
            .as_ref()
            .and_then(|trailer| trailer.images.maximum_image_url.clone()),
        provider_rating: item.score.map(|value| (value / 10.0).clamp(0.0, 1.0)),
        nsfw: jikan_is_nsfw(item.rating.as_deref()),
        aliases,
        genres: item.genres.into_iter().map(|genre| genre.name).collect(),
        tags: item
            .themes
            .into_iter()
            .chain(item.demographics.into_iter())
            .map(|item| item.name)
            .collect(),
        external_ids,
        source_payloads: vec![SourcePayload {
            source: SourceName::Jikan,
            source_id: item.mal_id.to_string(),
            url: item.url.clone(),
            remote_updated_at: item
                .updated_at
                .as_ref()
                .and_then(|value| value.from.clone()),
            raw_json: Some(raw_json),
        }],
        field_provenance: Vec::new(),
    })
}

fn map_kitsu_media(
    item: &KitsuResource,
    included: &[KitsuIncluded],
    media_kind: MediaKind,
) -> Result<CanonicalMedia> {
    let raw_json = serde_json::to_value(item)?;
    let attributes = &item.attributes;
    let title_display = attributes
        .canonical_title
        .clone()
        .or(attributes.titles.en.clone())
        .or(attributes.titles.en_jp.clone())
        .or(attributes.titles.ja_jp.clone())
        .ok_or_else(|| Error::Validation(format!("Kitsu media {} has no title", item.id)))?;

    let mut aliases = attributes.abbreviated_titles.clone();
    if let Some(en) = &attributes.titles.en {
        aliases.push(en.clone());
    }
    if let Some(en_jp) = &attributes.titles.en_jp {
        aliases.push(en_jp.clone());
    }
    if let Some(ja_jp) = &attributes.titles.ja_jp {
        aliases.push(ja_jp.clone());
    }

    let categories = kitsu_categories(item, included);
    let mappings = kitsu_mappings(item, included);
    let mut external_ids = vec![ExternalId {
        source: SourceName::Kitsu,
        source_id: item.id.clone(),
        url: Some(format!(
            "https://kitsu.io/{}/{}",
            kitsu_kind_path(media_kind),
            item.id
        )),
    }];
    for mapping in mappings {
        if let Some(external_id) = mapping.external_id {
            let site = mapping.external_site.to_ascii_lowercase();
            if site.contains("myanimelist") {
                external_ids.push(ExternalId {
                    source: SourceName::MyAnimeList,
                    source_id: external_id,
                    url: None,
                });
            }
        }
    }

    Ok(CanonicalMedia {
        media_kind,
        title_display,
        title_romaji: attributes.titles.en_jp.clone(),
        title_english: attributes.titles.en.clone(),
        title_native: attributes.titles.ja_jp.clone(),
        synopsis: attributes
            .synopsis
            .clone()
            .or(attributes.description.clone()),
        format: attributes.subtype.clone(),
        status: attributes.status.clone(),
        season: None,
        season_year: attributes.start_date.as_deref().and_then(parse_kitsu_year),
        episodes: attributes.episode_count,
        chapters: attributes.chapter_count,
        volumes: attributes.volume_count,
        country_of_origin: None,
        cover_image: attributes
            .poster_image
            .as_ref()
            .and_then(prefer_kitsu_image),
        banner_image: attributes.cover_image.as_ref().and_then(prefer_kitsu_image),
        provider_rating: attributes
            .average_rating
            .as_deref()
            .and_then(|value| value.parse::<f64>().ok())
            .map(|value| (value / 100.0).clamp(0.0, 1.0)),
        nsfw: attributes.nsfw.unwrap_or(false)
            || matches!(attributes.age_rating.as_deref(), Some("R18")),
        aliases,
        genres: Vec::new(),
        tags: categories,
        external_ids,
        source_payloads: vec![SourcePayload {
            source: SourceName::Kitsu,
            source_id: item.id.clone(),
            url: Some(format!(
                "https://kitsu.io/{}/{}",
                kitsu_kind_path(media_kind),
                item.id
            )),
            remote_updated_at: attributes.updated_at.clone(),
            raw_json: Some(raw_json),
        }],
        field_provenance: Vec::new(),
    })
}

fn kitsu_kind_path(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Anime | MediaKind::Show => "anime",
        MediaKind::Manga | MediaKind::Movie => "manga",
    }
}

fn kitsu_categories(item: &KitsuResource, included: &[KitsuIncluded]) -> Vec<String> {
    let Some(relationships) = &item.relationships else {
        return Vec::new();
    };
    let Some(categories) = &relationships.categories else {
        return Vec::new();
    };

    categories
        .data
        .iter()
        .filter_map(|reference| {
            included.iter().find_map(|candidate| match candidate {
                KitsuIncluded::Category(category)
                    if category.id == reference.id && reference.kind == "categories" =>
                {
                    category.attributes.title.clone()
                }
                _ => None,
            })
        })
        .collect()
}

fn kitsu_mappings(item: &KitsuResource, included: &[KitsuIncluded]) -> Vec<KitsuMappingAttributes> {
    let Some(relationships) = &item.relationships else {
        return Vec::new();
    };
    let Some(mappings) = &relationships.mappings else {
        return Vec::new();
    };

    mappings
        .data
        .iter()
        .filter_map(|reference| {
            included.iter().find_map(|candidate| match candidate {
                KitsuIncluded::Mapping(mapping)
                    if mapping.id == reference.id && reference.kind == "mappings" =>
                {
                    Some(mapping.attributes.clone())
                }
                _ => None,
            })
        })
        .collect()
}

fn prefer_kitsu_image(image: &KitsuImageSet) -> Option<String> {
    image
        .original
        .clone()
        .or(image.large.clone())
        .or(image.medium.clone())
        .or(image.small.clone())
        .or(image.tiny.clone())
}

fn parse_kitsu_year(value: &str) -> Option<i32> {
    value.get(0..4)?.parse().ok()
}

fn jikan_is_nsfw(rating: Option<&str>) -> bool {
    matches!(
        rating,
        Some(value)
            if value.contains("Rx")
                || value.contains("Hentai")
                || value.contains("R+")
                || value.contains("Mild Nudity")
    )
}

#[derive(Debug, Clone)]
pub struct TvmazeProvider {
    client: Client,
    endpoint: String,
}

impl Default for TvmazeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TvmazeProvider {
    pub const DEFAULT_ENDPOINT: &'static str = "https://api.tvmaze.com";

    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("animedb/0.1")
            .build()
            .expect("reqwest blocking client should build");

        Self {
            client,
            endpoint: Self::DEFAULT_ENDPOINT.to_string(),
        }
    }

    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        let mut provider = Self::new();
        provider.endpoint = endpoint.into();
        provider
    }
}

impl RemoteProvider for TvmazeProvider {
    fn source(&self) -> SourceName {
        SourceName::Tvmaze
    }

    fn min_interval(&self) -> Duration {
        Duration::from_millis(500)
    }

    fn fetch_page(&self, request: &SyncRequest, cursor: SyncCursor) -> Result<RemotePage> {
        let _page_size = request.page_size.clamp(1, 250);

        let response = self
            .client
            .get(format!("{}/shows", self.endpoint))
            .query(&[("page", cursor.page.to_string())])
            .send()?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(RemotePage {
                items: Vec::new(),
                next_cursor: None,
            });
        }

        let response = response.error_for_status()?;
        let shows: Vec<TvmazeShow> = response.json()?;

        if shows.is_empty() {
            return Ok(RemotePage {
                items: Vec::new(),
                next_cursor: None,
            });
        }

        let items = shows
            .into_iter()
            .filter_map(|show| map_tvmaze_show(show))
            .collect::<Vec<_>>();

        let next_cursor = Some(SyncCursor {
            page: cursor.page + 1,
        });

        Ok(RemotePage { items, next_cursor })
    }

    fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<CanonicalMedia>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let limit = options.limit.clamp(1, 50);

        let response = self
            .client
            .get(format!("{}/search/shows", self.endpoint))
            .query(&[("q", query.to_string()), ("limit", limit.to_string())])
            .send()?
            .error_for_status()?
            .json::<Vec<TvmazeSearchResult>>()?;

        Ok(response
            .into_iter()
            .filter_map(|result| map_tvmaze_show(result.show))
            .collect())
    }

    fn get_by_id(&self, _media_kind: MediaKind, source_id: &str) -> Result<Option<CanonicalMedia>> {
        let response = self
            .client
            .get(format!("{}/shows/{}", self.endpoint, source_id))
            .send()?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let response = response.error_for_status()?;
        let show: TvmazeShow = response.json()?;
        Ok(map_tvmaze_show(show))
    }
}

fn map_tvmaze_show(show: TvmazeShow) -> Option<CanonicalMedia> {
    let title_display = show.name.clone();
    if title_display.trim().is_empty() {
        return None;
    }

    let mut external_ids = vec![ExternalId {
        source: SourceName::Tvmaze,
        source_id: show.id.to_string(),
        url: show.url.clone(),
    }];

    if let Some(ref imdb_id) = show.externals.imdb {
        external_ids.push(ExternalId {
            source: SourceName::Imdb,
            source_id: imdb_id.clone(),
            url: Some(format!("https://www.imdb.com/title/{imdb_id}")),
        });
    }

    let mut genres = Vec::new();
    if let Some(ref show_genres) = show.genres {
        genres.extend(show_genres.iter().cloned());
    }

    let cover_image = show
        .image
        .as_ref()
        .and_then(|img| img.original.clone().or(img.medium.clone()));

    let synopsis = show.summary.as_deref().map(|s| {
        let stripped = s
            .replace("<p>", "")
            .replace("</p>", "")
            .replace("<br>", "\n")
            .replace("<br/>", "\n")
            .replace("<b>", "")
            .replace("</b>", "")
            .replace("<i>", "")
            .replace("</i>", "");
        stripped.trim().to_string()
    });

    Some(CanonicalMedia {
        media_kind: MediaKind::Show,
        title_display,
        title_romaji: None,
        title_english: None,
        title_native: None,
        synopsis,
        format: None,
        status: show.status.clone(),
        season: None,
        season_year: show.premiered.as_deref().and_then(parse_year),
        episodes: None,
        chapters: None,
        volumes: None,
        country_of_origin: show
            .network
            .as_ref()
            .and_then(|n| n.country.as_ref().and_then(|c| c.code.clone()))
            .or_else(|| {
                show.web_channel
                    .as_ref()
                    .and_then(|wc| wc.country.as_ref().and_then(|c| c.code.clone()))
            }),
        cover_image,
        banner_image: None,
        provider_rating: show
            .rating
            .as_ref()
            .and_then(|r| r.average)
            .map(|value| (value / 10.0).clamp(0.0, 1.0)),
        nsfw: false,
        aliases: Vec::new(),
        genres,
        tags: Vec::new(),
        external_ids,
        source_payloads: vec![SourcePayload {
            source: SourceName::Tvmaze,
            source_id: show.id.to_string(),
            url: show.url.clone(),
            remote_updated_at: show.updated.map(|v| v.to_string()),
            raw_json: Some(serde_json::to_value(&show).unwrap_or_default()),
        }],
        field_provenance: Vec::new(),
    })
}

fn parse_year(value: &str) -> Option<i32> {
    value.get(0..4)?.parse().ok()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TvmazeShow {
    id: i64,
    url: Option<String>,
    name: String,
    #[serde(default)]
    genres: Option<Vec<String>>,
    status: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    premiered: Option<String>,
    #[serde(default)]
    image: Option<TvmazeImage>,
    #[serde(default)]
    rating: Option<TvmazeRating>,
    #[serde(default)]
    network: Option<TvmazeNetwork>,
    #[serde(default, rename = "webChannel")]
    web_channel: Option<TvmazeWebChannel>,
    #[serde(default)]
    externals: TvmazeExternals,
    #[serde(default)]
    updated: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TvmazeSearchResult {
    score: Option<f64>,
    show: TvmazeShow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TvmazeImage {
    medium: Option<String>,
    original: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TvmazeRating {
    average: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TvmazeNetwork {
    #[serde(default)]
    country: Option<TvmazeCountry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TvmazeCountry {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TvmazeWebChannel {
    #[serde(default)]
    country: Option<TvmazeCountry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TvmazeExternals {
    #[serde(default)]
    imdb: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ImdbProvider {
    client: Client,
    base_url: String,
}

impl Default for ImdbProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ImdbProvider {
    pub const DEFAULT_BASE_URL: &'static str = "https://datasets.imdb.com";

    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .user_agent("animedb/0.1")
            .build()
            .expect("reqwest blocking client should build");

        Self {
            client,
            base_url: Self::DEFAULT_BASE_URL.to_string(),
        }
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let mut provider = Self::new();
        provider.base_url = base_url.into();
        provider
    }

    fn download_title_basics(&self) -> Result<Vec<u8>> {
        let url = format!("{}/title.basics.tsv.gz", self.base_url);
        let response = self.client.get(&url).send()?.error_for_status()?;
        let bytes = response.bytes()?;
        Ok(bytes.to_vec())
    }

    fn download_title_ratings(&self) -> Result<Vec<u8>> {
        let url = format!("{}/title.ratings.tsv.gz", self.base_url);
        let response = self.client.get(&url).send()?.error_for_status()?;
        let bytes = response.bytes()?;
        Ok(bytes.to_vec())
    }

    fn parse_title_type(title_type: &str) -> Option<MediaKind> {
        match title_type {
            "movie" | "tvMovie" | "video" => Some(MediaKind::Movie),
            "tvSeries" | "tvMiniSeries" | "tvSpecial" => Some(MediaKind::Show),
            _ => None,
        }
    }

    fn load_ratings(data: &[u8]) -> HashMap<String, f64> {
        let mut ratings = HashMap::new();
        let decoder = flate2::read::GzDecoder::new(data);
        let mut reader = std::io::BufReader::new(decoder);
        let mut line = String::new();

        let _ = line.clear();
        if std::io::BufRead::read_line(&mut reader, &mut line).is_ok() {
            // skip header
        }

        loop {
            line.clear();
            if std::io::BufRead::read_line(&mut reader, &mut line).is_err() || line.is_empty() {
                break;
            }
            let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');
            let parts: Vec<&str> = trimmed.split('\t').collect();
            if parts.len() >= 2 {
                if let Ok(rating) = parts[1].parse::<f64>() {
                    ratings.insert(parts[0].to_string(), rating);
                }
            }
        }

        ratings
    }
}

fn imdb_null(value: &str) -> Option<String> {
    if value == "\\N" || value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn imdb_null_i32(value: &str) -> Option<i32> {
    if value == "\\N" || value.is_empty() {
        None
    } else {
        value.parse().ok()
    }
}

impl RemoteProvider for ImdbProvider {
    fn source(&self) -> SourceName {
        SourceName::Imdb
    }

    fn min_interval(&self) -> Duration {
        Duration::ZERO
    }

    fn fetch_page(&self, request: &SyncRequest, cursor: SyncCursor) -> Result<RemotePage> {
        let media_kind = request.media_kind.unwrap_or(MediaKind::Movie);
        let page_size = request.page_size.clamp(1, 500);

        let basics_data = self.download_title_basics()?;
        let ratings_data = self.download_title_ratings()?;
        let ratings = ImdbProvider::load_ratings(&ratings_data);

        let decoder = flate2::read::GzDecoder::new(&basics_data[..]);
        let reader = std::io::BufReader::new(decoder);
        let mut lines = reader.lines();

        let _ = lines.next();

        let skip = (cursor.page.saturating_sub(1)) * page_size;
        let mut items = Vec::new();
        let mut line_index = 0usize;
        let mut consumed = 0usize;

        while let Some(Ok(line)) = lines.next() {
            let trimmed = line.trim_end_matches('\r');
            let parts: Vec<&str> = trimmed.split('\t').collect();

            if parts.len() < 9 {
                continue;
            }

            let title_type = parts[1];
            let kind = match Self::parse_title_type(title_type) {
                Some(k) => k,
                None => continue,
            };

            if kind != media_kind {
                continue;
            }

            line_index += 1;
            if line_index <= skip {
                continue;
            }

            if consumed >= page_size {
                break;
            }

            let tconst = parts[0];
            let primary_title = imdb_null(parts[2]).unwrap_or_default();
            let original_title = imdb_null(parts[3]);
            let is_adult = parts[4] == "1";
            let start_year = imdb_null_i32(parts[5]);
            let end_year = imdb_null_i32(parts[6]);
            let runtime_minutes = imdb_null_i32(parts[7]);
            let genres = imdb_null(parts[8]);

            if primary_title.trim().is_empty() {
                continue;
            }

            let rating = ratings.get(tconst).copied();

            let title_display = original_title
                .as_deref()
                .unwrap_or(&primary_title)
                .to_string();

            let external_ids = vec![ExternalId {
                source: SourceName::Imdb,
                source_id: tconst.to_string(),
                url: Some(format!("https://www.imdb.com/title/{tconst}")),
            }];

            let genre_list: Vec<String> = genres
                .map(|g| g.split(',').map(|s| s.to_string()).collect())
                .unwrap_or_default();

            let mut aliases = Vec::new();
            if original_title.as_deref() != Some(&primary_title) {
                if let Some(ref ot) = original_title {
                    aliases.push(ot.clone());
                }
            }

            let mut raw = serde_json::Map::new();
            raw.insert(
                "tconst".into(),
                serde_json::Value::String(tconst.to_string()),
            );
            raw.insert(
                "titleType".into(),
                serde_json::Value::String(title_type.to_string()),
            );
            raw.insert(
                "primaryTitle".into(),
                serde_json::Value::String(primary_title.clone()),
            );
            raw.insert("isAdult".into(), serde_json::Value::Bool(is_adult));
            if let Some(ref sy) = start_year {
                raw.insert("startYear".into(), serde_json::Value::Number((*sy).into()));
            }
            if let Some(ref ey) = end_year {
                raw.insert("endYear".into(), serde_json::Value::Number((*ey).into()));
            }
            if let Some(ref rm) = runtime_minutes {
                raw.insert(
                    "runtimeMinutes".into(),
                    serde_json::Value::Number((*rm).into()),
                );
            }

            items.push(CanonicalMedia {
                media_kind: kind,
                title_display,
                title_romaji: None,
                title_english: original_title,
                title_native: None,
                synopsis: None,
                format: Some(title_type.to_string()),
                status: None,
                season: start_year.map(|y| {
                    let mut s = String::new();
                    s.push_str(&y.to_string());
                    s
                }),
                season_year: start_year,
                episodes: runtime_minutes,
                chapters: None,
                volumes: None,
                country_of_origin: None,
                cover_image: None,
                banner_image: None,
                provider_rating: rating.map(|r| (r / 10.0).clamp(0.0, 1.0)),
                nsfw: is_adult,
                aliases,
                genres: genre_list,
                tags: Vec::new(),
                external_ids,
                source_payloads: vec![SourcePayload {
                    source: SourceName::Imdb,
                    source_id: tconst.to_string(),
                    url: Some(format!("https://www.imdb.com/title/{tconst}")),
                    remote_updated_at: None,
                    raw_json: Some(serde_json::Value::Object(raw)),
                }],
                field_provenance: Vec::new(),
            });

            consumed += 1;
        }

        let next_cursor = if consumed >= page_size {
            Some(SyncCursor {
                page: cursor.page + 1,
            })
        } else {
            None
        };

        Ok(RemotePage { items, next_cursor })
    }

    fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<CanonicalMedia>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let media_kind = options.media_kind.unwrap_or(MediaKind::Movie);
        let limit = options.limit.clamp(1, 100);

        let basics_data = self.download_title_basics()?;
        let decoder = flate2::read::GzDecoder::new(&basics_data[..]);
        let reader = std::io::BufReader::new(decoder);
        let mut lines = reader.lines();

        let _ = lines.next();

        let query_lower = query.to_ascii_lowercase();
        let mut items = Vec::new();
        let mut found = 0usize;

        while let Some(Ok(line)) = lines.next() {
            if found >= limit {
                break;
            }

            let trimmed = line.trim_end_matches('\r');
            let parts: Vec<&str> = trimmed.split('\t').collect();

            if parts.len() < 9 {
                continue;
            }

            let title_type = parts[1];
            let kind = match Self::parse_title_type(title_type) {
                Some(k) => k,
                None => continue,
            };

            if kind != media_kind {
                continue;
            }

            let primary_title = imdb_null(parts[2]).unwrap_or_default();
            if primary_title.to_ascii_lowercase().contains(&query_lower) {
                if let Some(media) = build_imdb_media_from_parts(&parts, kind) {
                    items.push(media);
                    found += 1;
                }
            }
        }

        Ok(items)
    }

    fn get_by_id(&self, media_kind: MediaKind, source_id: &str) -> Result<Option<CanonicalMedia>> {
        let basics_data = self.download_title_basics()?;
        let decoder = flate2::read::GzDecoder::new(&basics_data[..]);
        let reader = std::io::BufReader::new(decoder);
        let mut lines = reader.lines();

        let _ = lines.next();

        while let Some(Ok(line)) = lines.next() {
            let trimmed = line.trim_end_matches('\r');
            let parts: Vec<&str> = trimmed.split('\t').collect();

            if parts.len() < 9 {
                continue;
            }

            if parts[0] != source_id {
                continue;
            }

            let title_type = parts[1];
            let kind = match Self::parse_title_type(title_type) {
                Some(k) => k,
                None => return Ok(None),
            };

            if kind != media_kind {
                continue;
            }

            return Ok(build_imdb_media_from_parts(&parts, kind));
        }

        Ok(None)
    }
}

fn build_imdb_media_from_parts(parts: &[&str], kind: MediaKind) -> Option<CanonicalMedia> {
    if parts.len() < 9 {
        return None;
    }

    let tconst = parts[0];
    let primary_title = imdb_null(parts[2])?;
    let original_title = imdb_null(parts[3]);
    let is_adult = parts[4] == "1";
    let start_year = imdb_null_i32(parts[5]);
    let _end_year = imdb_null_i32(parts[6]);
    let runtime_minutes = imdb_null_i32(parts[7]);
    let genres = imdb_null(parts[8]);

    let title_display = original_title
        .as_deref()
        .unwrap_or(&primary_title)
        .to_string();

    let external_ids = vec![ExternalId {
        source: SourceName::Imdb,
        source_id: tconst.to_string(),
        url: Some(format!("https://www.imdb.com/title/{tconst}")),
    }];

    let genre_list: Vec<String> = genres
        .map(|g| g.split(',').map(|s| s.to_string()).collect())
        .unwrap_or_default();

    let mut aliases = Vec::new();
    if original_title.as_deref() != Some(&primary_title) {
        if let Some(ref ot) = original_title {
            aliases.push(ot.clone());
        }
    }

    let title_type = parts[1];

    Some(CanonicalMedia {
        media_kind: kind,
        title_display,
        title_romaji: None,
        title_english: original_title,
        title_native: None,
        synopsis: None,
        format: Some(title_type.to_string()),
        status: None,
        season: start_year.map(|y| y.to_string()),
        season_year: start_year,
        episodes: runtime_minutes,
        chapters: None,
        volumes: None,
        country_of_origin: None,
        cover_image: None,
        banner_image: None,
        provider_rating: None,
        nsfw: is_adult,
        aliases,
        genres: genre_list,
        tags: Vec::new(),
        external_ids,
        source_payloads: vec![SourcePayload {
            source: SourceName::Imdb,
            source_id: tconst.to_string(),
            url: Some(format!("https://www.imdb.com/title/{tconst}")),
            remote_updated_at: None,
            raw_json: Some(serde_json::json!({
                "tconst": tconst,
                "titleType": title_type,
                "primaryTitle": primary_title,
                "isAdult": is_adult,
                "startYear": start_year,
                "runtimeMinutes": runtime_minutes,
            })),
        }],
        field_provenance: Vec::new(),
    })
}

const ANILIST_PAGE_QUERY: &str = r#"
query ($page: Int, $perPage: Int, $type: MediaType, $sort: [MediaSort]) {
  Page(page: $page, perPage: $perPage) {
    pageInfo {
      currentPage
      hasNextPage
    }
    media(type: $type, sort: $sort) {
      id
      idMal
      type
      title {
        romaji
        english
        native
      }
      synonyms
      description(asHtml: false)
      format
      status
      episodes
      chapters
      volumes
      countryOfOrigin
      season
      seasonYear
      genres
      averageScore
      updatedAt
      siteUrl
      isAdult
      bannerImage
      coverImage {
        medium
        large
        extraLarge
      }
      tags {
        name
      }
    }
  }
}
"#;

const ANILIST_SEARCH_QUERY: &str = r#"
query ($page: Int, $perPage: Int, $type: MediaType, $search: String) {
  Page(page: $page, perPage: $perPage) {
    media(type: $type, search: $search) {
      id
      idMal
      type
      title {
        romaji
        english
        native
      }
      synonyms
      description(asHtml: false)
      format
      status
      episodes
      chapters
      volumes
      countryOfOrigin
      season
      seasonYear
      genres
      averageScore
      updatedAt
      siteUrl
      isAdult
      bannerImage
      coverImage {
        medium
        large
        extraLarge
      }
      tags {
        name
      }
    }
  }
}
"#;

const ANILIST_BY_ID_QUERY: &str = r#"
query ($id: Int, $type: MediaType) {
  Media(id: $id, type: $type) {
    id
    idMal
    type
    title {
      romaji
      english
      native
    }
    synonyms
    description(asHtml: false)
    format
    status
    episodes
    chapters
    volumes
    countryOfOrigin
    season
    seasonYear
    genres
    averageScore
    updatedAt
    siteUrl
    isAdult
    bannerImage
    coverImage {
      medium
      large
      extraLarge
    }
    tags {
      name
    }
  }
}
"#;

#[derive(Debug, Deserialize)]
struct AniListGraphQlResponse {
    #[serde(default)]
    data: Option<AniListData>,
    #[serde(default)]
    errors: Vec<AniListError>,
}

#[derive(Debug, Deserialize)]
struct AniListSingleMediaResponse {
    #[serde(default)]
    data: Option<AniListSingleMediaData>,
    #[serde(default)]
    errors: Vec<AniListError>,
}

#[derive(Debug, Deserialize)]
struct AniListError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct AniListData {
    #[serde(rename = "Page")]
    page: AniListPage,
}

#[derive(Debug, Deserialize)]
struct AniListSingleMediaData {
    #[serde(rename = "Media")]
    media: Option<AniListMedia>,
}

#[derive(Debug, Deserialize)]
struct AniListPage {
    #[serde(rename = "pageInfo", default)]
    page_info: AniListPageInfo,
    #[serde(default)]
    media: Vec<AniListMedia>,
}

#[derive(Debug, Default, Deserialize)]
struct AniListPageInfo {
    #[serde(rename = "currentPage", default)]
    _current_page: Option<u32>,
    #[serde(rename = "hasNextPage", default)]
    has_next_page: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AniListMedia {
    id: i64,
    #[serde(rename = "idMal")]
    id_mal: Option<i64>,
    title: AniListTitle,
    #[serde(default)]
    synonyms: Vec<String>,
    description: Option<String>,
    format: Option<String>,
    status: Option<String>,
    episodes: Option<i32>,
    chapters: Option<i32>,
    volumes: Option<i32>,
    #[serde(rename = "countryOfOrigin")]
    country_of_origin: Option<String>,
    season: Option<String>,
    #[serde(rename = "seasonYear")]
    season_year: Option<i32>,
    #[serde(default)]
    genres: Vec<String>,
    #[serde(rename = "updatedAt")]
    updated_at: Option<i64>,
    #[serde(rename = "siteUrl")]
    site_url: Option<String>,
    #[serde(rename = "isAdult")]
    is_adult: Option<bool>,
    #[serde(rename = "averageScore")]
    average_score: Option<f64>,
    #[serde(rename = "bannerImage")]
    banner_image: Option<String>,
    #[serde(rename = "coverImage")]
    cover_image: Option<AniListCoverImage>,
    #[serde(default)]
    tags: Vec<AniListTag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AniListTitle {
    romaji: Option<String>,
    english: Option<String>,
    native: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AniListTag {
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AniListCoverImage {
    medium: Option<String>,
    large: Option<String>,
    #[serde(rename = "extraLarge")]
    extra_large: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JikanListResponse {
    #[serde(default)]
    data: Vec<JikanMedia>,
    pagination: Option<JikanPagination>,
}

#[derive(Debug, Deserialize)]
struct JikanItemResponse {
    data: Option<JikanMedia>,
}

#[derive(Debug, Deserialize)]
struct JikanPagination {
    #[serde(rename = "has_next_page")]
    has_next_page: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JikanMedia {
    #[serde(rename = "mal_id")]
    mal_id: i64,
    url: Option<String>,
    title: Option<String>,
    #[serde(rename = "title_english")]
    title_english: Option<String>,
    #[serde(rename = "title_japanese")]
    title_japanese: Option<String>,
    #[serde(rename = "title_synonyms", default)]
    title_synonyms: Vec<String>,
    #[serde(default)]
    titles: Vec<JikanTitle>,
    synopsis: Option<String>,
    #[serde(rename = "type")]
    media_type: Option<String>,
    status: Option<String>,
    episodes: Option<i32>,
    chapters: Option<i32>,
    volumes: Option<i32>,
    season: Option<String>,
    year: Option<i32>,
    score: Option<f64>,
    rating: Option<String>,
    images: JikanImages,
    trailer: Option<JikanTrailer>,
    #[serde(default)]
    genres: Vec<JikanNamedValue>,
    #[serde(default)]
    themes: Vec<JikanNamedValue>,
    #[serde(default)]
    demographics: Vec<JikanNamedValue>,
    #[serde(rename = "updated_at")]
    updated_at: Option<JikanUpdatedAt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JikanTitle {
    #[serde(rename = "type")]
    _title_type: Option<String>,
    title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JikanImages {
    jpg: JikanImageSet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JikanImageSet {
    #[serde(rename = "image_url")]
    image_url: Option<String>,
    #[serde(rename = "large_image_url")]
    large_image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JikanTrailer {
    images: JikanTrailerImages,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JikanTrailerImages {
    #[serde(rename = "maximum_image_url")]
    maximum_image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JikanNamedValue {
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JikanUpdatedAt {
    from: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KitsuCollectionResponse {
    #[serde(default)]
    data: Vec<KitsuResource>,
    #[serde(default)]
    included: Vec<KitsuIncluded>,
    #[serde(default)]
    links: KitsuLinks,
}

#[derive(Debug, Deserialize)]
struct KitsuItemResponse {
    data: Option<KitsuResource>,
    #[serde(default)]
    included: Vec<KitsuIncluded>,
}

#[derive(Debug, Default, Deserialize)]
struct KitsuLinks {
    next: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KitsuResource {
    id: String,
    #[serde(rename = "type")]
    _resource_type: String,
    attributes: KitsuMediaAttributes,
    relationships: Option<KitsuRelationships>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KitsuMediaAttributes {
    #[serde(rename = "updatedAt")]
    updated_at: Option<String>,
    description: Option<String>,
    synopsis: Option<String>,
    #[serde(rename = "canonicalTitle")]
    canonical_title: Option<String>,
    titles: KitsuTitles,
    #[serde(rename = "abbreviatedTitles", default)]
    abbreviated_titles: Vec<String>,
    #[serde(rename = "averageRating")]
    average_rating: Option<String>,
    #[serde(rename = "startDate")]
    start_date: Option<String>,
    #[serde(rename = "ageRating")]
    age_rating: Option<String>,
    status: Option<String>,
    #[serde(rename = "episodeCount")]
    episode_count: Option<i32>,
    #[serde(rename = "chapterCount")]
    chapter_count: Option<i32>,
    #[serde(rename = "volumeCount")]
    volume_count: Option<i32>,
    subtype: Option<String>,
    nsfw: Option<bool>,
    #[serde(rename = "coverImage")]
    cover_image: Option<KitsuImageSet>,
    #[serde(rename = "posterImage")]
    poster_image: Option<KitsuImageSet>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct KitsuTitles {
    en: Option<String>,
    #[serde(rename = "en_jp")]
    en_jp: Option<String>,
    #[serde(rename = "ja_jp")]
    ja_jp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KitsuImageSet {
    tiny: Option<String>,
    small: Option<String>,
    medium: Option<String>,
    large: Option<String>,
    original: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KitsuRelationships {
    categories: Option<KitsuRelationshipCollection>,
    mappings: Option<KitsuRelationshipCollection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KitsuRelationshipCollection {
    #[serde(default)]
    data: Vec<KitsuRelationshipRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KitsuRelationshipRef {
    id: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum KitsuIncluded {
    #[serde(rename = "categories")]
    Category(KitsuCategoryResource),
    #[serde(rename = "mappings")]
    Mapping(KitsuMappingResource),
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KitsuCategoryResource {
    id: String,
    attributes: KitsuCategoryAttributes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KitsuCategoryAttributes {
    title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KitsuMappingResource {
    id: String,
    attributes: KitsuMappingAttributes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KitsuMappingAttributes {
    #[serde(rename = "externalSite")]
    external_site: String,
    #[serde(rename = "externalId")]
    external_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{MediaKind, SourceName};

    #[test]
    fn tvmaze_show_maps_to_canonical() {
        let json = r#"{
            "id": 169,
            "url": "https://www.tvmaze.com/shows/169/breaking-bad",
            "name": "Breaking Bad",
            "genres": ["Drama", "Crime"],
            "status": "Ended",
            "summary": "<p>A high school chemistry teacher turns to making meth.</p>",
            "premiered": "2008-01-20",
            "image": {"medium": "https://static.tvmaze.com/uploads/images/medium/0/2000.jpg", "original": "https://static.tvmaze.com/uploads/images/original_untouched/0/2000.jpg"},
            "rating": {"average": 9.5},
            "network": {"country": {"name": "United States", "code": "US", "timezone": "America/New_York"}},
            "webChannel": null,
            "externals": {"imdb": "tt0903747"},
            "updated": 1712440000
        }"#;

        let show: TvmazeShow = serde_json::from_str(json).expect("parse tvmaze show");
        let canonical = map_tvmaze_show(show).expect("map tvmaze show");

        assert_eq!(canonical.media_kind, MediaKind::Show);
        assert_eq!(canonical.title_display, "Breaking Bad");
        assert_eq!(canonical.season_year, Some(2008));
        assert_eq!(
            canonical.synopsis.as_deref(),
            Some("A high school chemistry teacher turns to making meth.")
        );
        assert!(canonical.genres.contains(&"Drama".to_string()));
        assert!(canonical.genres.contains(&"Crime".to_string()));
        assert!(canonical
            .external_ids
            .iter()
            .any(|id| id.source == SourceName::Tvmaze));
        assert!(canonical
            .external_ids
            .iter()
            .any(|id| id.source == SourceName::Imdb && id.source_id == "tt0903747"));
        assert_eq!(canonical.provider_rating, Some(0.95));
        assert!(canonical.cover_image.is_some());
        assert_eq!(canonical.country_of_origin.as_deref(), Some("US"));
    }

    #[test]
    fn tvmaze_show_without_optional_fields() {
        let json = r#"{
            "id": 1,
            "url": "https://www.tvmaze.com/shows/1/under-the-dome",
            "name": "Under the Dome",
            "genres": null,
            "status": "Ended",
            "summary": null,
            "premiered": null,
            "image": null,
            "rating": null,
            "network": null,
            "webChannel": null,
            "externals": {},
            "updated": null
        }"#;

        let show: TvmazeShow = serde_json::from_str(json).expect("parse tvmaze show");
        let canonical = map_tvmaze_show(show).expect("map tvmaze show");

        assert_eq!(canonical.title_display, "Under the Dome");
        assert!(canonical.synopsis.is_none());
        assert!(canonical.genres.is_empty());
        assert!(canonical
            .external_ids
            .iter()
            .any(|id| id.source == SourceName::Tvmaze));
    }

    #[test]
    fn imdb_title_type_mapping() {
        assert_eq!(
            ImdbProvider::parse_title_type("movie"),
            Some(MediaKind::Movie)
        );
        assert_eq!(
            ImdbProvider::parse_title_type("tvMovie"),
            Some(MediaKind::Movie)
        );
        assert_eq!(
            ImdbProvider::parse_title_type("video"),
            Some(MediaKind::Movie)
        );
        assert_eq!(
            ImdbProvider::parse_title_type("tvSeries"),
            Some(MediaKind::Show)
        );
        assert_eq!(
            ImdbProvider::parse_title_type("tvMiniSeries"),
            Some(MediaKind::Show)
        );
        assert_eq!(
            ImdbProvider::parse_title_type("tvSpecial"),
            Some(MediaKind::Show)
        );
        assert_eq!(ImdbProvider::parse_title_type("short"), None);
        assert_eq!(ImdbProvider::parse_title_type("tvEpisode"), None);
    }

    #[test]
    fn imdb_null_handling() {
        assert_eq!(imdb_null("\\N"), None);
        assert_eq!(imdb_null(""), None);
        assert_eq!(imdb_null("Drama"), Some("Drama".to_string()));
        assert_eq!(imdb_null_i32("\\N"), None);
        assert_eq!(imdb_null_i32("2008"), Some(2008));
    }

    #[test]
    fn media_kind_str_roundtrip() {
        assert_eq!(MediaKind::Show.as_str(), "show");
        assert_eq!(MediaKind::Movie.as_str(), "movie");
        assert!(matches!("show".parse::<MediaKind>(), Ok(MediaKind::Show)));
        assert!(matches!("movie".parse::<MediaKind>(), Ok(MediaKind::Movie)));
        assert!(matches!("SHOW".parse::<MediaKind>(), Ok(MediaKind::Show)));
        assert!(matches!("MOVIE".parse::<MediaKind>(), Ok(MediaKind::Movie)));
    }

    #[test]
    fn source_name_str_roundtrip() {
        assert_eq!(SourceName::Tvmaze.as_str(), "tvmaze");
        assert_eq!(SourceName::Imdb.as_str(), "imdb");
        assert!(matches!(
            "tvmaze".parse::<SourceName>(),
            Ok(SourceName::Tvmaze)
        ));
        assert!(matches!("imdb".parse::<SourceName>(), Ok(SourceName::Imdb)));
    }

    #[test]
    fn provider_weights_for_new_sources() {
        use crate::merge::provider_weight;
        assert_eq!(provider_weight(SourceName::Tvmaze), 0.82);
        assert_eq!(provider_weight(SourceName::Imdb), 0.85);
    }

    #[test]
    fn tvmaze_empty_name_returns_none() {
        let json = r#"{
            "id": 999,
            "url": "https://www.tvmaze.com/shows/999/",
            "name": "",
            "genres": null,
            "status": null,
            "summary": null,
            "premiered": null,
            "image": null,
            "rating": null,
            "network": null,
            "webChannel": null,
            "externals": {},
            "updated": null
        }"#;

        let show: TvmazeShow = serde_json::from_str(json).expect("parse");
        assert!(map_tvmaze_show(show).is_none());
    }

    #[test]
    fn tvmaze_whitespace_only_name_returns_none() {
        let json = r#"{
            "id": 999,
            "url": "https://www.tvmaze.com/shows/999/",
            "name": "   ",
            "genres": null,
            "status": null,
            "summary": null,
            "premiered": null,
            "image": null,
            "rating": null,
            "network": null,
            "webChannel": null,
            "externals": {},
            "updated": null
        }"#;

        let show: TvmazeShow = serde_json::from_str(json).expect("parse");
        assert!(map_tvmaze_show(show).is_none());
    }

    #[test]
    fn tvmaze_strips_html_from_summary() {
        let json = r#"{
            "id": 10,
            "url": "https://www.tvmaze.com/shows/10/",
            "name": "Test Show",
            "genres": [],
            "status": "Running",
            "summary": "<p>This is <b>bold</b> and <i>italic</i>.</p><br/>New line.",
            "premiered": "2023-05-01",
            "image": null,
            "rating": null,
            "network": null,
            "webChannel": null,
            "externals": {},
            "updated": null
        }"#;

        let show: TvmazeShow = serde_json::from_str(json).expect("parse");
        let canonical = map_tvmaze_show(show).expect("map");
        assert_eq!(
            canonical.synopsis.as_deref(),
            Some("This is bold and italic.\nNew line.")
        );
    }

    #[test]
    fn tvmaze_no_imdb_external_id() {
        let json = r#"{
            "id": 42,
            "url": "https://www.tvmaze.com/shows/42/",
            "name": "Obscure Show",
            "genres": [],
            "status": "Ended",
            "summary": "<p>A show without IMDB.</p>",
            "premiered": "2020-01-01",
            "image": null,
            "rating": {"average": 7.2},
            "network": null,
            "webChannel": null,
            "externals": {},
            "updated": null
        }"#;

        let show: TvmazeShow = serde_json::from_str(json).expect("parse");
        let canonical = map_tvmaze_show(show).expect("map");

        assert_eq!(canonical.external_ids.len(), 1);
        assert_eq!(canonical.external_ids[0].source, SourceName::Tvmaze);
        assert_eq!(canonical.external_ids[0].source_id, "42");
        assert!(!canonical
            .external_ids
            .iter()
            .any(|id| id.source == SourceName::Imdb));
    }

    #[test]
    fn tvmaze_web_channel_country() {
        let json = r#"{
            "id": 55,
            "url": "https://www.tvmaze.com/shows/55/",
            "name": "Netflix Original",
            "genres": ["Sci-Fi"],
            "status": "Running",
            "summary": null,
            "premiered": "2021-06-15",
            "image": null,
            "rating": null,
            "network": null,
            "webChannel": {"country": {"name": "United States", "code": "US"}},
            "externals": {"imdb": "tt1234567"},
            "updated": 1712440000
        }"#;

        let show: TvmazeShow = serde_json::from_str(json).expect("parse");
        let canonical = map_tvmaze_show(show).expect("map");
        assert_eq!(canonical.country_of_origin.as_deref(), Some("US"));
    }

    #[test]
    fn tvmaze_rating_zero() {
        let json = r#"{
            "id": 77,
            "url": "https://www.tvmaze.com/shows/77/",
            "name": "Unrated Show",
            "genres": [],
            "status": "Ended",
            "summary": null,
            "premiered": null,
            "image": null,
            "rating": {"average": 0.0},
            "network": null,
            "webChannel": null,
            "externals": {},
            "updated": null
        }"#;

        let show: TvmazeShow = serde_json::from_str(json).expect("parse");
        let canonical = map_tvmaze_show(show).expect("map");
        assert_eq!(canonical.provider_rating, Some(0.0));
    }

    #[test]
    fn imdb_parse_year_various_formats() {
        assert_eq!(parse_year("2023-01-15"), Some(2023));
        assert_eq!(parse_year("1999"), Some(1999));
        assert_eq!(parse_year(""), None);
        assert_eq!(parse_year("abc"), None);
    }

    #[test]
    fn imdb_all_null_fields() {
        assert_eq!(imdb_null("\\N"), None);
        assert_eq!(imdb_null(""), None);
        assert_eq!(imdb_null("Action,Crime"), Some("Action,Crime".to_string()));
        assert_eq!(imdb_null_i32("\\N"), None);
        assert_eq!(imdb_null_i32(""), None);
        assert_eq!(imdb_null_i32("0"), Some(0));
        assert_eq!(imdb_null_i32("2024"), Some(2024));
    }

    #[test]
    fn imdb_title_type_all_variants() {
        let movie_types = ["movie", "tvMovie", "video"];
        for t in &movie_types {
            assert!(
                matches!(ImdbProvider::parse_title_type(t), Some(MediaKind::Movie)),
                "expected Movie for type '{t}'"
            );
        }

        let show_types = ["tvSeries", "tvMiniSeries", "tvSpecial"];
        for t in &show_types {
            assert!(
                matches!(ImdbProvider::parse_title_type(t), Some(MediaKind::Show)),
                "expected Show for type '{t}'"
            );
        }

        let skipped_types = ["short", "tvEpisode", "tvShort", "videoGame"];
        for t in &skipped_types {
            assert!(
                ImdbProvider::parse_title_type(t).is_none(),
                "expected None for type '{t}'"
            );
        }
    }

    #[test]
    fn build_imdb_media_from_parts_basic() {
        let parts = vec![
            "tt0111161",
            "movie",
            "The Shawshank Redemption",
            "The Shawshank Redemption",
            "0",
            "1994",
            "\\N",
            "142",
            "Drama",
        ];
        let result = build_imdb_media_from_parts(&parts, MediaKind::Movie);
        assert!(result.is_some());
        let media = result.unwrap();
        assert_eq!(media.media_kind, MediaKind::Movie);
        assert_eq!(media.title_display, "The Shawshank Redemption");
        assert_eq!(media.season_year, Some(1994));
        assert_eq!(media.episodes, Some(142));
        assert!(!media.nsfw);
        assert!(media.genres.contains(&"Drama".to_string()));
        assert!(media
            .external_ids
            .iter()
            .any(|id| id.source == SourceName::Imdb && id.source_id == "tt0111161"));
    }

    #[test]
    fn build_imdb_media_from_parts_with_nulls() {
        let parts = vec![
            "tt0000001",
            "movie",
            "A Film",
            "\\N",
            "0",
            "\\N",
            "\\N",
            "\\N",
            "\\N",
        ];
        let result = build_imdb_media_from_parts(&parts, MediaKind::Movie);
        assert!(result.is_some());
        let media = result.unwrap();
        assert_eq!(media.title_display, "A Film");
        assert!(media.title_english.is_none());
        assert!(media.season_year.is_none());
        assert!(media.episodes.is_none());
        assert!(media.genres.is_empty());
    }

    #[test]
    fn build_imdb_media_from_parts_adult_flag() {
        let parts = vec![
            "tt1234567",
            "movie",
            "Adult Film",
            "Adult Film",
            "1",
            "2020",
            "\\N",
            "90",
            "Adult",
        ];
        let result = build_imdb_media_from_parts(&parts, MediaKind::Movie);
        assert!(result.is_some());
        let media = result.unwrap();
        assert!(media.nsfw);
    }

    #[test]
    fn build_imdb_media_from_parts_different_original_title() {
        let parts = vec![
            "tt1234567",
            "tvSeries",
            "The Office",
            "The Office (UK)",
            "0",
            "2001",
            "2003",
            "14",
            "Comedy",
        ];
        let result = build_imdb_media_from_parts(&parts, MediaKind::Show);
        assert!(result.is_some());
        let media = result.unwrap();
        assert_eq!(media.title_display, "The Office (UK)");
        assert!(media.title_english.as_deref() == Some("The Office (UK)"));
        assert!(media.aliases.contains(&"The Office (UK)".to_string()));
        assert_eq!(media.genres.len(), 1);
    }

    #[test]
    fn imdb_load_ratings_parses_tsv_lines() {
        use std::collections::HashMap;

        let raw = "tt0111161\t9.3\t2700000\ntt0068646\t9.2\t1800000\n";
        let mut ratings: HashMap<String, f64> = HashMap::new();
        for line in raw.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                if let Ok(rating) = parts[1].parse::<f64>() {
                    ratings.insert(parts[0].to_string(), rating);
                }
            }
        }
        assert_eq!(ratings.get("tt0111161"), Some(&9.3_f64));
        assert_eq!(ratings.get("tt0068646"), Some(&9.2_f64));
        assert_eq!(ratings.get("tt9999999"), None);
    }

    #[test]
    fn media_kind_display_roundtrip() {
        for (kind, s) in [
            (MediaKind::Anime, "anime"),
            (MediaKind::Manga, "manga"),
            (MediaKind::Show, "show"),
            (MediaKind::Movie, "movie"),
        ] {
            assert_eq!(kind.as_str(), s);
            assert_eq!(kind.to_string(), s);
        }
    }

    #[test]
    fn source_name_display_roundtrip() {
        for (source, s) in [
            (SourceName::AniList, "anilist"),
            (SourceName::MyAnimeList, "myanimelist"),
            (SourceName::Jikan, "jikan"),
            (SourceName::Kitsu, "kitsu"),
            (SourceName::Tvmaze, "tvmaze"),
            (SourceName::Imdb, "imdb"),
        ] {
            assert_eq!(source.as_str(), s);
            assert_eq!(source.to_string(), s);
        }
    }
}
