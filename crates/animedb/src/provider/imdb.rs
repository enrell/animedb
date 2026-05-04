/// IMDb dataset provider.
///
/// IMDb does not offer a public REST/GraphQL API.  Instead, this provider
/// downloads the gzip-compressed TSV dumps from `datasets.imdb.com` and
/// streams through them line-by-line to produce [`CanonicalMedia`] records.
///
/// Because each call to `fetch_page`, `search`, or `get_by_id` downloads
/// the full `title.basics.tsv.gz` dump (~100 MB), this provider is best
/// suited for **bulk sync** — not interactive search.
use flate2::read::GzDecoder;
use std::collections::HashMap;
use std::io::BufRead as _;
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
pub struct ImdbProvider {
    client: HttpClient,
}

impl Default for ImdbProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ImdbProvider {
    /// Default base URL for the IMDb dataset TSV files.
    pub const DEFAULT_BASE_URL: &'static str = "https://datasets.imdb.com";

    pub fn new() -> Self {
        // IMDb datasets are large; allow 5 minutes for the download.
        Self {
            client: HttpClient::new(Duration::from_secs(300), Self::DEFAULT_BASE_URL),
        }
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            client: HttpClient::new(Duration::from_secs(300), Self::DEFAULT_BASE_URL)
                .with_base_url(base_url),
        }
    }
}

// ---------------------------------------------------------------------------
// Provider impl
// ---------------------------------------------------------------------------

impl Provider for ImdbProvider {
    fn source(&self) -> SourceName {
        SourceName::Imdb
    }

    fn min_interval(&self) -> Duration {
        // Each call already downloads the full dataset — no additional
        // throttling is needed.
        Duration::ZERO
    }

    fn fetch_page(&self, request: &SyncRequest, cursor: SyncCursor) -> Result<FetchPage> {
        let kind = request.media_kind.unwrap_or(MediaKind::Movie);
        let page_size = clamp_page_size(request.page_size, 500);
        let skip = cursor.page.saturating_sub(1) * page_size;

        let basics = self.download("/title.basics.tsv.gz")?;
        let ratings = self.download("/title.ratings.tsv.gz")?;
        let ratings_map = parse_ratings(&ratings);

        let mut items = Vec::with_capacity(page_size);
        let mut line_index = 0usize;
        let mut consumed = 0usize;

        for_each_basics_row(&basics, |row| {
            if row.kind != kind {
                return RowAction::Skip;
            }

            line_index += 1;
            if line_index <= skip {
                return RowAction::Skip;
            }
            if consumed >= page_size {
                return RowAction::Stop;
            }

            let rating = ratings_map.get(row.tconst).copied();
            items.push(row.into_canonical(rating));
            consumed += 1;
            RowAction::Continue
        })?;

        let next_cursor = (consumed >= page_size).then_some(SyncCursor {
            page: cursor.page + 1,
        });

        Ok(FetchPage { items, next_cursor })
    }

    fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<CanonicalMedia>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let kind = options.media_kind.unwrap_or(MediaKind::Movie);
        let limit = clamp_page_size(options.limit, 100);
        let query_lower = query.to_ascii_lowercase();

        let basics = self.download("/title.basics.tsv.gz")?;
        let mut items = Vec::new();

        for_each_basics_row(&basics, |row| {
            if row.kind != kind {
                return RowAction::Skip;
            }
            if items.len() >= limit {
                return RowAction::Stop;
            }
            if row
                .primary_title
                .to_ascii_lowercase()
                .contains(&query_lower)
            {
                items.push(row.into_canonical(None));
            }
            RowAction::Continue
        })?;

        Ok(items)
    }

    fn get_by_id(&self, media_kind: MediaKind, source_id: &str) -> Result<Option<CanonicalMedia>> {
        let basics = self.download("/title.basics.tsv.gz")?;
        let mut result = None;

        for_each_basics_row(&basics, |row| {
            if row.tconst != source_id {
                return RowAction::Skip;
            }
            if row.kind != media_kind {
                return RowAction::Stop;
            }
            result = Some(row.into_canonical(None));
            RowAction::Stop
        })?;

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Dataset download helpers (private)
// ---------------------------------------------------------------------------

impl ImdbProvider {
    fn download(&self, path: &str) -> Result<Vec<u8>> {
        let resp = self.client.get(path).send()?.error_for_status()?;
        Ok(resp.bytes()?.to_vec())
    }
}

/// Action returned by the row visitor closure.
enum RowAction {
    Continue,
    Skip,
    Stop,
}

/// Parses the ratings TSV dump into a `tconst → rating` map.
fn parse_ratings(data: &[u8]) -> HashMap<String, f64> {
    let mut map = HashMap::new();
    let mut reader = std::io::BufReader::new(GzDecoder::new(data));
    let mut line = String::new();

    // Skip header.
    let _ = reader.read_line(&mut line);

    loop {
        line.clear();
        if reader.read_line(&mut line).is_err() || line.is_empty() {
            break;
        }
        let trimmed = line.trim_end_matches(['\n', '\r']);
        let mut parts = trimmed.splitn(3, '\t');
        if let (Some(tconst), Some(rating_str), _) = (parts.next(), parts.next(), parts.next())
            && let Ok(rating) = rating_str.parse::<f64>()
        {
            map.insert(tconst.to_string(), rating);
        }
    }

    map
}

/// A parsed row from `title.basics.tsv.gz`.
struct BasicsRow<'a> {
    tconst: &'a str,
    kind: MediaKind,
    primary_title: &'a str,
    original_title: Option<&'a str>,
    is_adult: bool,
    start_year: Option<i32>,
    end_year: Option<i32>,
    runtime_minutes: Option<i32>,
    genres: Vec<String>,
    title_type: &'a str,
}

impl<'a> BasicsRow<'a> {
    fn into_canonical(self, rating: Option<f64>) -> CanonicalMedia {
        let title_display = self
            .original_title
            .unwrap_or(self.primary_title)
            .to_string();

        let mut aliases = Vec::new();
        if self.original_title.map(|o| o != self.primary_title) == Some(true)
            && let Some(o) = self.original_title
        {
            aliases.push(o.to_string());
        }

        CanonicalMedia {
            media_kind: self.kind,
            title_display,
            title_romaji: None,
            title_english: self.original_title.map(str::to_string),
            title_native: None,
            synopsis: None,
            format: Some(self.title_type.to_string()),
            status: None,
            season: self.start_year.map(|y| y.to_string()),
            season_year: self.start_year,
            // IMDb doesn't expose episode count per-title in this dataset;
            // `runtime_minutes` is the closest proxy for movies.
            episodes: self.runtime_minutes,
            chapters: None,
            volumes: None,
            country_of_origin: None,
            cover_image: None,
            banner_image: None,
            provider_rating: rating.map(|r| (r / 10.0).clamp(0.0, 1.0)),
            nsfw: self.is_adult,
            aliases,
            genres: self.genres,
            tags: Vec::new(),
            external_ids: vec![ExternalId {
                source: SourceName::Imdb,
                source_id: self.tconst.to_string(),
                url: Some(format!("https://www.imdb.com/title/{}", self.tconst)),
            }],
            source_payloads: vec![SourcePayload {
                source: SourceName::Imdb,
                source_id: self.tconst.to_string(),
                url: Some(format!("https://www.imdb.com/title/{}", self.tconst)),
                remote_updated_at: None,
                raw_json: Some(serde_json::json!({
                    "tconst": self.tconst,
                    "titleType": self.title_type,
                    "primaryTitle": self.primary_title,
                    "isAdult": self.is_adult,
                    "startYear": self.start_year,
                    "endYear": self.end_year,
                    "runtimeMinutes": self.runtime_minutes,
                })),
            }],
            field_provenance: Vec::new(),
        }
    }
}

/// Parses a `titleType` column into a [`MediaKind`], returning `None` for
/// types that we don't index (short films, videos, etc.).
fn parse_title_type(title_type: &str) -> Option<MediaKind> {
    match title_type {
        "movie" | "tvMovie" | "video" => Some(MediaKind::Movie),
        "tvSeries" | "tvMiniSeries" | "tvSpecial" => Some(MediaKind::Show),
        _ => None,
    }
}

/// Returns `None` if `value` is `\N` or empty, otherwise `Some(String)`.
fn imdb_str(value: &str) -> Option<&str> {
    if value == "\\N" || value.is_empty() {
        None
    } else {
        Some(value)
    }
}

/// Returns `None` if `value` is `\N` or empty, otherwise parses as `i32`.
fn imdb_i32(value: &str) -> Option<i32> {
    imdb_str(value).and_then(|v| v.parse().ok())
}

/// Streams through the gzip-compressed basics TSV, calling `visitor` for each
/// usable row.  The visitor returns a `RowAction` to control iteration.
fn for_each_basics_row<F>(data: &[u8], mut visitor: F) -> Result<()>
where
    F: FnMut(BasicsRow<'_>) -> RowAction,
{
    let mut reader = std::io::BufReader::new(GzDecoder::new(data));
    let mut line = String::new();

    // Skip TSV header.
    reader
        .read_line(&mut line)
        .map_err(|e| Error::Validation(format!("failed to read IMDb dataset header: {e}")))?;

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }

        let trimmed = line.trim_end_matches(['\n', '\r']);
        let parts: Vec<&str> = trimmed.split('\t').collect();

        if parts.len() < 9 {
            continue;
        }

        let Some(kind) = parse_title_type(parts[1]) else {
            continue;
        };

        let primary_title = match imdb_str(parts[2]) {
            Some(t) => t,
            None => continue,
        };

        let genres: Vec<String> = imdb_str(parts[8])
            .map(|g| g.split(',').map(str::to_string).collect())
            .unwrap_or_default();

        let row = BasicsRow {
            tconst: parts[0],
            kind,
            title_type: parts[1],
            primary_title,
            original_title: imdb_str(parts[3]),
            is_adult: parts[4] == "1",
            start_year: imdb_i32(parts[5]),
            end_year: imdb_i32(parts[6]),
            runtime_minutes: imdb_i32(parts[7]),
            genres,
        };

        match visitor(row) {
            RowAction::Continue | RowAction::Skip => {}
            RowAction::Stop => break,
        }
    }

    Ok(())
}
