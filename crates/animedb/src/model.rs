//! Pure data types — always available, no SQLite dependency required.
//!
//! These types are the canonical vocabulary of the animedb domain layer and are
//! re-exported at the crate root. All fields are public for structural pattern-matching;
//! use the builder-style `with_*` methods on [`SearchOptions`] for construction.
//!
//! # Media identity
//!
//! A [`CanonicalMedia`] is identified by one or more [`ExternalId`] records (one per provider).
//! The first external ID in the list is used as the primary identity when resolving conflicts.
//! Strong identity providers (AniList, MyAnimeList) are preferred as primaries.
//!
//! # Provider priority
//!
//! During merge, fields are scored using per-provider weights:
//!
//! | Provider  | Weight | Notes |
//! |-----------|--------|-------|
//! | AniList   | 0.90   | Highest; GraphQL source, most complete |
//! | IMDb     | 0.85   | Movies & TV shows |
//! | TVmaze   | 0.82   | TV series |
//! | MyAnimeList | 0.80 | via Jikan REST API |
//! | Jikan    | 0.76   | Anime/Manga REST, rate-limited |
//! | Kitsu    | 0.78   | Anime/Manga REST |

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::str::FromStr;

use crate::error::{Error, Result};

/// Discriminates the four supported media kinds.
///
/// All provider data is mapped onto one of these four variants. The kind drives
/// which providers are queried (e.g. TVmaze only returns `Show`) and how search
/// results are filtered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    /// Japanese animation (TV series, movies, OVA).
    Anime,
    /// Japanese comics and printed manga.
    Manga,
    /// General TV series sourced from TVmaze or IMDb.
    Show,
    /// Feature films sourced from IMDb.
    Movie,
}

impl MediaKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Anime => "anime",
            Self::Manga => "manga",
            Self::Show => "show",
            Self::Movie => "movie",
        }
    }
}

impl fmt::Display for MediaKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for MediaKind {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "anime" | "ANIME" => Ok(Self::Anime),
            "manga" | "MANGA" => Ok(Self::Manga),
            "show" | "SHOW" => Ok(Self::Show),
            "movie" | "MOVIE" => Ok(Self::Movie),
            other => Err(Error::Validation(format!(
                "unsupported media kind: {other}"
            ))),
        }
    }
}

/// Names the remote metadata source that originally supplied a record.
///
/// Used as:
/// - the `source` field on every identifier, payload, and provenance record
/// - the key for selecting a provider in [`RemoteApi::with_provider`](crate::remote::RemoteApi::with_provider)
/// - the qualifier in kind-specific lookups (`media_external_id.source = ? AND media_kind = ?`)
///
/// # Provider capabilities
///
/// | Source  | Anime | Manga | Show | Movie | Episodes | Trending | Relations |
/// |---------|:-----:|:-----:|:----:|:-----:|:--------:|:--------:|:--------:|
/// | AniList |  Y   |   Y   |  N   |   N   |    N     |    Y     |    Y     |
/// | Jikan   |  Y   |   Y   |  N   |   N   |    Y     |    N     |    N     |
/// | Kitsu   |  Y   |   Y   |  N   |   N   |    Y     |    Y     |    N     |
/// | TVmaze  |  N   |   N   |  Y   |   N   |    Y     |    N     |    N     |
/// | IMDb   |  N   |   N   |  Y   |   Y   |  Bulk    |    N     |    N     |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SourceName {
    /// AniList.co — GraphQL API, anime and manga.
    AniList,
    /// MyAnimeList.net via the Jikan REST API wrapper.
    MyAnimeList,
    /// Jikan (MyAnimeList) REST API — anime and manga.
    Jikan,
    /// Kitsu.io REST API — anime and manga.
    Kitsu,
    /// TVmaze.com REST API — TV series only.
    Tvmaze,
    /// IMDb.com datasets — movies and TV shows.
    Imdb,
}

impl SourceName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AniList => "anilist",
            Self::MyAnimeList => "myanimelist",
            Self::Jikan => "jikan",
            Self::Kitsu => "kitsu",
            Self::Tvmaze => "tvmaze",
            Self::Imdb => "imdb",
        }
    }
}

impl fmt::Display for SourceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for SourceName {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "anilist" => Ok(Self::AniList),
            "myanimelist" => Ok(Self::MyAnimeList),
            "jikan" => Ok(Self::Jikan),
            "kitsu" => Ok(Self::Kitsu),
            "tvmaze" => Ok(Self::Tvmaze),
            "imdb" => Ok(Self::Imdb),
            other => Err(Error::Validation(format!("unsupported source: {other}"))),
        }
    }
}

/// A source-specific identifier for a media record.
///
/// Multiple [`ExternalId`] entries with different [`SourceName`] values may point to the same
/// logical media item; the merge layer resolves these into a single [`StoredMedia`] record.
/// The `url` field is optional and provided when the source exposes a browseable page.
///
/// [`is_strong_identity`](ExternalId::is_strong_identity) returns `true` for AniList and
/// MyAnimeList, which are used as primary identifiers during conflict resolution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalId {
    /// Which provider issued this identifier.
    pub source: SourceName,
    /// The provider's own string/integer ID for the record.
    pub source_id: String,
    /// Browseable URL for the record on the provider's site, if available.
    pub url: Option<String>,
}

impl ExternalId {
    pub fn is_strong_identity(&self) -> bool {
        matches!(self.source, SourceName::MyAnimeList | SourceName::AniList)
    }
}

/// Raw per-source payload as received from a provider.
///
/// Stored verbatim in SQLite for audit, re-analysis, and re-merge when provider
/// data formats are revised. The `raw_json` field holds the complete original
/// JSON response snippet; `payload_hash` enables change detection without
/// re-parsing large strings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourcePayload {
    /// Origin provider.
    pub source: SourceName,
    /// Identifier on the origin provider.
    pub source_id: String,
    /// Browseable URL on the origin provider, if available.
    pub url: Option<String>,
    /// Value of the `updated_at` or `aired` field on the provider side, if present.
    pub remote_updated_at: Option<String>,
    /// The complete original JSON response from the provider for this record.
    /// Use `serde_json::from_value` to parse into provider-specific structs.
    pub raw_json: Option<Value>,
}

/// Normalized media record as produced by a provider adapter and accepted by
/// [`AnimeDb`](crate::db::AnimeDb)::upsert_media.
///
/// This struct is the core domain type — it carries the canonical fields after
/// provider-specific normalization but before any local merge. It must contain
/// at least one [`ExternalId`] to be persisted.
///
/// # Field merge
///
/// When an existing record is found for the same external ID, the merge engine
/// scores incoming fields against stored provenance and keeps whichever has the
/// higher score. Provenance records are written to `field_provenance` so every
/// field value in the final [`StoredMedia`] is auditable back to a provider,
/// source ID, score, and reason string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonicalMedia {
    /// Discriminates anime / manga / show / movie.
    pub media_kind: MediaKind,
    /// Display title — always populated. Used as SQLite FTS index anchor.
    pub title_display: String,
    /// Romaji title (Latin-script Japanese title), if known.
    pub title_romaji: Option<String>,
    /// Official English-language title, if known.
    pub title_english: Option<String>,
    /// Native-script title (Japanese kanji/hiragana, Korean, etc.), if known.
    pub title_native: Option<String>,
    /// Synopsis/description text, if known.
    pub synopsis: Option<String>,
    /// Provider-specific format label (`"TV"`, `"MOVIE"`, `"OVA"`, `"MANGA"`, etc.).
    pub format: Option<String>,
    /// Airing/release status string, if known.
    pub status: Option<String>,
    /// Season token (`"spring"`, `"summer"`, etc.), if known.
    pub season: Option<String>,
    /// Primary release year, if known.
    pub season_year: Option<i32>,
    /// Total episode count per provider metadata (may differ from episode table).
    pub episodes: Option<i32>,
    /// Total chapter count for manga, if known.
    pub chapters: Option<i32>,
    /// Total volume count for manga, if known.
    pub volumes: Option<i32>,
    /// ISO 3166-1 alpha-2 country code of origin, if known.
    pub country_of_origin: Option<String>,
    /// URL to cover art image, if known.
    pub cover_image: Option<String>,
    /// URL to banner/header image, if known.
    pub banner_image: Option<String>,
    /// Provider-supplied score (0.0–1.0 normalized scale), if known.
    pub provider_rating: Option<f64>,
    /// Whether the provider marks this record as adult/mature content.
    pub nsfw: bool,
    /// Alternative titles, synonyms, and localized variants.
    pub aliases: Vec<String>,
    /// Genre strings as supplied by the provider.
    pub genres: Vec<String>,
    /// Free-form tag strings (demographics, themes, target audience).
    pub tags: Vec<String>,
    /// One entry per provider that has an ID for this media item.
    pub external_ids: Vec<ExternalId>,
    /// One entry per provider that has returned raw payload data for this media item.
    pub source_payloads: Vec<SourcePayload>,
    /// Merge provenance records — one per field that was resolved by the merge engine.
    /// Empty when the record is freshly parsed from a provider with no prior local state.
    pub field_provenance: Vec<FieldProvenance>,
}

impl CanonicalMedia {
    pub fn validate(&self) -> Result<()> {
        if self.title_display.trim().is_empty() {
            return Err(Error::Validation("title_display cannot be empty".into()));
        }

        if self.external_ids.is_empty() {
            return Err(Error::Validation(
                "at least one external id is required to persist media".into(),
            ));
        }

        Ok(())
    }

    pub fn name(&self) -> &str {
        &self.title_display
    }
}

/// A media record as persisted in the local SQLite database.
///
/// Returned by [`crate::db::AnimeDb::get_media`] and repository lookups. In addition to
/// the canonical fields it carries fully-resolved `external_ids`,
/// `source_payloads`, and `field_provenance` loaded from SQLite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredMedia {
    pub id: i64,
    pub media_kind: MediaKind,
    pub title_display: String,
    pub title_romaji: Option<String>,
    pub title_english: Option<String>,
    pub title_native: Option<String>,
    pub synopsis: Option<String>,
    pub format: Option<String>,
    pub status: Option<String>,
    pub season: Option<String>,
    pub season_year: Option<i32>,
    pub episodes: Option<i32>,
    pub chapters: Option<i32>,
    pub volumes: Option<i32>,
    pub country_of_origin: Option<String>,
    pub cover_image: Option<String>,
    pub banner_image: Option<String>,
    pub provider_rating: Option<f64>,
    pub nsfw: bool,
    pub aliases: Vec<String>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub external_ids: Vec<ExternalId>,
    pub source_payloads: Vec<SourcePayload>,
    pub field_provenance: Vec<FieldProvenance>,
}

impl StoredMedia {
    pub fn name(&self) -> &str {
        &self.title_display
    }
}

/// Records which provider field won during merge, with score and reasoning.
///
/// One [`FieldProvenance`] entry is written per canonical field whenever the merge
/// engine replaces a stored value with a new incoming value. The `score` is a
/// 0.0–1.0 float combining provider weight, field completeness, and text quality.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldProvenance {
    /// Name of the field this record covers (e.g. `"synopsis"`, `"cover_image"`).
    pub field_name: String,
    /// Provider that supplied the winning value.
    pub source: SourceName,
    /// Source-specific ID of the record that supplied the winning value.
    pub source_id: String,
    /// Weighted quality score (0.0–1.0) used in the merge decision.
    pub score: f64,
    /// Human-readable breakdown of how the score was computed.
    pub reason: String,
    /// Unix timestamp of when this record was written to the database.
    pub updated_at: String,
}

/// Options that govern local FTS search and remote provider queries.
///
/// Construct with [`Default::default`](SearchOptions::default) and chain builders:
///
/// ```ignore
/// let opts = SearchOptions::default()
///     .with_limit(10)
///     .with_offset(20)
///     .with_media_kind(MediaKind::Anime)
///     .with_format("TV");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchOptions {
    /// Maximum number of results to return. Default: 20.
    pub limit: usize,
    /// Number of results to skip for pagination. Default: 0.
    pub offset: usize,
    /// Restrict results to one media kind. Default: none (all kinds).
    pub media_kind: Option<MediaKind>,
    /// Restrict results to those with a matching `format` field value (e.g. `"MOVIE"`).
    pub format: Option<String>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limit: 20,
            offset: 0,
            media_kind: None,
            format: None,
        }
    }
}

impl SearchOptions {
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    pub fn with_media_kind(mut self, media_kind: MediaKind) -> Self {
        self.media_kind = Some(media_kind);
        self
    }

    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.format = Some(format.into());
        self
    }
}

/// A search result returned by local FTS5 or remote provider search.
///
/// The `score` field reflects BM25 ranking for local FTS queries; for remote
/// provider searches it reflects the provider's native relevance ranking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    /// Primary key of the matching media record in the local database.
    pub media_id: i64,
    /// Media kind of the matching record.
    pub media_kind: MediaKind,
    /// Display title of the matching record.
    pub title_display: String,
    /// Synopsis (may be `None` if truncated by FTS ranking).
    pub synopsis: Option<String>,
    /// BM25 score (lower = more relevant for FTS; higher = more relevant for remote).
    pub score: f64,
}

/// Controls whether a sync run overwrites all fields or only fills empty ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    /// Full overwrite — all canonical fields are re-scored and replaced.
    Full,
    /// Incremental — only empty fields on the stored record are filled.
    Incremental,
}

impl SyncMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Incremental => "incremental",
        }
    }
}

/// Cursor for paginating through a provider's result set.
///
/// The only currently used field is `page` (1-indexed per provider convention).
/// Providers may extend this struct with opaque tokens in future minor versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncCursor {
    /// Current page number (1-indexed, matching AniList/Jikan page numbering).
    pub page: usize,
}

impl Default for SyncCursor {
    fn default() -> Self {
        Self { page: 1 }
    }
}

/// Parameters that guide a single provider-to-catalog sync run.
///
/// Construct with [`SyncRequest::new`] and chain builders. The `source` field
/// is required; all other fields have sensible defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncRequest {
    /// Which provider to fetch from. Required.
    pub source: SourceName,
    /// Filter to one media kind (anime / manga / show / movie). Default: all kinds.
    pub media_kind: Option<MediaKind>,
    /// Full or incremental merge strategy. Default: [`SyncMode::Full`].
    pub mode: SyncMode,
    /// Number of records requested per page. Default: 50.
    pub page_size: usize,
    /// Hard cap on pages to fetch in this run. Default: unlimited.
    pub max_pages: Option<usize>,
    /// Resume from this cursor instead of the stored checkpoint. Default: none.
    pub start_cursor: Option<SyncCursor>,
}

impl SyncRequest {
    pub fn new(source: SourceName) -> Self {
        Self {
            source,
            media_kind: None,
            mode: SyncMode::Full,
            page_size: 50,
            max_pages: None,
            start_cursor: None,
        }
    }

    pub fn with_media_kind(mut self, media_kind: MediaKind) -> Self {
        self.media_kind = Some(media_kind);
        self
    }

    pub fn with_mode(mut self, mode: SyncMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_page_size(mut self, page_size: usize) -> Self {
        self.page_size = page_size;
        self
    }

    pub fn with_max_pages(mut self, max_pages: usize) -> Self {
        self.max_pages = Some(max_pages);
        self
    }

    pub fn with_start_cursor(mut self, cursor: SyncCursor) -> Self {
        self.start_cursor = Some(cursor);
        self
    }
}

/// Outcome of a single provider sync run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncOutcome {
    /// Provider that was synced.
    pub source: SourceName,
    /// Media kind that was synced (or `None` for multi-kind runs).
    pub media_kind: Option<MediaKind>,
    /// Number of HTTP request pages successfully fetched.
    pub fetched_pages: usize,
    /// Number of individual media records upserted into the local catalog.
    pub upserted_records: usize,
    /// Final cursor position reached. `None` if the provider exhausted its dataset.
    pub last_cursor: Option<SyncCursor>,
}

/// Aggregated result of a multi-source sync operation.
///
/// Returned by [`AnimeDb::sync_default_sources`](crate::db::AnimeDb::sync_default_sources)
/// and [`SyncService::sync_database`](crate::sync::SyncService::sync_database).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncReport {
    /// One outcome per provider × media-kind combination that was attempted.
    pub outcomes: Vec<SyncOutcome>,
    /// Sum of all `upserted_records` across every outcome.
    pub total_upserted_records: usize,
}

/// Persistent checkpoint for a provider sync, stored in SQLite `sync_state`.
///
/// Loaded on subsequent runs of the same provider+scope to resume from the
/// last successful cursor. Written after every completed page fetch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedSyncState {
    pub source: SourceName,
    pub scope: String,
    pub cursor: Option<SyncCursor>,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
    pub last_page: Option<i64>,
    pub mode: SyncMode,
}

// ---------------------------------------------------------------------------
// Episode types
// ---------------------------------------------------------------------------

/// Canonical episode data parsed from a provider adapter, used in upsert operations.
///
/// Populated by [`Provider::fetch_episodes`](crate::provider::Provider::fetch_episodes).
/// The `media_kind` field disambiguates anime vs. show when the same source ID
/// could refer to either a movie or a series episode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonicalEpisode {
    /// Provider that supplied this episode.
    pub source: SourceName,
    /// Provider-specific episode string ID.
    pub source_id: String,
    /// Media kind of the parent media item.
    pub media_kind: MediaKind,
    /// Season number within the parent series (1-indexed).
    pub season_number: Option<i32>,
    /// Episode number within the season (1-indexed).
    pub episode_number: Option<i32>,
    /// Absolute episode number across all seasons (for series with non-sequential numbering).
    pub absolute_number: Option<i32>,
    /// Display title for this episode.
    pub title_display: Option<String>,
    /// Original-language title.
    pub title_original: Option<String>,
    /// Synopsis/summary for this episode.
    pub synopsis: Option<String>,
    /// Original broadcast date string (provider-formatted, not normalized).
    pub air_date: Option<String>,
    /// Runtime in minutes, if known.
    pub runtime_minutes: Option<i32>,
    /// Thumbnail image URL, if available.
    pub thumbnail_url: Option<String>,
    /// Titles map (`{"en": "...", "ja_jp": "..."}`) in provider JSON form.
    pub raw_titles_json: Option<Value>,
    /// Complete original episode JSON from the provider.
    pub raw_json: Option<Value>,
}

/// Canonical episode data persisted in the local SQLite `episode` table.
///
/// This is the merged, provider-neutral form. When multiple providers supply
/// episode data for the same media item, the merge engine selects field values
/// from the highest-priority provider per field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredEpisode {
    /// Primary key in the local `episode` table.
    pub id: i64,
    /// Foreign key to the parent media record in `media.id`.
    pub media_id: i64,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub absolute_number: Option<i32>,
    pub title_display: Option<String>,
    pub title_original: Option<String>,
    pub titles_json: Option<Value>,
    pub synopsis: Option<String>,
    pub air_date: Option<String>,
    pub runtime_minutes: Option<i32>,
    pub thumbnail_url: Option<String>,
}

/// Episode record as received from a specific provider (raw/normalized).
///
/// Stored in `episode_source_record` to preserve the per-provider view for
/// audit and future re-merging. The `episode_id` field links to the canonical
/// `episode.id` after merge resolution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpisodeSourceRecord {
    pub id: i64,
    /// Canonical episode ID after merge resolution. `None` until merge runs.
    pub episode_id: Option<i64>,
    pub source: SourceName,
    pub source_id: String,
    pub media_id: i64,
    pub media_kind: MediaKind,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub absolute_number: Option<i32>,
    pub title_display: Option<String>,
    pub title_original: Option<String>,
    pub titles_json: Option<Value>,
    pub synopsis: Option<String>,
    pub air_date: Option<String>,
    pub runtime_minutes: Option<i32>,
    pub thumbnail_url: Option<String>,
    pub raw_json: Option<Value>,
    pub fetched_at: String,
}

/// A media record paired with its enriched episode list.
///
/// Returned by [`crate::repository::SearchRepository::media_document_by_id`] and
/// [`crate::repository::SearchRepository::media_document_by_external_id`].
/// The `episodes` vector contains canonical episode records for the media item
/// and is empty for media kinds that do not support episodes (e.g. movies).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaDocument {
    /// The canonical media record with all fields and identifiers.
    pub media: StoredMedia,
    /// Enriched episode records for this media item.
    pub episodes: Vec<StoredEpisode>,
}
