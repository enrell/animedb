/// Kitsu REST/JSON:API provider.
///
/// Implements [`Provider`](super::Provider) for `kitsu.io/api/edge`.
///
/// # Note on `kitsu_entities`
///
/// The old `kitsu_entities.rs` (1 565 lines) exposed a giant `KitsuEntities`
/// helper that mirrors every endpoint of the Kitsu API.  That helper is only
/// used by external consumers who want raw Kitsu entity access (e.g.
/// streaming links, castings, etc.) — not by the core sync/search pipeline.
///
/// Those types are preserved in the companion `kitsu::entities` submodule
/// and can be reached via `animedb::provider::kitsu::entities`.  The public
/// surface of **this** module is exclusively the `KitsuProvider` struct and
/// its `Provider` impl.
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::{Error, Result};
use crate::model::{
    CanonicalEpisode, CanonicalMedia, ExternalId, MediaKind, SearchOptions, SourceName,
    SourcePayload, SyncCursor, SyncRequest,
};

use super::http::{HttpClient, clamp_page_size, page_to_offset};
use super::{FetchPage, Provider};

// ---------------------------------------------------------------------------
// Provider struct
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct KitsuProvider {
    client: HttpClient,
}

impl Default for KitsuProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KitsuProvider {
    pub const DEFAULT_ENDPOINT: &'static str = "https://kitsu.io/api/edge";

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

    pub fn with_proxy(mut self, proxy_url: impl Into<String>) -> Self {
        self.client = self.client.with_proxy(proxy_url);
        self
    }
}

// ---------------------------------------------------------------------------
// Provider impl
// ---------------------------------------------------------------------------

impl Provider for KitsuProvider {
    fn source(&self) -> SourceName {
        SourceName::Kitsu
    }

    fn min_interval(&self) -> Duration {
        Duration::from_millis(900)
    }

    fn fetch_page(&self, request: &SyncRequest, cursor: SyncCursor) -> Result<FetchPage> {
        let kind = request.media_kind.unwrap_or(MediaKind::Anime);
        let page_size = clamp_page_size(request.page_size, 20);
        let offset = page_to_offset(cursor.page, page_size);
        let path = kind_path(kind);

        let resp: CollectionResponse = self
            .client
            .get(&format!("/{path}"))
            .header("Accept", "application/vnd.api+json")
            .query(&[
                ("page[limit]", page_size.to_string()),
                ("page[offset]", offset.to_string()),
                ("sort", "id".to_string()),
                ("include", "categories,mappings".to_string()),
            ])
            .send()?
            .error_for_status()?
            .json()?;

        let items = resp
            .data
            .iter()
            .map(|r| into_canonical(r, &resp.included, kind))
            .collect::<Result<Vec<_>>>()?;

        let next_cursor = resp.links.next.is_some().then_some(SyncCursor {
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
        let limit = clamp_page_size(options.limit, 20);
        let path = kind_path(kind);

        let resp: CollectionResponse = self
            .client
            .get(&format!("/{path}"))
            .header("Accept", "application/vnd.api+json")
            .query(&[
                ("filter[text]", query.to_string()),
                ("page[limit]", limit.to_string()),
                ("page[offset]", options.offset.to_string()),
                ("include", "categories,mappings".to_string()),
            ])
            .send()?
            .error_for_status()?
            .json()?;

        let mut items = resp
            .data
            .iter()
            .map(|r| into_canonical(r, &resp.included, kind))
            .collect::<Result<Vec<_>>>()?;

        if let Some(fmt) = options.format {
            items.retain(|m| m.format.as_ref().map(|v| v.eq_ignore_ascii_case(&fmt)) == Some(true));
        }

        Ok(items)
    }

    fn get_by_id(&self, media_kind: MediaKind, source_id: &str) -> Result<Option<CanonicalMedia>> {
        let path = kind_path(media_kind);
        let resp = self
            .client
            .get(&format!("/{path}/{source_id}"))
            .header("Accept", "application/vnd.api+json")
            .query(&[("include", "categories,mappings")])
            .send()?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let resp: ItemResponse = resp.error_for_status()?.json()?;
        match resp.data {
            Some(ref r) => Ok(Some(into_canonical(r, &resp.included, media_kind)?)),
            None => Ok(None),
        }
    }

    fn fetch_trending(&self, kind: MediaKind) -> Result<Vec<CanonicalMedia>> {
        let path = trending_path(kind);
        let resp: CollectionResponse = self
            .client
            .get(&format!("/{path}"))
            .header("Accept", "application/vnd.api+json")
            .send()?
            .error_for_status()?
            .json()?;

        resp.data
            .iter()
            .map(|r| into_canonical(r, &resp.included, kind))
            .collect()
    }

    fn fetch_episodes(
        &self,
        media_kind: MediaKind,
        source_id: &str,
    ) -> Result<Vec<CanonicalEpisode>> {
        let path = format!("{}/{}/episodes", kind_path(media_kind), source_id);
        let resp: EpisodeCollectionResponse = self
            .client
            .get(&format!("/{path}"))
            .header("Accept", "application/vnd.api+json")
            .query(&[("page[limit]", "20")])
            .send()?
            .error_for_status()?
            .json()?;

        resp.data
            .iter()
            .map(|r| into_episode(r, media_kind))
            .collect()
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

fn trending_path(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Anime | MediaKind::Show => "trending/anime",
        MediaKind::Manga | MediaKind::Movie => "trending/manga",
    }
}

/// Picks the best available URL from a `KitsuImageSet`.
fn best_image(img: &KitsuImageSet) -> Option<String> {
    img.original
        .clone()
        .or_else(|| img.large.clone())
        .or_else(|| img.medium.clone())
        .or_else(|| img.small.clone())
        .or_else(|| img.tiny.clone())
}

fn into_canonical(
    resource: &Resource,
    included: &[Included],
    kind: MediaKind,
) -> Result<CanonicalMedia> {
    let raw = serde_json::to_value(resource)?;
    let attrs = &resource.attributes;

    let title_display = attrs
        .canonical_title
        .clone()
        .or_else(|| attrs.titles.en.clone())
        .or_else(|| attrs.titles.en_jp.clone())
        .or_else(|| attrs.titles.ja_jp.clone())
        .ok_or_else(|| {
            Error::Validation(format!("Kitsu media {} has no usable title", resource.id))
        })?;

    // Build alias list from all title variants.
    let mut aliases = attrs.abbreviated_titles.clone();
    for title in [&attrs.titles.en, &attrs.titles.en_jp, &attrs.titles.ja_jp]
        .into_iter()
        .flatten()
    {
        aliases.push(title.clone());
    }

    // Extract categories and external mappings from the sideloaded `included`.
    let tags = extract_categories(resource, included);
    let mappings = extract_mappings(resource, included);

    let mut external_ids = vec![ExternalId {
        source: SourceName::Kitsu,
        source_id: resource.id.clone(),
        url: Some(format!("https://kitsu.io/{}", kind_path(kind))),
    }];
    for m in mappings {
        if let Some(ext_id) = m.external_id
            && m.external_site.to_ascii_lowercase().contains("myanimelist")
        {
            external_ids.push(ExternalId {
                source: SourceName::MyAnimeList,
                source_id: ext_id,
                url: None,
            });
        }
    }

    Ok(CanonicalMedia {
        media_kind: kind,
        title_display,
        title_romaji: attrs.titles.en_jp.clone(),
        title_english: attrs.titles.en.clone(),
        title_native: attrs.titles.ja_jp.clone(),
        synopsis: attrs.synopsis.clone().or_else(|| attrs.description.clone()),
        format: attrs.subtype.clone(),
        status: attrs.status.clone(),
        season: None,
        season_year: attrs
            .start_date
            .as_deref()
            .and_then(|d| d.get(0..4)?.parse().ok()),
        episodes: attrs.episode_count,
        chapters: attrs.chapter_count,
        volumes: attrs.volume_count,
        country_of_origin: None,
        cover_image: attrs.poster_image.as_ref().and_then(best_image),
        banner_image: attrs.cover_image.as_ref().and_then(best_image),
        provider_rating: attrs
            .average_rating
            .as_deref()
            .and_then(|r| r.parse::<f64>().ok())
            .map(|r| (r / 100.0).clamp(0.0, 1.0)),
        nsfw: attrs.nsfw.unwrap_or(false) || matches!(attrs.age_rating.as_deref(), Some("R18")),
        aliases,
        genres: Vec::new(),
        tags,
        external_ids,
        source_payloads: vec![SourcePayload {
            source: SourceName::Kitsu,
            source_id: resource.id.clone(),
            url: Some(format!("https://kitsu.io/{}", kind_path(kind))),
            remote_updated_at: attrs.updated_at.clone(),
            raw_json: Some(raw),
        }],
        field_provenance: Vec::new(),
    })
}

/// Returns category titles for the given `resource` from sideloaded data.
fn extract_categories(resource: &Resource, included: &[Included]) -> Vec<String> {
    let Some(rels) = resource.relationships.as_ref() else {
        return Vec::new();
    };
    let Some(cats) = rels.categories.as_ref() else {
        return Vec::new();
    };

    cats.data
        .iter()
        .filter_map(|r| {
            included.iter().find_map(|inc| match inc {
                Included::Category(c) if c.id == r.id && r.kind == "categories" => {
                    c.attributes.title.clone()
                }
                _ => None,
            })
        })
        .collect()
}

fn extract_mappings(resource: &Resource, included: &[Included]) -> Vec<MappingAttributes> {
    let Some(rels) = resource.relationships.as_ref() else {
        return Vec::new();
    };
    let Some(maps) = rels.mappings.as_ref() else {
        return Vec::new();
    };

    maps.data
        .iter()
        .filter_map(|r| {
            included.iter().find_map(|inc| match inc {
                Included::Mapping(m) if m.id == r.id && r.kind == "mappings" => {
                    Some(m.attributes.clone())
                }
                _ => None,
            })
        })
        .collect()
}

fn into_episode(resource: &EpisodeResource, media_kind: MediaKind) -> Result<CanonicalEpisode> {
    let raw = serde_json::to_value(resource)?;
    let attrs = &resource.attributes;

    let titles_json = serde_json::to_string(&attrs.titles)?;

    Ok(CanonicalEpisode {
        source: SourceName::Kitsu,
        source_id: resource.id.clone(),
        media_kind,
        season_number: attrs.season_number,
        episode_number: attrs.number,
        absolute_number: attrs.relative_number,
        title_display: attrs
            .titles
            .en
            .clone()
            .or_else(|| attrs.titles.en_jp.clone()),
        title_original: attrs.titles.ja_jp.clone(),
        synopsis: attrs.synopsis.clone().or_else(|| attrs.description.clone()),
        air_date: attrs.airdate.clone(),
        runtime_minutes: attrs.runtime,
        thumbnail_url: attrs
            .thumbnail
            .as_ref()
            .and_then(|t| t.original.clone().or(t.small.clone())),
        raw_titles_json: Some(
            serde_json::from_str(&titles_json).unwrap_or(serde_json::Value::Null),
        ),
        raw_json: Some(raw),
    })
}

// ---------------------------------------------------------------------------
// Private response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct CollectionResponse {
    #[serde(default)]
    data: Vec<Resource>,
    #[serde(default)]
    included: Vec<Included>,
    #[serde(default)]
    links: Links,
}

#[derive(Debug, Deserialize)]
struct ItemResponse {
    data: Option<Resource>,
    #[serde(default)]
    included: Vec<Included>,
}

#[derive(Debug, Default, Deserialize)]
struct Links {
    next: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Resource {
    pub id: String,
    #[serde(rename = "type")]
    pub _type: String,
    pub attributes: MediaAttributes,
    pub relationships: Option<Relationships>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MediaAttributes {
    #[serde(rename = "updatedAt")]
    updated_at: Option<String>,
    description: Option<String>,
    synopsis: Option<String>,
    #[serde(rename = "canonicalTitle")]
    canonical_title: Option<String>,
    titles: Titles,
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
struct Titles {
    en: Option<String>,
    en_jp: Option<String>,
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
struct Relationships {
    categories: Option<RelationshipCollection>,
    mappings: Option<RelationshipCollection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RelationshipCollection {
    #[serde(default)]
    data: Vec<RelationshipRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RelationshipRef {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum Included {
    #[serde(rename = "categories")]
    Category(CategoryResource),
    #[serde(rename = "mappings")]
    Mapping(MappingResource),
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CategoryResource {
    id: String,
    attributes: CategoryAttributes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CategoryAttributes {
    title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MappingResource {
    id: String,
    attributes: MappingAttributes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MappingAttributes {
    #[serde(rename = "externalSite")]
    external_site: String,
    #[serde(rename = "externalId")]
    external_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Episode response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct EpisodeCollectionResponse {
    #[serde(default)]
    data: Vec<EpisodeResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EpisodeResource {
    pub id: String,
    #[serde(rename = "type")]
    pub _type: String,
    pub attributes: EpisodeAttributes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EpisodeAttributes {
    titles: EpisodeTitles,
    description: Option<String>,
    synopsis: Option<String>,
    number: Option<i32>,
    #[serde(rename = "relativeNumber")]
    relative_number: Option<i32>,
    #[serde(rename = "airdate")]
    airdate: Option<String>,
    runtime: Option<i32>,
    #[serde(rename = "seasonNumber")]
    season_number: Option<i32>,
    thumbnail: Option<KitsuImageSet>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct EpisodeTitles {
    en: Option<String>,
    en_jp: Option<String>,
    ja_jp: Option<String>,
}
