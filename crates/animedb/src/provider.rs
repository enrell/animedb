use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
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
        provider_rating: item.average_score.map(|value| (value / 100.0).clamp(0.0, 1.0)),
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
            MediaKind::Anime => "anime",
            MediaKind::Manga => "manga",
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
            MediaKind::Anime => "anime",
            MediaKind::Manga => "manga",
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
            MediaKind::Anime => "anime",
            MediaKind::Manga => "manga",
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

        Ok(Some(map_kitsu_media(&item, &response.included, media_kind)?))
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
        cover_image: item.images.jpg.large_image_url.clone().or(item.images.jpg.image_url.clone()),
        banner_image: item.trailer.as_ref().and_then(|trailer| trailer.images.maximum_image_url.clone()),
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
            remote_updated_at: item.updated_at.as_ref().and_then(|value| value.from.clone()),
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
        url: Some(format!("https://kitsu.io/{}/{}", kitsu_kind_path(media_kind), item.id)),
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
        season_year: attributes
            .start_date
            .as_deref()
            .and_then(parse_kitsu_year),
        episodes: attributes.episode_count,
        chapters: attributes.chapter_count,
        volumes: attributes.volume_count,
        country_of_origin: None,
        cover_image: attributes.poster_image.as_ref().and_then(prefer_kitsu_image),
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
            url: Some(format!("https://kitsu.io/{}/{}", kitsu_kind_path(media_kind), item.id)),
            remote_updated_at: attributes.updated_at.clone(),
            raw_json: Some(raw_json),
        }],
        field_provenance: Vec::new(),
    })
}

fn kitsu_kind_path(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Anime => "anime",
        MediaKind::Manga => "manga",
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
