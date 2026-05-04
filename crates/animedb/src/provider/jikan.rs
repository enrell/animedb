/// Jikan v4 provider (unofficial MyAnimeList REST API).
///
/// Implements [`Provider`](super::Provider) for `api.jikan.moe`.
/// All Jikan-specific response types are private to this module.
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::{Error, Result};
use crate::model::{
    CanonicalMedia, ExternalId, MediaKind, SearchOptions, SourceName, SourcePayload, SyncCursor,
    SyncRequest,
};

use super::http::{HttpClient, clamp_page_size};
use super::{FetchPage, Provider};

// ---------------------------------------------------------------------------
// Provider struct
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct JikanProvider {
    client: HttpClient,
}

impl Default for JikanProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl JikanProvider {
    pub const DEFAULT_ENDPOINT: &'static str = "https://api.jikan.moe/v4";

    pub fn new() -> Self {
        Self {
            client: HttpClient::new(Duration::from_secs(30), Self::DEFAULT_ENDPOINT),
        }
    }

    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            client: HttpClient::new(Duration::from_secs(30), Self::DEFAULT_ENDPOINT)
                .with_base_url(endpoint),
        }
    }
}

// ---------------------------------------------------------------------------
// Provider impl
// ---------------------------------------------------------------------------

impl Provider for JikanProvider {
    fn source(&self) -> SourceName {
        SourceName::Jikan
    }

    fn min_interval(&self) -> Duration {
        // Jikan free tier: max 1 req/sec per the official docs.
        Duration::from_millis(1_100)
    }

    fn fetch_page(&self, request: &SyncRequest, cursor: SyncCursor) -> Result<FetchPage> {
        let kind = request.media_kind.unwrap_or(MediaKind::Anime);
        let page_size = clamp_page_size(request.page_size, 25);
        let path = kind_path(kind);

        let resp: ListResponse = self
            .client
            .get(&format!("/{path}"))
            .query(&[
                ("page", cursor.page.to_string()),
                ("limit", page_size.to_string()),
                ("sfw", "false".to_string()),
            ])
            .send()?
            .error_for_status()?
            .json()?;

        let items = resp
            .data
            .into_iter()
            .map(|m| into_canonical(m, kind))
            .collect::<Result<Vec<_>>>()?;

        let next_cursor = resp
            .pagination
            .and_then(|p| p.has_next_page)
            .unwrap_or(false)
            .then_some(SyncCursor {
                page: cursor.page + 1,
            });

        Ok(FetchPage { items, next_cursor })
    }

    fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<CanonicalMedia>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let kind = options.media_kind.unwrap_or(MediaKind::Anime);
        let limit = clamp_page_size(options.limit, 25);
        let path = kind_path(kind);

        let resp: ListResponse = self
            .client
            .get(&format!("/{path}"))
            .query(&[
                ("q", query.to_string()),
                ("limit", limit.to_string()),
                ("sfw", "false".to_string()),
            ])
            .send()?
            .error_for_status()?
            .json()?;

        let mut items = resp
            .data
            .into_iter()
            .map(|m| into_canonical(m, kind))
            .collect::<Result<Vec<_>>>()?;

        if let Some(fmt) = options.format {
            items.retain(|m| m.format.as_ref().map(|v| v.eq_ignore_ascii_case(&fmt)) == Some(true));
        }

        Ok(items)
    }

    fn get_by_id(&self, media_kind: MediaKind, source_id: &str) -> Result<Option<CanonicalMedia>> {
        let mal_id: i64 = source_id
            .parse()
            .map_err(|_| Error::Validation(format!("invalid Jikan/MAL id: {source_id}")))?;

        let path = kind_path(media_kind);
        let resp = self.client.get(&format!("/{path}/{mal_id}")).send()?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let resp: ItemResponse = resp.error_for_status()?.json()?;
        match resp.data {
            Some(m) => Ok(Some(into_canonical(m, media_kind)?)),
            None => Ok(None),
        }
    }

    fn fetch_trending(&self, kind: MediaKind) -> Result<Vec<CanonicalMedia>> {
        let path = kind_path(kind);
        let resp: ListResponse = self
            .client
            .get(&format!("/top/{path}"))
            .query(&[("sfw", "false")])
            .send()?
            .error_for_status()?
            .json()?;

        resp.data
            .into_iter()
            .map(|m| into_canonical(m, kind))
            .collect()
    }

    fn fetch_recommendations(
        &self,
        media_kind: MediaKind,
        source_id: &str,
    ) -> Result<Vec<CanonicalMedia>> {
        let mal_id: i64 = source_id
            .parse()
            .map_err(|_| Error::Validation(format!("invalid Jikan/MAL id: {source_id}")))?;

        let path = kind_path(media_kind);
        let resp = self
            .client
            .get(&format!("/{path}/{mal_id}/recommendations"))
            .send()?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }

        let resp: RecommendationsResponse = resp.error_for_status()?.json()?;
        Ok(resp
            .data
            .into_iter()
            .map(|r| partial_entry_into_canonical(r.entry, media_kind))
            .collect())
    }

    fn fetch_related(&self, media_kind: MediaKind, source_id: &str) -> Result<Vec<CanonicalMedia>> {
        let mal_id: i64 = source_id
            .parse()
            .map_err(|_| Error::Validation(format!("invalid Jikan/MAL id: {source_id}")))?;

        let path = kind_path(media_kind);
        let resp = self
            .client
            .get(&format!("/{path}/{mal_id}/relations"))
            .send()?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }

        let resp: RelationsResponse = resp.error_for_status()?.json()?;
        Ok(resp
            .data
            .into_iter()
            .flat_map(|group| group.entry)
            .map(relation_entry_into_canonical)
            .collect())
    }
}

// ---------------------------------------------------------------------------
// Domain helpers (private)
// ---------------------------------------------------------------------------

fn kind_path(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Anime | MediaKind::Show => "anime",
        MediaKind::Manga | MediaKind::Movie => "manga",
    }
}

/// Returns `true` if the Jikan rating string indicates NSFW content.
fn is_nsfw(rating: Option<&str>) -> bool {
    matches!(
        rating,
        Some(r) if r.contains("Rx") || r.contains("Hentai") || r.contains("R+") || r.contains("Mild Nudity")
    )
}

fn into_canonical(item: Media, kind: MediaKind) -> Result<CanonicalMedia> {
    let raw = serde_json::to_value(&item)?;

    let title_display = item
        .title_english
        .clone()
        .or_else(|| item.title.clone())
        .or_else(|| item.title_japanese.clone())
        .ok_or_else(|| {
            Error::Validation(format!("Jikan media {} has no usable title", item.mal_id))
        })?;

    let mut aliases: Vec<String> = item.titles.iter().filter_map(|t| t.title.clone()).collect();
    aliases.extend(item.title_synonyms.clone());
    if let Some(t) = item.title.clone() {
        aliases.push(t);
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
        media_kind: kind,
        title_display,
        title_romaji: item.title.clone(),
        title_english: item.title_english,
        title_native: item.title_japanese,
        synopsis: item.synopsis,
        format: item.media_type,
        status: item.status,
        season: item.season,
        season_year: item.year,
        episodes: item.episodes,
        chapters: item.chapters,
        volumes: item.volumes,
        country_of_origin: None,
        cover_image: item
            .images
            .jpg
            .large_image_url
            .or(item.images.jpg.image_url),
        banner_image: item
            .trailer
            .as_ref()
            .and_then(|t| t.images.maximum_image_url.clone()),
        provider_rating: item.score.map(|s| (s / 10.0).clamp(0.0, 1.0)),
        nsfw: is_nsfw(item.rating.as_deref()),
        aliases,
        genres: item.genres.into_iter().map(|g| g.name).collect(),
        tags: item
            .themes
            .into_iter()
            .chain(item.demographics)
            .map(|g| g.name)
            .collect(),
        external_ids,
        source_payloads: vec![SourcePayload {
            source: SourceName::Jikan,
            source_id: item.mal_id.to_string(),
            url: item.url,
            remote_updated_at: None,
            raw_json: Some(raw),
        }],
        field_provenance: Vec::new(),
    })
}

fn partial_entry_into_canonical(entry: PartialEntry, media_kind: MediaKind) -> CanonicalMedia {
    let external_ids = vec![ExternalId {
        source: SourceName::MyAnimeList,
        source_id: entry.mal_id.to_string(),
        url: Some(entry.url.clone()),
    }];

    let cover_image = entry
        .images
        .as_ref()
        .and_then(|i| i.jpg.large_image_url.clone().or(i.jpg.image_url.clone()));

    CanonicalMedia {
        media_kind,
        title_display: entry.title.clone(),
        title_romaji: Some(entry.title),
        title_english: None,
        title_native: None,
        synopsis: None,
        format: None,
        status: None,
        season: None,
        season_year: None,
        episodes: None,
        chapters: None,
        volumes: None,
        country_of_origin: None,
        cover_image,
        banner_image: None,
        provider_rating: None,
        nsfw: false,
        aliases: Vec::new(),
        genres: Vec::new(),
        tags: Vec::new(),
        external_ids,
        source_payloads: vec![SourcePayload {
            source: SourceName::Jikan,
            source_id: entry.mal_id.to_string(),
            url: Some(entry.url),
            remote_updated_at: None,
            raw_json: None,
        }],
        field_provenance: Vec::new(),
    }
}

fn relation_entry_into_canonical(entry: RelationEntry) -> CanonicalMedia {
    let media_kind = match entry.kind.as_str() {
        "manga" => MediaKind::Manga,
        _ => MediaKind::Anime,
    };

    let external_ids = vec![ExternalId {
        source: SourceName::MyAnimeList,
        source_id: entry.mal_id.to_string(),
        url: Some(entry.url.clone()),
    }];

    CanonicalMedia {
        media_kind,
        title_display: entry.name.clone(),
        title_romaji: Some(entry.name),
        title_english: None,
        title_native: None,
        synopsis: None,
        format: None,
        status: None,
        season: None,
        season_year: None,
        episodes: None,
        chapters: None,
        volumes: None,
        country_of_origin: None,
        cover_image: None,
        banner_image: None,
        provider_rating: None,
        nsfw: false,
        aliases: Vec::new(),
        genres: Vec::new(),
        tags: Vec::new(),
        external_ids,
        source_payloads: vec![SourcePayload {
            source: SourceName::Jikan,
            source_id: entry.mal_id.to_string(),
            url: Some(entry.url),
            remote_updated_at: None,
            raw_json: None,
        }],
        field_provenance: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Jikan API Schemas (private) response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ListResponse {
    #[serde(default)]
    data: Vec<Media>,
    pagination: Option<Pagination>,
}

#[derive(Debug, Deserialize)]
struct ItemResponse {
    data: Option<Media>,
}

#[derive(Debug, Deserialize)]
struct Pagination {
    has_next_page: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Media {
    mal_id: i64,
    url: Option<String>,
    title: Option<String>,
    title_english: Option<String>,
    title_japanese: Option<String>,
    #[serde(default)]
    title_synonyms: Vec<String>,
    #[serde(default)]
    titles: Vec<AltTitle>,
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
    images: Images,
    trailer: Option<Trailer>,
    #[serde(default)]
    genres: Vec<NamedValue>,
    #[serde(default)]
    themes: Vec<NamedValue>,
    #[serde(default)]
    demographics: Vec<NamedValue>,
    updated_at: Option<UpdatedAt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AltTitle {
    #[serde(rename = "type")]
    _kind: Option<String>,
    title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Images {
    jpg: ImageSet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImageSet {
    image_url: Option<String>,
    large_image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Trailer {
    images: TrailerImages,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrailerImages {
    maximum_image_url: Option<String>,
    large_image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NamedValue {
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UpdatedAt {
    // We just ignore the contents of `updated_at` since it's an opaque struct or date string
    // depending on the endpoint.
}

#[derive(Debug, Deserialize)]
struct RecommendationsResponse {
    data: Vec<RecommendationItem>,
}

#[derive(Debug, Deserialize)]
struct RecommendationItem {
    entry: PartialEntry,
}

#[derive(Debug, Deserialize)]
struct PartialEntry {
    mal_id: i64,
    url: String,
    images: Option<Images>,
    title: String,
}

#[derive(Debug, Deserialize)]
struct RelationsResponse {
    data: Vec<RelationGroup>,
}

#[derive(Debug, Deserialize)]
struct RelationGroup {
    entry: Vec<RelationEntry>,
}

#[derive(Debug, Deserialize)]
struct RelationEntry {
    mal_id: i64,
    #[serde(rename = "type")]
    kind: String,
    name: String,
    url: String,
}
