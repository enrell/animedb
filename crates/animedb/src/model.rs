use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::str::FromStr;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Anime,
    Manga,
    Show,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SourceName {
    AniList,
    MyAnimeList,
    Jikan,
    Kitsu,
    Tvmaze,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalId {
    pub source: SourceName,
    pub source_id: String,
    pub url: Option<String>,
}

impl ExternalId {
    pub fn is_strong_identity(&self) -> bool {
        matches!(self.source, SourceName::MyAnimeList | SourceName::AniList)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourcePayload {
    pub source: SourceName,
    pub source_id: String,
    pub url: Option<String>,
    pub remote_updated_at: Option<String>,
    pub raw_json: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonicalMedia {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldProvenance {
    pub field_name: String,
    pub source: SourceName,
    pub source_id: String,
    pub score: f64,
    pub reason: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchOptions {
    pub limit: usize,
    pub offset: usize,
    pub media_kind: Option<MediaKind>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub media_id: i64,
    pub media_kind: MediaKind,
    pub title_display: String,
    pub synopsis: Option<String>,
    pub score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    Full,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncCursor {
    pub page: usize,
}

impl Default for SyncCursor {
    fn default() -> Self {
        Self { page: 1 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncRequest {
    pub source: SourceName,
    pub media_kind: Option<MediaKind>,
    pub mode: SyncMode,
    pub page_size: usize,
    pub max_pages: Option<usize>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncOutcome {
    pub source: SourceName,
    pub media_kind: Option<MediaKind>,
    pub fetched_pages: usize,
    pub upserted_records: usize,
    pub last_cursor: Option<SyncCursor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncReport {
    pub outcomes: Vec<SyncOutcome>,
    pub total_upserted_records: usize,
}

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
