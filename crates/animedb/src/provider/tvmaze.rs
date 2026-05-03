/// TVmaze REST provider.
///
/// Implements [`Provider`](super::Provider) for `api.tvmaze.com`.
/// All TVmaze-specific response types are private to this module.
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
pub struct TvmazeProvider {
    client: HttpClient,
}

impl Default for TvmazeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TvmazeProvider {
    pub const DEFAULT_ENDPOINT: &'static str = "https://api.tvmaze.com";

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

impl Provider for TvmazeProvider {
    fn source(&self) -> SourceName {
        SourceName::Tvmaze
    }

    fn min_interval(&self) -> Duration {
        Duration::from_millis(500)
    }

    fn fetch_page(&self, request: &SyncRequest, cursor: SyncCursor) -> Result<FetchPage> {
        let _page_size = clamp_page_size(request.page_size, 250);

        let resp = self
            .client
            .get("/shows")
            .query(&[("page", cursor.page.to_string())])
            .send()?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(FetchPage {
                items: Vec::new(),
                next_cursor: None,
            });
        }

        let shows: Vec<Show> = resp.error_for_status()?.json()?;

        if shows.is_empty() {
            return Ok(FetchPage {
                items: Vec::new(),
                next_cursor: None,
            });
        }

        let items = shows.into_iter().filter_map(into_canonical).collect();
        let next_cursor = Some(SyncCursor { page: cursor.page + 1 });

        Ok(FetchPage { items, next_cursor })
    }

    fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<CanonicalMedia>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let limit = clamp_page_size(options.limit, 50);
        let results: Vec<SearchResult> = self
            .client
            .get("/search/shows")
            .query(&[("q", query.to_string()), ("limit", limit.to_string())])
            .send()?
            .error_for_status()?
            .json()?;

        Ok(results.into_iter().filter_map(|r| into_canonical(r.show)).collect())
    }

    fn get_by_id(&self, _media_kind: MediaKind, source_id: &str) -> Result<Option<CanonicalMedia>> {
        let resp = self.client.get(&format!("/shows/{source_id}")).send()?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let show: Show = resp.error_for_status()?.json()?;
        Ok(into_canonical(show))
    }

    fn fetch_trending(&self, media_kind: MediaKind) -> Result<Vec<CanonicalMedia>> {
        if media_kind != MediaKind::Show {
            return Err(Error::Validation("TVmaze only supports shows".into()));
        }

        let resp: Vec<Show> = self
            .client
            .get("/shows")
            .query(&[("page", "0")])
            .send()?
            .error_for_status()?
            .json()?;

        Ok(resp.into_iter().filter_map(into_canonical).collect())
    }
}

// ---------------------------------------------------------------------------
// Domain helpers (private)
// ---------------------------------------------------------------------------

fn into_canonical(show: Show) -> Option<CanonicalMedia> {
    let title_display = show.name.trim().to_string();
    if title_display.is_empty() {
        return None;
    }

    // Serialize early, before we start moving fields out of `show`.
    let raw_json = serde_json::to_value(&show).ok();

    let synopsis = show.summary.as_deref().map(strip_html);
    let cover_image = show
        .image
        .as_ref()
        .and_then(|img| img.original.clone().or_else(|| img.medium.clone()));

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

    let country = show
        .network
        .as_ref()
        .and_then(|n| n.country.as_ref())
        .or_else(|| show.web_channel.as_ref().and_then(|wc| wc.country.as_ref()))
        .and_then(|c| c.code.clone());

    let season_year = show
        .premiered
        .as_deref()
        .and_then(|d| d.get(0..4)?.parse().ok());

    let provider_rating = show
        .rating
        .as_ref()
        .and_then(|r| r.average)
        .map(|v| (v / 10.0).clamp(0.0, 1.0));

    let genres = show.genres.unwrap_or_default();

    Some(CanonicalMedia {
        media_kind: MediaKind::Show,
        title_display,
        title_romaji: None,
        title_english: None,
        title_native: None,
        synopsis,
        format: None,
        status: show.status,
        season: None,
        season_year,
        episodes: None,
        chapters: None,
        volumes: None,
        country_of_origin: country,
        cover_image,
        banner_image: None,
        provider_rating,
        nsfw: false,
        aliases: Vec::new(),
        genres,
        tags: Vec::new(),
        external_ids,
        source_payloads: vec![SourcePayload {
            source: SourceName::Tvmaze,
            source_id: show.id.to_string(),
            url: show.url,
            remote_updated_at: show.updated.map(|v| v.to_string()),
            raw_json,
        }],
        field_provenance: Vec::new(),
    })
}

/// Strips common HTML tags from TVmaze HTML-encoded summaries.
fn strip_html(html: &str) -> String {
    html.replace("<p>", "")
        .replace("</p>", "")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<b>", "")
        .replace("</b>", "")
        .replace("<i>", "")
        .replace("</i>", "")
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// Private response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Show {
    id: i64,
    url: Option<String>,
    name: String,
    genres: Option<Vec<String>>,
    status: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    premiered: Option<String>,
    #[serde(default)]
    image: Option<Image>,
    #[serde(default)]
    rating: Option<Rating>,
    #[serde(default)]
    network: Option<Network>,
    #[serde(default, rename = "webChannel")]
    web_channel: Option<WebChannel>,
    #[serde(default)]
    externals: Externals,
    #[serde(default)]
    updated: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchResult {
    #[allow(dead_code)]
    score: Option<f64>,
    show: Show,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Image {
    medium: Option<String>,
    original: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Rating {
    average: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Network {
    country: Option<Country>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebChannel {
    country: Option<Country>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Country {
    code: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Externals {
    #[serde(default)]
    imdb: Option<String>,
}