/// AniList GraphQL provider.
///
/// Implements [`Provider`](super::Provider) for the AniList v2 GraphQL API.
/// All AniList-specific types (response structs, queries) are private to this
/// module, keeping the public surface minimal.
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::model::{
    CanonicalMedia, ExternalId, MediaKind, SearchOptions, SourceName, SourcePayload, SyncCursor,
    SyncRequest,
};

use super::http::{HttpClient, clamp_page_size, with_retry};
use super::{FetchPage, Provider};

// ---------------------------------------------------------------------------
// Provider struct
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AniListProvider {
    client: HttpClient,
}

impl Default for AniListProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AniListProvider {
    pub const DEFAULT_ENDPOINT: &'static str = "https://graphql.anilist.co";

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

    fn post_with_retry(&self, payload: &serde_json::Value) -> Result<reqwest::blocking::Response> {
        let client = self.client.client()?;
        let url = self.client.base_url.clone();
        let payload = payload.clone();

        with_retry(3, Duration::from_secs(2), move || {
            let req = client.post(&url).json(&payload).build()?;
            Ok(client.execute(req)?)
        })
    }
}

// ---------------------------------------------------------------------------
// Provider impl
// ---------------------------------------------------------------------------

impl Provider for AniListProvider {
    fn source(&self) -> SourceName {
        SourceName::AniList
    }

    fn min_interval(&self) -> Duration {
        Duration::from_millis(700)
    }

    fn fetch_page(&self, request: &SyncRequest, cursor: SyncCursor) -> Result<FetchPage> {
        let kind = request.media_kind.unwrap_or(MediaKind::Anime);
        let page_size = clamp_page_size(request.page_size, 50);

        let payload = json!({
            "query": queries::PAGE,
            "variables": {
                "page": cursor.page as i64,
                "perPage": page_size as i64,
                "type": media_kind_str(kind),
                "sort": ["ID"]
            }
        });

        let resp = self
            .post_with_retry(&payload)?
            .json::<GraphQlResponse<PageData>>()?;

        if is_anilist_page_exhausted(&resp.errors) {
            eprintln!(
                "AniList: page {} exceeds 100-page hard limit — ending sync",
                cursor.page,
            );
            return Ok(FetchPage { items: Vec::new(), next_cursor: None });
        }

        check_errors(&resp.errors)?;

        let page = resp
            .data
            .ok_or_else(|| Error::Validation("AniList response missing data".into()))?
            .page;

        let items = page
            .media
            .into_iter()
            .map(|m| into_canonical(m, kind))
            .collect::<Result<Vec<_>>>()?;

        let next_cursor = page.page_info.has_next_page.then_some(SyncCursor {
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
        let limit = clamp_page_size(options.limit, 50);

        let payload = json!({
            "query": queries::SEARCH,
            "variables": {
                "page": 1,
                "perPage": limit as i64,
                "type": media_kind_str(kind),
                "search": query,
            }
        });

        let resp = self
            .post_with_retry(&payload)?
            .json::<GraphQlResponse<PageData>>()?;

        check_errors(&resp.errors)?;

        let page = resp
            .data
            .ok_or_else(|| Error::Validation("AniList response missing data".into()))?
            .page;

        let mut items = page
            .media
            .into_iter()
            .map(|m| into_canonical(m, kind))
            .collect::<Result<Vec<_>>>()?;

        if let Some(fmt) = options.format {
            items.retain(|m| m.format.as_ref().map(|v| v.eq_ignore_ascii_case(&fmt)) == Some(true));
        }

        Ok(items)
    }

    fn get_by_id(&self, media_kind: MediaKind, source_id: &str) -> Result<Option<CanonicalMedia>> {
        let media_id: i64 = source_id
            .parse()
            .map_err(|_| Error::Validation(format!("invalid AniList id: {source_id}")))?;

        let payload = json!({
            "query": queries::BY_ID,
            "variables": {
                "id": media_id,
                "type": media_kind_str(media_kind),
            }
        });

        let resp = self
            .post_with_retry(&payload)?
            .json::<GraphQlResponse<SingleMediaData>>()?;

        check_errors(&resp.errors)?;

        let Some(data) = resp.data else {
            return Ok(None);
        };
        let Some(media) = data.media else {
            return Ok(None);
        };

        Ok(Some(into_canonical(media, media_kind)?))
    }

    fn fetch_trending(&self, media_kind: MediaKind) -> Result<Vec<CanonicalMedia>> {
        let payload = json!({
            "query": queries::PAGE,
            "variables": {
                "page": 1,
                "perPage": 50,
                "type": media_kind_str(media_kind),
                "sort": ["TRENDING_DESC"]
            }
        });

        let resp = self
            .post_with_retry(&payload)?
            .json::<GraphQlResponse<PageData>>()?;

        check_errors(&resp.errors)?;

        let page = resp
            .data
            .ok_or_else(|| Error::Validation("AniList response missing data".into()))?
            .page;

        page.media
            .into_iter()
            .map(|m| into_canonical(m, media_kind))
            .collect()
    }

    fn fetch_recommendations(
        &self,
        media_kind: MediaKind,
        source_id: &str,
    ) -> Result<Vec<CanonicalMedia>> {
        let media_id: i64 = source_id
            .parse()
            .map_err(|_| Error::Validation(format!("invalid AniList id: {source_id}")))?;

        let payload = json!({
            "query": queries::RECOMMENDATIONS,
            "variables": {
                "id": media_id,
                "type": media_kind_str(media_kind),
            }
        });

        let resp = self
            .post_with_retry(&payload)?
            .json::<GraphQlResponse<SingleMediaData>>()?;

        check_errors(&resp.errors)?;

        let Some(data) = resp.data else {
            return Ok(Vec::new());
        };
        let Some(media) = data.media else {
            return Ok(Vec::new());
        };
        let Some(recs) = media.recommendations else {
            return Ok(Vec::new());
        };

        recs.nodes
            .into_iter()
            .filter_map(|n| n.media_recommendation)
            .map(|m| into_canonical(m, media_kind))
            .collect()
    }

    fn fetch_related(&self, media_kind: MediaKind, source_id: &str) -> Result<Vec<CanonicalMedia>> {
        let media_id: i64 = source_id
            .parse()
            .map_err(|_| Error::Validation(format!("invalid AniList id: {source_id}")))?;

        let payload = json!({
            "query": queries::RELATED,
            "variables": {
                "id": media_id,
                "type": media_kind_str(media_kind),
            }
        });

        let resp = self
            .post_with_retry(&payload)?
            .json::<GraphQlResponse<SingleMediaData>>()?;

        check_errors(&resp.errors)?;

        let Some(data) = resp.data else {
            return Ok(Vec::new());
        };
        let Some(media) = data.media else {
            return Ok(Vec::new());
        };
        let Some(rels) = media.relations else {
            return Ok(Vec::new());
        };

        rels.edges
            .into_iter()
            .filter_map(|e| e.node)
            .map(|m| into_canonical(m, media_kind))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Domain helpers (private)
// ---------------------------------------------------------------------------

fn media_kind_str(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Anime | MediaKind::Show | MediaKind::Movie => "ANIME",
        MediaKind::Manga => "MANGA",
    }
}

fn check_errors(errors: &[ApiError]) -> Result<()> {
    if errors.is_empty() {
        return Ok(());
    }
    let msg = errors
        .iter()
        .map(|e| e.message.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Err(Error::Validation(format!("AniList API errors: {msg}")))
}

fn is_anilist_page_exhausted(errors: &[ApiError]) -> bool {
    errors.iter().any(|e| e.message.contains("Page must be between 1 and 100"))
}

fn into_canonical(item: Media, kind: MediaKind) -> Result<CanonicalMedia> {
    let raw = serde_json::to_value(&item)?;

    let title_display = item
        .title
        .english
        .clone()
        .or_else(|| item.title.romaji.clone())
        .or_else(|| item.title.native.clone())
        .ok_or_else(|| {
            Error::Validation(format!("AniList media {} has no usable title", item.id))
        })?;

    let mut external_ids = vec![ExternalId {
        source: SourceName::AniList,
        source_id: item.id.to_string(),
        url: item.site_url.clone(),
    }];

    if let Some(mal_id) = item.id_mal {
        external_ids.push(ExternalId {
            source: SourceName::MyAnimeList,
            source_id: mal_id.to_string(),
            url: None,
        });
    }

    Ok(CanonicalMedia {
        media_kind: kind,
        title_display,
        title_romaji: item.title.romaji,
        title_english: item.title.english,
        title_native: item.title.native,
        synopsis: item.description,
        format: item.format,
        status: item.status,
        season: item.season.map(|s| s.to_ascii_lowercase()),
        season_year: item.season_year,
        episodes: item.episodes,
        chapters: item.chapters,
        volumes: item.volumes,
        country_of_origin: item.country_of_origin,
        cover_image: item.cover_image.as_ref().and_then(|c| {
            c.extra_large
                .clone()
                .or_else(|| c.large.clone())
                .or_else(|| c.medium.clone())
        }),
        banner_image: item.banner_image,
        provider_rating: item.average_score.map(|s| (s / 100.0).clamp(0.0, 1.0)),
        nsfw: item.is_adult.unwrap_or(false),
        aliases: item.synonyms,
        genres: item.genres,
        tags: item.tags.into_iter().map(|t| t.name).collect(),
        external_ids,
        source_payloads: vec![SourcePayload {
            source: SourceName::AniList,
            source_id: item.id.to_string(),
            url: item.site_url,
            remote_updated_at: item.updated_at.map(|ts| ts.to_string()),
            raw_json: Some(raw),
        }],
        field_provenance: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// GraphQL queries (private constant strings)
// ---------------------------------------------------------------------------

mod queries {
    pub const PAGE: &str = r#"
query ($page: Int, $perPage: Int, $type: MediaType, $sort: [MediaSort]) {
  Page(page: $page, perPage: $perPage) {
    pageInfo { hasNextPage }
    media(type: $type, sort: $sort) {
      id idMal
      title { romaji english native }
      synonyms
      description(asHtml: false)
      format status episodes chapters volumes
      countryOfOrigin season seasonYear
      genres averageScore updatedAt siteUrl isAdult bannerImage
      coverImage { medium large extraLarge }
      tags { name }
    }
  }
}
"#;

    pub const SEARCH: &str = r#"
query ($page: Int, $perPage: Int, $type: MediaType, $search: String) {
  Page(page: $page, perPage: $perPage) {
    media(type: $type, search: $search) {
      id idMal
      title { romaji english native }
      synonyms
      description(asHtml: false)
      format status episodes chapters volumes
      countryOfOrigin season seasonYear
      genres averageScore updatedAt siteUrl isAdult bannerImage
      coverImage { medium large extraLarge }
      tags { name }
    }
  }
}
"#;

    pub const BY_ID: &str = r#"
query ($id: Int, $type: MediaType) {
  Media(id: $id, type: $type) {
    id idMal
    title { romaji english native }
    synonyms
    description(asHtml: false)
    format status episodes chapters volumes
    countryOfOrigin season seasonYear
    genres averageScore updatedAt siteUrl isAdult bannerImage
    coverImage { medium large extraLarge }
    tags { name }
  }
}
"#;
    pub const RECOMMENDATIONS: &str = r#"
query ($id: Int, $type: MediaType) {
  Media(id: $id, type: $type) {
    recommendations {
      nodes {
        mediaRecommendation {
          id idMal
          title { romaji english native }
          synonyms
          description(asHtml: false)
          format status episodes chapters volumes
          countryOfOrigin season seasonYear
          genres averageScore updatedAt siteUrl isAdult bannerImage
          coverImage { medium large extraLarge }
          tags { name }
        }
      }
    }
  }
}
"#;

    pub const RELATED: &str = r#"
query ($id: Int, $type: MediaType) {
  Media(id: $id, type: $type) {
    relations {
      edges {
        node {
          id idMal
          title { romaji english native }
          synonyms
          description(asHtml: false)
          format status episodes chapters volumes
          countryOfOrigin season seasonYear
          genres averageScore updatedAt siteUrl isAdult bannerImage
          coverImage { medium large extraLarge }
          tags { name }
        }
      }
    }
  }
}
"#;
}

// ---------------------------------------------------------------------------
// Private response types — never leak across the module boundary
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GraphQlResponse<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<ApiError>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct PageData {
    #[serde(rename = "Page")]
    page: Page,
}

#[derive(Debug, Deserialize)]
struct SingleMediaData {
    #[serde(rename = "Media")]
    media: Option<Media>,
}

#[derive(Debug, Deserialize)]
struct Page {
    #[serde(rename = "pageInfo", default)]
    page_info: PageInfo,
    #[serde(default)]
    media: Vec<Media>,
}

#[derive(Debug, Default, Deserialize)]
struct PageInfo {
    #[serde(rename = "hasNextPage", default)]
    has_next_page: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Media {
    id: i64,
    #[serde(rename = "idMal")]
    id_mal: Option<i64>,
    title: Title,
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
    cover_image: Option<CoverImage>,
    #[serde(default)]
    tags: Vec<Tag>,
    #[serde(default, skip_serializing)]
    recommendations: Option<RecommendationsConnection>,
    #[serde(default, skip_serializing)]
    relations: Option<RelationsConnection>,
}

#[derive(Debug, Clone, Deserialize)]
struct RecommendationsConnection {
    #[serde(default)]
    nodes: Vec<RecommendationNode>,
}

#[derive(Debug, Clone, Deserialize)]
struct RecommendationNode {
    #[serde(rename = "mediaRecommendation")]
    media_recommendation: Option<Media>,
}

#[derive(Debug, Clone, Deserialize)]
struct RelationsConnection {
    #[serde(default)]
    edges: Vec<RelationEdge>,
}

#[derive(Debug, Clone, Deserialize)]
struct RelationEdge {
    node: Option<Media>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Title {
    romaji: Option<String>,
    english: Option<String>,
    native: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Tag {
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CoverImage {
    medium: Option<String>,
    large: Option<String>,
    #[serde(rename = "extraLarge")]
    extra_large: Option<String>,
}
