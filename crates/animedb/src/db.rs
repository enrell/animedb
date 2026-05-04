use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error as StdError;
use std::path::Path;
use std::thread;

use crate::error::{Error, Result};
use crate::merge::{score_boolean, score_cover_image, score_optional_i32, score_text_field};
use crate::model::{
    CanonicalEpisode, CanonicalMedia, ExternalId, FieldProvenance, MediaDocument, MediaKind,
    PersistedSyncState, SearchHit, SearchOptions, SourceName, SourcePayload, StoredEpisode,
    StoredMedia, SyncCursor, SyncMode, SyncOutcome, SyncReport, SyncRequest,
};
use crate::provider::{
    AniListProvider, ImdbProvider, JikanProvider, KitsuProvider, Provider, TvmazeProvider,
};
use crate::remote::{RemoteApi, RemoteSource};

/// Local-first entry point for the SQLite-backed catalog.
///
/// `AnimeDb` owns schema creation and migrations, provider sync, merge materialization,
/// local queries, and access to the underlying SQLite connection when lower-level control
/// is necessary.
pub struct AnimeDb {
    conn: Connection,
}

impl AnimeDb {
    /// Builds a remote-only facade for a selected provider.
    pub fn remote(source: RemoteSource) -> RemoteApi {
        RemoteApi::from(source)
    }

    /// Builds a remote-only AniList facade.
    pub fn remote_anilist() -> RemoteApi {
        RemoteApi::anilist()
    }

    /// Builds a remote-only Jikan facade.
    pub fn remote_jikan() -> RemoteApi {
        RemoteApi::jikan()
    }

    /// Builds a remote-only Kitsu facade.
    pub fn remote_kitsu() -> RemoteApi {
        RemoteApi::kitsu()
    }

    /// Creates or opens a database and performs the default bootstrap sync.
    pub fn generate_database(path: impl AsRef<Path>) -> Result<Self> {
        let (db, _) = Self::generate_database_with_report(path)?;
        Ok(db)
    }

    /// Creates or opens a database and performs the default bootstrap sync, returning the report.
    pub fn generate_database_with_report(path: impl AsRef<Path>) -> Result<(Self, SyncReport)> {
        let mut db = Self::open(path)?;
        let report = db.sync_default_sources()?;
        Ok((db, report))
    }

    /// Opens an existing database path and syncs the default providers into it.
    pub fn sync_database(path: impl AsRef<Path>) -> Result<SyncReport> {
        let mut db = Self::open(path)?;
        db.sync_default_sources()
    }

    /// Syncs AniList records for one media kind into the local database.
    pub fn sync_anilist(&mut self, media_kind: MediaKind) -> Result<SyncOutcome> {
        self.sync_from(
            &AniListProvider::default(),
            SyncRequest::new(SourceName::AniList).with_media_kind(media_kind),
        )
    }

    /// Syncs Jikan records for one media kind into the local database.
    pub fn sync_jikan(&mut self, media_kind: MediaKind) -> Result<SyncOutcome> {
        self.sync_from(
            &JikanProvider::default(),
            SyncRequest::new(SourceName::Jikan).with_media_kind(media_kind),
        )
    }

    /// Syncs Kitsu records for one media kind into the local database.
    pub fn sync_kitsu(&mut self, media_kind: MediaKind) -> Result<SyncOutcome> {
        self.sync_from(
            &KitsuProvider::default(),
            SyncRequest::new(SourceName::Kitsu).with_media_kind(media_kind),
        )
    }

    /// Syncs TVmaze show records into the local database.
    pub fn sync_tvmaze(&mut self) -> Result<SyncOutcome> {
        self.sync_from(
            &TvmazeProvider::default(),
            SyncRequest::new(SourceName::Tvmaze).with_media_kind(MediaKind::Show),
        )
    }

    /// Syncs IMDb records for one media kind into the local database.
    pub fn sync_imdb(&mut self, media_kind: MediaKind) -> Result<SyncOutcome> {
        self.sync_from(
            &ImdbProvider::default(),
            SyncRequest::new(SourceName::Imdb).with_media_kind(media_kind),
        )
    }

    /// Opens or creates a SQLite catalog, applies runtime pragmas, and runs migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.configure()?;
        db.migrate()?;
        Ok(db)
    }

    /// Opens an in-memory SQLite catalog with the same pragmas and migrations as a file-backed DB.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.configure()?;
        db.migrate()?;
        Ok(db)
    }

    /// Exposes the underlying SQLite connection for advanced integrations.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn upsert_media(&mut self, media: &CanonicalMedia) -> Result<i64> {
        media.validate()?;
        let tx = self.conn.transaction()?;
        let media_id = upsert_media_in_tx(&tx, media)?;
        tx.commit()?;
        Ok(media_id)
    }

    pub fn get_media(&self, media_id: i64) -> Result<StoredMedia> {
        let row = self
            .conn
            .query_row(
                r#"
                SELECT
                    id,
                    media_kind,
                    title_display,
                    title_romaji,
                    title_english,
                    title_native,
                    synopsis,
                    format,
                    status,
                    season,
                    season_year,
                    episodes,
                    chapters,
                    volumes,
                    country_of_origin,
                    cover_image,
                    banner_image,
                    provider_rating,
                    nsfw,
                    tags_json,
                    genres_json
                FROM media
                WHERE id = ?1
                "#,
                params![media_id],
                |row| {
                    let media_kind = parse_media_kind(row.get_ref(1)?.as_str()?)
                        .map_err(|err| rusqlite_decode_error(1, err))?;
                    let tags = serde_json::from_str(&row.get::<_, String>(19)?)
                        .map_err(|err| rusqlite_decode_error(19, err))?;
                    let genres = serde_json::from_str(&row.get::<_, String>(20)?)
                        .map_err(|err| rusqlite_decode_error(20, err))?;

                    Ok(StoredMedia {
                        id: row.get(0)?,
                        media_kind,
                        title_display: row.get(2)?,
                        title_romaji: row.get(3)?,
                        title_english: row.get(4)?,
                        title_native: row.get(5)?,
                        synopsis: row.get(6)?,
                        format: row.get(7)?,
                        status: row.get(8)?,
                        season: row.get(9)?,
                        season_year: row.get(10)?,
                        episodes: row.get(11)?,
                        chapters: row.get(12)?,
                        volumes: row.get(13)?,
                        country_of_origin: row.get(14)?,
                        cover_image: row.get(15)?,
                        banner_image: row.get(16)?,
                        provider_rating: row.get(17)?,
                        nsfw: row.get::<_, i64>(18)? != 0,
                        aliases: Vec::new(),
                        tags,
                        genres,
                        external_ids: Vec::new(),
                        source_payloads: Vec::new(),
                        field_provenance: Vec::new(),
                    })
                },
            )
            .optional()?
            .ok_or(Error::NotFound)?;

        let aliases = self.load_aliases(media_id)?;
        let external_ids = self.load_external_ids(media_id)?;
        let source_payloads = self.load_source_payloads(media_id)?;
        let field_provenance = self.load_field_provenance(media_id)?;

        Ok(StoredMedia {
            aliases,
            external_ids,
            source_payloads,
            field_provenance,
            ..row
        })
    }

    pub fn get_by_external_id(&self, source: SourceName, source_id: &str) -> Result<StoredMedia> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT media_id FROM media_external_id WHERE source = ?1 AND source_id = ?2 ORDER BY media_id",
        )?;
        let media_ids = stmt
            .query_map(params![source.as_str(), source_id], |row| {
                row.get::<_, i64>(0)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let media_id = match media_ids.as_slice() {
            [] => return Err(Error::NotFound),
            [media_id] => *media_id,
            _ => {
                return Err(Error::Validation(format!(
                    "external id {}:{} is ambiguous across media kinds; use a kind-aware query",
                    source, source_id
                )));
            }
        };

        self.get_media(media_id)
    }

    pub fn get_by_external_id_and_kind(
        &self,
        source: SourceName,
        media_kind: MediaKind,
        source_id: &str,
    ) -> Result<StoredMedia> {
        let media_id = self
            .conn
            .query_row(
                "SELECT media_id FROM media_external_id WHERE source = ?1 AND media_kind = ?2 AND source_id = ?3",
                params![source.as_str(), media_kind.as_str(), source_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .ok_or(Error::NotFound)?;

        self.get_media(media_id)
    }

    pub fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<SearchHit>> {
        let fts_query = build_fts_query(query)?;
        let limit = options.limit.max(1) as i64;
        let offset = options.offset as i64;
        let format = options
            .format
            .clone()
            .map(|value| value.to_ascii_uppercase());

        let mut statement =
            if let (Some(kind), Some(format)) = (options.media_kind, format.as_ref()) {
                self.conn
                    .prepare(
                        r#"
                SELECT
                    m.id,
                    m.media_kind,
                    m.title_display,
                    m.synopsis,
                    -bm25(media_fts) AS score
                FROM media_fts
                INNER JOIN media m ON m.id = media_fts.media_id
                WHERE media_fts MATCH ?1
                  AND m.media_kind = ?2
                  AND UPPER(COALESCE(m.format, '')) = ?3
                ORDER BY bm25(media_fts)
                LIMIT ?4 OFFSET ?5
                "#,
                    )?
                    .query_map(
                        params![fts_query, kind.as_str(), format, limit, offset],
                        |row| {
                            let media_kind = parse_media_kind(row.get_ref(1)?.as_str()?)
                                .map_err(|err| rusqlite_decode_error(1, err))?;
                            Ok(SearchHit {
                                media_id: row.get(0)?,
                                media_kind,
                                title_display: row.get(2)?,
                                synopsis: row.get(3)?,
                                score: row.get(4)?,
                            })
                        },
                    )?
                    .collect::<std::result::Result<Vec<_>, _>>()?
            } else if let Some(kind) = options.media_kind {
                self.conn
                    .prepare(
                        r#"
                SELECT
                    m.id,
                    m.media_kind,
                    m.title_display,
                    m.synopsis,
                    -bm25(media_fts) AS score
                FROM media_fts
                INNER JOIN media m ON m.id = media_fts.media_id
                WHERE media_fts MATCH ?1
                  AND m.media_kind = ?2
                ORDER BY bm25(media_fts)
                LIMIT ?3 OFFSET ?4
                "#,
                    )?
                    .query_map(params![fts_query, kind.as_str(), limit, offset], |row| {
                        let media_kind = parse_media_kind(row.get_ref(1)?.as_str()?)
                            .map_err(|err| rusqlite_decode_error(1, err))?;
                        Ok(SearchHit {
                            media_id: row.get(0)?,
                            media_kind,
                            title_display: row.get(2)?,
                            synopsis: row.get(3)?,
                            score: row.get(4)?,
                        })
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()?
            } else if let Some(format) = format.as_ref() {
                self.conn
                    .prepare(
                        r#"
                SELECT
                    m.id,
                    m.media_kind,
                    m.title_display,
                    m.synopsis,
                    -bm25(media_fts) AS score
                FROM media_fts
                INNER JOIN media m ON m.id = media_fts.media_id
                WHERE media_fts MATCH ?1
                  AND UPPER(COALESCE(m.format, '')) = ?2
                ORDER BY bm25(media_fts)
                LIMIT ?3 OFFSET ?4
                "#,
                    )?
                    .query_map(params![fts_query, format, limit, offset], |row| {
                        let media_kind = parse_media_kind(row.get_ref(1)?.as_str()?)
                            .map_err(|err| rusqlite_decode_error(1, err))?;
                        Ok(SearchHit {
                            media_id: row.get(0)?,
                            media_kind,
                            title_display: row.get(2)?,
                            synopsis: row.get(3)?,
                            score: row.get(4)?,
                        })
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()?
            } else {
                self.conn
                    .prepare(
                        r#"
                SELECT
                    m.id,
                    m.media_kind,
                    m.title_display,
                    m.synopsis,
                    -bm25(media_fts) AS score
                FROM media_fts
                INNER JOIN media m ON m.id = media_fts.media_id
                WHERE media_fts MATCH ?1
                ORDER BY bm25(media_fts)
                LIMIT ?2 OFFSET ?3
                "#,
                    )?
                    .query_map(params![fts_query, limit, offset], |row| {
                        let media_kind = parse_media_kind(row.get_ref(1)?.as_str()?)
                            .map_err(|err| rusqlite_decode_error(1, err))?;
                        Ok(SearchHit {
                            media_id: row.get(0)?,
                            media_kind,
                            title_display: row.get(2)?,
                            synopsis: row.get(3)?,
                            score: row.get(4)?,
                        })
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()?
            };

        statement.sort_by(|left, right| right.score.total_cmp(&left.score));
        Ok(statement)
    }

    pub fn anime_metadata(&self) -> MetadataCollection<'_> {
        MetadataCollection::new(
            self,
            SearchOptions::default().with_media_kind(MediaKind::Anime),
        )
    }

    pub fn manga_metadata(&self) -> MetadataCollection<'_> {
        MetadataCollection::new(
            self,
            SearchOptions::default().with_media_kind(MediaKind::Manga),
        )
    }

    pub fn movie_metadata(&self) -> MetadataCollection<'_> {
        MetadataCollection::new(
            self,
            SearchOptions::default()
                .with_media_kind(MediaKind::Anime)
                .with_format("MOVIE"),
        )
    }

    pub fn sync_from<P: Provider>(
        &mut self,
        provider: &P,
        request: SyncRequest,
    ) -> Result<SyncOutcome> {
        if provider.source() != request.source {
            return Err(Error::Validation(format!(
                "sync source mismatch: request={} provider={}",
                request.source,
                provider.source()
            )));
        }

        let scope = request
            .media_kind
            .map(|kind| kind.as_str().to_string())
            .unwrap_or_else(|| "all".to_string());

        let mut cursor = request
            .start_cursor
            .clone()
            .or_else(|| {
                self.load_sync_state(request.source, &scope)
                    .ok()
                    .and_then(|state| state.cursor)
            })
            .unwrap_or_default();

        let max_pages = request.max_pages.unwrap_or(usize::MAX);
        let mut fetched_pages = 0usize;
        let mut upserted_records = 0usize;
        let mut last_cursor = None;

        while fetched_pages < max_pages {
            let page = provider.fetch_page(&request, cursor.clone())?;
            if page.items.is_empty() {
                self.save_sync_state(PersistedSyncState {
                    source: request.source,
                    scope: scope.clone(),
                    cursor: last_cursor.clone(),
                    last_success_at: Some(now_string()),
                    last_error: None,
                    last_page: last_cursor.as_ref().map(|value| value.page as i64),
                    mode: request.mode,
                })?;
                break;
            }

            for item in &page.items {
                self.upsert_media(item)?;
                upserted_records += 1;
            }

            fetched_pages += 1;
            last_cursor = Some(cursor.clone());

            self.save_sync_state(PersistedSyncState {
                source: request.source,
                scope: scope.clone(),
                cursor: page.next_cursor.clone(),
                last_success_at: Some(now_string()),
                last_error: None,
                last_page: Some(cursor.page as i64),
                mode: request.mode,
            })?;

            let Some(next_cursor) = page.next_cursor else {
                break;
            };

            cursor = next_cursor;
            let sleep_for = provider.min_interval();
            if !sleep_for.is_zero() {
                thread::sleep(sleep_for);
            }
        }

        Ok(SyncOutcome {
            source: request.source,
            media_kind: request.media_kind,
            fetched_pages,
            upserted_records,
            last_cursor,
        })
    }

    pub fn sync_default_sources(&mut self) -> Result<SyncReport> {
        let anilist = AniListProvider::default();
        let jikan = JikanProvider::default();
        let kitsu = KitsuProvider::default();
        let tvmaze = TvmazeProvider::default();
        let imdb = ImdbProvider::default();
        let mut outcomes = Vec::new();

        for media_kind in [MediaKind::Anime, MediaKind::Manga] {
            outcomes.push(self.sync_from(
                &anilist,
                SyncRequest::new(SourceName::AniList).with_media_kind(media_kind),
            )?);
            outcomes.push(self.sync_from(
                &jikan,
                SyncRequest::new(SourceName::Jikan).with_media_kind(media_kind),
            )?);
            outcomes.push(self.sync_from(
                &kitsu,
                SyncRequest::new(SourceName::Kitsu).with_media_kind(media_kind),
            )?);
        }

        outcomes.push(self.sync_from(
            &tvmaze,
            SyncRequest::new(SourceName::Tvmaze).with_media_kind(MediaKind::Show),
        )?);

        for media_kind in [MediaKind::Show, MediaKind::Movie] {
            outcomes.push(self.sync_from(
                &imdb,
                SyncRequest::new(SourceName::Imdb).with_media_kind(media_kind),
            )?);
        }

        let total_upserted_records = outcomes.iter().map(|item| item.upserted_records).sum();

        Ok(SyncReport {
            outcomes,
            total_upserted_records,
        })
    }

    pub fn load_sync_state(&self, source: SourceName, scope: &str) -> Result<PersistedSyncState> {
        self.conn
            .query_row(
                r#"
                SELECT source, scope, cursor_json, last_success_at, last_error, last_page, mode
                FROM sync_state
                WHERE source = ?1 AND scope = ?2
                "#,
                params![source.as_str(), scope],
                |row| {
                    let source = parse_source(row.get_ref(0)?.as_str()?)
                        .map_err(|err| rusqlite_decode_error(0, err))?;
                    let scope = row.get::<_, String>(1)?;
                    let cursor = row
                        .get::<_, Option<String>>(2)?
                        .map(|value| serde_json::from_str::<SyncCursor>(&value))
                        .transpose()
                        .map_err(|err| rusqlite_decode_error(2, err))?;
                    let mode_str: String = row.get(6)?;
                    let mode = match mode_str.as_str() {
                        "full" => SyncMode::Full,
                        "incremental" => SyncMode::Incremental,
                        other => {
                            return Err(rusqlite_decode_error(
                                6,
                                Error::Validation(format!("unsupported sync mode: {other}")),
                            ));
                        }
                    };

                    Ok(PersistedSyncState {
                        source,
                        scope,
                        cursor,
                        last_success_at: row.get(3)?,
                        last_error: row.get(4)?,
                        last_page: row.get(5)?,
                        mode,
                    })
                },
            )
            .optional()?
            .ok_or(Error::NotFound)
    }

    pub fn save_sync_state(&self, state: PersistedSyncState) -> Result<()> {
        let cursor_json = state
            .cursor
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        self.conn.execute(
            r#"
            INSERT INTO sync_state (
                source,
                scope,
                cursor_json,
                last_success_at,
                last_error,
                last_page,
                mode
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(source, scope) DO UPDATE SET
                cursor_json = excluded.cursor_json,
                last_success_at = excluded.last_success_at,
                last_error = excluded.last_error,
                last_page = excluded.last_page,
                mode = excluded.mode
            "#,
            params![
                state.source.as_str(),
                state.scope,
                cursor_json,
                state.last_success_at,
                state.last_error,
                state.last_page,
                state.mode.as_str(),
            ],
        )?;

        Ok(())
    }

    fn configure(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA busy_timeout = 5000;
            PRAGMA temp_store = MEMORY;
            "#,
        )?;
        Ok(())
    }

    fn migrate(&self) -> Result<()> {
        let version: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))?;

        if version >= 5 {
            return Ok(());
        }

        if version == 0 {
            self.conn.execute_batch(
                r#"
                BEGIN;

                CREATE TABLE IF NOT EXISTS media (
                    id INTEGER PRIMARY KEY,
                    media_kind TEXT NOT NULL CHECK(media_kind IN ('anime', 'manga', 'show', 'movie')),
                    title_display TEXT NOT NULL,
                    title_romaji TEXT,
                    title_english TEXT,
                    title_native TEXT,
                    synopsis TEXT,
                    format TEXT,
                    status TEXT,
                    season TEXT,
                    season_year INTEGER,
                    episodes INTEGER,
                    chapters INTEGER,
                    volumes INTEGER,
                    country_of_origin TEXT,
                    cover_image TEXT,
                    banner_image TEXT,
                    provider_rating REAL,
                    nsfw INTEGER NOT NULL DEFAULT 0,
                    tags_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(tags_json)),
                    genres_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(genres_json)),
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );

                CREATE TABLE IF NOT EXISTS media_alias (
                    media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
                    alias TEXT NOT NULL,
                    normalized_alias TEXT NOT NULL,
                    UNIQUE(media_id, normalized_alias)
                );

                CREATE INDEX IF NOT EXISTS media_alias_normalized_idx
                    ON media_alias(normalized_alias);
                CREATE INDEX IF NOT EXISTS media_kind_idx
                    ON media(media_kind);
                CREATE INDEX IF NOT EXISTS media_season_year_idx
                    ON media(season_year);

                CREATE TABLE IF NOT EXISTS media_external_id (
                    media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
                    media_kind TEXT NOT NULL CHECK(media_kind IN ('anime', 'manga', 'show', 'movie')),
                    source TEXT NOT NULL,
                    source_id TEXT NOT NULL,
                    url TEXT,
                    UNIQUE(source, media_kind, source_id),
                    UNIQUE(media_id, source)
                );

                CREATE TABLE IF NOT EXISTS source_record (
                    media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
                    media_kind TEXT NOT NULL CHECK(media_kind IN ('anime', 'manga', 'show', 'movie')),
                    source TEXT NOT NULL,
                    source_id TEXT NOT NULL,
                    url TEXT,
                    remote_updated_at TEXT,
                    fetched_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    raw_json TEXT CHECK(raw_json IS NULL OR json_valid(raw_json)),
                    payload_hash TEXT,
                    UNIQUE(source, media_kind, source_id)
                );

                CREATE TABLE IF NOT EXISTS field_provenance (
                    media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
                    field_name TEXT NOT NULL,
                    source TEXT NOT NULL,
                    source_id TEXT NOT NULL,
                    score REAL NOT NULL,
                    reason TEXT NOT NULL,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    PRIMARY KEY(media_id, field_name)
                );

                CREATE TABLE IF NOT EXISTS sync_state (
                    source TEXT NOT NULL,
                    scope TEXT NOT NULL,
                    cursor_json TEXT,
                    last_success_at TEXT,
                    last_error TEXT,
                    last_page INTEGER,
                    mode TEXT NOT NULL DEFAULT 'full',
                    PRIMARY KEY(source, scope)
                );

                CREATE VIRTUAL TABLE IF NOT EXISTS media_fts USING fts5(
                    media_id UNINDEXED,
                    title_display,
                    aliases,
                    synopsis,
                    tokenize = 'unicode61 remove_diacritics 2'
                );

                PRAGMA user_version = 3;
                COMMIT;
                "#,
            )?;
        } else if version == 1 {
            self.conn.execute_batch(
                r#"
                BEGIN;
                ALTER TABLE media ADD COLUMN cover_image TEXT;
                ALTER TABLE media ADD COLUMN banner_image TEXT;
                ALTER TABLE media ADD COLUMN provider_rating REAL;

                CREATE TABLE IF NOT EXISTS media_external_id_v3 (
                    media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
                    media_kind TEXT NOT NULL CHECK(media_kind IN ('anime', 'manga')),
                    source TEXT NOT NULL,
                    source_id TEXT NOT NULL,
                    url TEXT,
                    UNIQUE(source, media_kind, source_id),
                    UNIQUE(media_id, source)
                );

                INSERT OR IGNORE INTO media_external_id_v3 (media_id, media_kind, source, source_id, url)
                SELECT media_external_id.media_id, media.media_kind, media_external_id.source, media_external_id.source_id, media_external_id.url
                FROM media_external_id
                INNER JOIN media ON media.id = media_external_id.media_id;

                DROP TABLE media_external_id;
                ALTER TABLE media_external_id_v3 RENAME TO media_external_id;

                CREATE TABLE IF NOT EXISTS source_record_v3 (
                    media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
                    media_kind TEXT NOT NULL CHECK(media_kind IN ('anime', 'manga')),
                    source TEXT NOT NULL,
                    source_id TEXT NOT NULL,
                    url TEXT,
                    remote_updated_at TEXT,
                    fetched_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    raw_json TEXT CHECK(raw_json IS NULL OR json_valid(raw_json)),
                    payload_hash TEXT,
                    UNIQUE(source, media_kind, source_id)
                );

                INSERT OR IGNORE INTO source_record_v3 (
                    media_id,
                    media_kind,
                    source,
                    source_id,
                    url,
                    remote_updated_at,
                    fetched_at,
                    raw_json,
                    payload_hash
                )
                SELECT source_record.media_id, media.media_kind, source_record.source, source_record.source_id, source_record.url, source_record.remote_updated_at, source_record.fetched_at, source_record.raw_json, source_record.payload_hash
                FROM source_record
                INNER JOIN media ON media.id = source_record.media_id;

                DROP TABLE source_record;
                ALTER TABLE source_record_v3 RENAME TO source_record;

                CREATE TABLE IF NOT EXISTS field_provenance (
                    media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
                    field_name TEXT NOT NULL,
                    source TEXT NOT NULL,
                    source_id TEXT NOT NULL,
                    score REAL NOT NULL,
                    reason TEXT NOT NULL,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    PRIMARY KEY(media_id, field_name)
                );

                PRAGMA user_version = 3;
                COMMIT;
                "#,
            )?;
        } else if version == 2 {
            self.conn.execute_batch(
                r#"
                BEGIN;

                CREATE TABLE IF NOT EXISTS media_external_id_v3 (
                    media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
                    media_kind TEXT NOT NULL CHECK(media_kind IN ('anime', 'manga')),
                    source TEXT NOT NULL,
                    source_id TEXT NOT NULL,
                    url TEXT,
                    UNIQUE(source, media_kind, source_id),
                    UNIQUE(media_id, source)
                );

                INSERT OR IGNORE INTO media_external_id_v3 (media_id, media_kind, source, source_id, url)
                SELECT media_external_id.media_id, media.media_kind, media_external_id.source, media_external_id.source_id, media_external_id.url
                FROM media_external_id
                INNER JOIN media ON media.id = media_external_id.media_id;

                DROP TABLE media_external_id;
                ALTER TABLE media_external_id_v3 RENAME TO media_external_id;

                CREATE TABLE IF NOT EXISTS source_record_v3 (
                    media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
                    media_kind TEXT NOT NULL CHECK(media_kind IN ('anime', 'manga')),
                    source TEXT NOT NULL,
                    source_id TEXT NOT NULL,
                    url TEXT,
                    remote_updated_at TEXT,
                    fetched_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    raw_json TEXT CHECK(raw_json IS NULL OR json_valid(raw_json)),
                    payload_hash TEXT,
                    UNIQUE(source, media_kind, source_id)
                );

                INSERT OR IGNORE INTO source_record_v3 (
                    media_id,
                    media_kind,
                    source,
                    source_id,
                    url,
                    remote_updated_at,
                    fetched_at,
                    raw_json,
                    payload_hash
                )
                SELECT source_record.media_id, media.media_kind, source_record.source, source_record.source_id, source_record.url, source_record.remote_updated_at, source_record.fetched_at, source_record.raw_json, source_record.payload_hash
                FROM source_record
                INNER JOIN media ON media.id = source_record.media_id;

                DROP TABLE source_record;
                ALTER TABLE source_record_v3 RENAME TO source_record;

                CREATE TABLE IF NOT EXISTS field_provenance (
                    media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
                    field_name TEXT NOT NULL,
                    source TEXT NOT NULL,
                    source_id TEXT NOT NULL,
                    score REAL NOT NULL,
                    reason TEXT NOT NULL,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    PRIMARY KEY(media_id, field_name)
                );

                PRAGMA user_version = 3;
                COMMIT;
                "#,
            )?;
        }

        if version < 4 {
            self.conn.execute_batch(
                r#"
                BEGIN;

                CREATE TABLE IF NOT EXISTS media_v4 (
                    id INTEGER PRIMARY KEY,
                    media_kind TEXT NOT NULL CHECK(media_kind IN ('anime', 'manga', 'show', 'movie')),
                    title_display TEXT NOT NULL,
                    title_romaji TEXT,
                    title_english TEXT,
                    title_native TEXT,
                    synopsis TEXT,
                    format TEXT,
                    status TEXT,
                    season TEXT,
                    season_year INTEGER,
                    episodes INTEGER,
                    chapters INTEGER,
                    volumes INTEGER,
                    country_of_origin TEXT,
                    cover_image TEXT,
                    banner_image TEXT,
                    provider_rating REAL,
                    nsfw INTEGER NOT NULL DEFAULT 0,
                    tags_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(tags_json)),
                    genres_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(genres_json)),
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );

                INSERT OR IGNORE INTO media_v4 (
                    id, media_kind, title_display, title_romaji, title_english, title_native,
                    synopsis, format, status, season, season_year, episodes, chapters, volumes,
                    country_of_origin, cover_image, banner_image, provider_rating, nsfw,
                    tags_json, genres_json, created_at, updated_at
                )
                SELECT
                    id, media_kind, title_display, title_romaji, title_english, title_native,
                    synopsis, format, status, season, season_year, episodes, chapters, volumes,
                    country_of_origin, cover_image, banner_image, provider_rating, nsfw,
                    tags_json, genres_json, created_at, updated_at
                FROM media;

                DROP TABLE media;
                ALTER TABLE media_v4 RENAME TO media;

                CREATE TABLE IF NOT EXISTS media_external_id_v4 (
                    media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
                    media_kind TEXT NOT NULL CHECK(media_kind IN ('anime', 'manga', 'show', 'movie')),
                    source TEXT NOT NULL,
                    source_id TEXT NOT NULL,
                    url TEXT,
                    UNIQUE(source, media_kind, source_id),
                    UNIQUE(media_id, source)
                );

                INSERT OR IGNORE INTO media_external_id_v4 (media_id, media_kind, source, source_id, url)
                SELECT media_id, media_kind, source, source_id, url FROM media_external_id;

                DROP TABLE media_external_id;
                ALTER TABLE media_external_id_v4 RENAME TO media_external_id;

                CREATE TABLE IF NOT EXISTS source_record_v4 (
                    media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
                    media_kind TEXT NOT NULL CHECK(media_kind IN ('anime', 'manga', 'show', 'movie')),
                    source TEXT NOT NULL,
                    source_id TEXT NOT NULL,
                    url TEXT,
                    remote_updated_at TEXT,
                    fetched_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    raw_json TEXT CHECK(raw_json IS NULL OR json_valid(raw_json)),
                    payload_hash TEXT,
                    UNIQUE(source, media_kind, source_id)
                );

                INSERT OR IGNORE INTO source_record_v4 (
                    media_id, media_kind, source, source_id, url, remote_updated_at, fetched_at, raw_json, payload_hash
                )
                SELECT media_id, media_kind, source, source_id, url, remote_updated_at, fetched_at, raw_json, payload_hash
                FROM source_record;

                DROP TABLE source_record;
                ALTER TABLE source_record_v4 RENAME TO source_record;

                CREATE INDEX IF NOT EXISTS media_alias_normalized_idx
                    ON media_alias(normalized_alias);
                CREATE INDEX IF NOT EXISTS media_kind_idx
                    ON media(media_kind);
                CREATE INDEX IF NOT EXISTS media_season_year_idx
                    ON media(season_year);

                PRAGMA user_version = 4;
                COMMIT;
                "#,
            )?;
        }

        if version < 5 {
            self.conn.execute_batch(
                r#"
                BEGIN;

                CREATE TABLE IF NOT EXISTS episode (
                    id INTEGER PRIMARY KEY,
                    source TEXT NOT NULL,
                    source_id TEXT NOT NULL,
                    media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
                    media_kind TEXT NOT NULL CHECK(media_kind IN ('anime', 'manga', 'show', 'movie')),
                    season_number INTEGER,
                    episode_number INTEGER,
                    absolute_number INTEGER,
                    title_display TEXT,
                    title_original TEXT,
                    synopsis TEXT,
                    air_date TEXT,
                    runtime_minutes INTEGER,
                    thumbnail_url TEXT,
                    raw_titles_json TEXT CHECK(raw_titles_json IS NULL OR json_valid(raw_titles_json)),
                    raw_json TEXT CHECK(raw_json IS NULL OR json_valid(raw_json)),
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    UNIQUE(source, media_kind, source_id, media_id)
                );

                CREATE INDEX IF NOT EXISTS episode_media_id_idx ON episode(media_id);
                CREATE INDEX IF NOT EXISTS episode_source_idx ON episode(source);
                CREATE INDEX IF NOT EXISTS episode_absolute_number_idx ON episode(media_id, absolute_number);
                CREATE INDEX IF NOT EXISTS episode_season_episode_idx ON episode(media_id, season_number, episode_number);

                PRAGMA user_version = 5;
                COMMIT;
                "#,
            )?;
        }

        Ok(())
    }

    fn load_aliases(&self, media_id: i64) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT alias FROM media_alias WHERE media_id = ?1 ORDER BY alias")?;
        let rows = stmt.query_map(params![media_id], |row| row.get::<_, String>(0))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn load_external_ids(&self, media_id: i64) -> Result<Vec<ExternalId>> {
        let mut stmt = self.conn.prepare(
            "SELECT source, source_id, url FROM media_external_id WHERE media_id = ?1 ORDER BY source",
        )?;
        let rows = stmt.query_map(params![media_id], |row| {
            let source = parse_source(row.get_ref(0)?.as_str()?)
                .map_err(|err| rusqlite_decode_error(0, err))?;
            Ok(ExternalId {
                source,
                source_id: row.get(1)?,
                url: row.get(2)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn load_source_payloads(&self, media_id: i64) -> Result<Vec<SourcePayload>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT source, source_id, url, remote_updated_at, raw_json
            FROM source_record
            WHERE media_id = ?1
            ORDER BY source
            "#,
        )?;
        let rows = stmt.query_map(params![media_id], |row| {
            let raw_json = row
                .get::<_, Option<String>>(4)?
                .map(|value| serde_json::from_str::<Value>(&value))
                .transpose()
                .map_err(|err| rusqlite_decode_error(4, err))?;
            let source = parse_source(row.get_ref(0)?.as_str()?)
                .map_err(|err| rusqlite_decode_error(0, err))?;
            Ok(SourcePayload {
                source,
                source_id: row.get(1)?,
                url: row.get(2)?,
                remote_updated_at: row.get(3)?,
                raw_json,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn load_field_provenance(&self, media_id: i64) -> Result<Vec<FieldProvenance>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT field_name, source, source_id, score, reason, updated_at
            FROM field_provenance
            WHERE media_id = ?1
            ORDER BY field_name
            "#,
        )?;
        let rows = stmt.query_map(params![media_id], |row| {
            let source = parse_source(row.get_ref(1)?.as_str()?)
                .map_err(|err| rusqlite_decode_error(1, err))?;
            Ok(FieldProvenance {
                field_name: row.get(0)?,
                source,
                source_id: row.get(2)?,
                score: row.get(3)?,
                reason: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    pub fn upsert_episode(&mut self, episode: &CanonicalEpisode, media_id: i64) -> Result<i64> {
        let raw_titles_json = episode
            .raw_titles_json
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let raw_json = episode
            .raw_json
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        self.conn.execute(
            r#"
            INSERT INTO episode (
                source, source_id, media_id, media_kind,
                season_number, episode_number, absolute_number,
                title_display, title_original, synopsis,
                air_date, runtime_minutes, thumbnail_url,
                raw_titles_json, raw_json,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, CURRENT_TIMESTAMP)
            ON CONFLICT(source, media_kind, source_id, media_id) DO UPDATE SET
                season_number = excluded.season_number,
                episode_number = excluded.episode_number,
                absolute_number = excluded.absolute_number,
                title_display = excluded.title_display,
                title_original = excluded.title_original,
                synopsis = excluded.synopsis,
                air_date = excluded.air_date,
                runtime_minutes = excluded.runtime_minutes,
                thumbnail_url = excluded.thumbnail_url,
                raw_titles_json = excluded.raw_titles_json,
                raw_json = excluded.raw_json,
                updated_at = CURRENT_TIMESTAMP
            "#,
            params![
                episode.source.as_str(),
                episode.source_id,
                media_id,
                episode.media_kind.as_str(),
                episode.season_number,
                episode.episode_number,
                episode.absolute_number,
                episode.title_display,
                episode.title_original,
                episode.synopsis,
                episode.air_date,
                episode.runtime_minutes,
                episode.thumbnail_url,
                raw_titles_json,
                raw_json,
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        Ok(id)
    }

    pub fn episodes_for_media(&self, media_id: i64) -> Result<Vec<StoredEpisode>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                id, source, source_id, media_id, media_kind,
                season_number, episode_number, absolute_number,
                title_display, title_original, synopsis,
                air_date, runtime_minutes, thumbnail_url,
                raw_titles_json, raw_json
            FROM episode
            WHERE media_id = ?1
            ORDER BY
                COALESCE(season_number, 0),
                COALESCE(episode_number, 0),
                COALESCE(absolute_number, 0)
            "#,
        )?;
        let rows = stmt.query_map(params![media_id], parse_stored_episode)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    pub fn episode_by_absolute_number(
        &self,
        media_id: i64,
        absolute_number: i32,
    ) -> Result<StoredEpisode> {
        self.conn
            .query_row(
                r#"
                SELECT
                    id, source, source_id, media_id, media_kind,
                    season_number, episode_number, absolute_number,
                    title_display, title_original, synopsis,
                    air_date, runtime_minutes, thumbnail_url,
                    raw_titles_json, raw_json
                FROM episode
                WHERE media_id = ?1 AND absolute_number = ?2
                "#,
                params![media_id, absolute_number],
                parse_stored_episode,
            )
            .optional()?
            .ok_or(Error::NotFound)
    }

    pub fn episode_by_season_episode(
        &self,
        media_id: i64,
        season_number: i32,
        episode_number: i32,
    ) -> Result<StoredEpisode> {
        self.conn
            .query_row(
                r#"
                SELECT
                    id, source, source_id, media_id, media_kind,
                    season_number, episode_number, absolute_number,
                    title_display, title_original, synopsis,
                    air_date, runtime_minutes, thumbnail_url,
                    raw_titles_json, raw_json
                FROM episode
                WHERE media_id = ?1 AND season_number = ?2 AND episode_number = ?3
                "#,
                params![media_id, season_number, episode_number],
                parse_stored_episode,
            )
            .optional()?
            .ok_or(Error::NotFound)
    }

    pub fn fetch_and_store_episodes(
        &mut self,
        provider: &dyn Provider,
        source: SourceName,
        source_id: &str,
    ) -> Result<Vec<StoredEpisode>> {
        let media = self
            .get_by_external_id_and_kind(source, MediaKind::Anime, source_id)
            .or_else(|_| self.get_by_external_id_and_kind(source, MediaKind::Show, source_id))?;

        let episodes = provider.fetch_episodes(media.media_kind, source_id)?;

        for episode in &episodes {
            self.upsert_episode(episode, media.id)?;
        }

        self.episodes_for_media(media.id)
    }

    pub fn media_document_by_id(&self, media_id: i64) -> Result<MediaDocument> {
        let media = self.get_media(media_id)?;
        let episodes = self.episodes_for_media(media_id)?;
        Ok(MediaDocument { media, episodes })
    }

    pub fn media_document_by_external_id(
        &self,
        source: SourceName,
        source_id: &str,
    ) -> Result<MediaDocument> {
        let media = self.get_by_external_id(source, source_id)?;
        let episodes = self.episodes_for_media(media.id)?;
        Ok(MediaDocument { media, episodes })
    }
}

/// Typed query facade over one local media slice.
///
/// Use this through [`AnimeDb::anime_metadata`], [`AnimeDb::manga_metadata`], or
/// [`AnimeDb::movie_metadata`] to avoid repeating search filters.
pub struct MetadataCollection<'a> {
    db: &'a AnimeDb,
    options: SearchOptions,
}

impl<'a> MetadataCollection<'a> {
    fn new(db: &'a AnimeDb, options: SearchOptions) -> Self {
        Self { db, options }
    }

    pub fn search(&self, query: &str) -> Result<Vec<SearchHit>> {
        self.db.search(query, self.options.clone())
    }

    pub fn get(&self, media_id: i64) -> Result<StoredMedia> {
        let media = self.db.get_media(media_id)?;
        if self.matches_media(&media) {
            Ok(media)
        } else {
            Err(Error::NotFound)
        }
    }

    pub fn by_external_id(&self, source: SourceName, source_id: &str) -> Result<StoredMedia> {
        let media = if let Some(kind) = self.options.media_kind {
            self.db
                .get_by_external_id_and_kind(source, kind, source_id)?
        } else {
            self.db.get_by_external_id(source, source_id)?
        };
        if self.matches_media(&media) {
            Ok(media)
        } else {
            Err(Error::NotFound)
        }
    }

    fn matches_media(&self, media: &StoredMedia) -> bool {
        if let Some(kind) = self.options.media_kind
            && media.media_kind != kind
        {
            return false;
        }

        if let Some(format) = &self.options.format
            && media
                .format
                .as_ref()
                .map(|value| value.eq_ignore_ascii_case(format))
                != Some(true)
        {
            return false;
        }

        true
    }
}

fn upsert_media_in_tx(tx: &Transaction<'_>, media: &CanonicalMedia) -> Result<i64> {
    let existing_media_id = resolve_media_id(tx, media.media_kind, &media.external_ids)?;
    ensure_no_conflicts(tx, media.media_kind, existing_media_id, &media.external_ids)?;
    let existing = existing_media_id
        .map(|media_id| load_stored_media_in_tx(tx, media_id))
        .transpose()?;
    let merged = merge_media(existing.as_ref(), media);
    let tags_json = serde_json::to_string(&merged.tags)?;
    let genres_json = serde_json::to_string(&merged.genres)?;

    let media_id = if let Some(media_id) = existing_media_id {
        tx.execute(
            r#"
            UPDATE media
            SET
                media_kind = ?2,
                title_display = ?3,
                title_romaji = ?4,
                title_english = ?5,
                title_native = ?6,
                synopsis = ?7,
                format = ?8,
                status = ?9,
                season = ?10,
                season_year = ?11,
                episodes = ?12,
                chapters = ?13,
                volumes = ?14,
                country_of_origin = ?15,
                cover_image = ?16,
                banner_image = ?17,
                provider_rating = ?18,
                nsfw = ?19,
                tags_json = ?20,
                genres_json = ?21,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            "#,
            params![
                media_id,
                merged.media_kind.as_str(),
                merged.title_display,
                merged.title_romaji,
                merged.title_english,
                merged.title_native,
                merged.synopsis,
                merged.format,
                merged.status,
                merged.season,
                merged.season_year,
                merged.episodes,
                merged.chapters,
                merged.volumes,
                merged.country_of_origin,
                merged.cover_image,
                merged.banner_image,
                merged.provider_rating,
                i64::from(merged.nsfw as i32),
                tags_json,
                genres_json,
            ],
        )?;
        media_id
    } else {
        tx.execute(
            r#"
            INSERT INTO media (
                media_kind,
                title_display,
                title_romaji,
                title_english,
                title_native,
                synopsis,
                format,
                status,
                season,
                season_year,
                episodes,
                chapters,
                volumes,
                country_of_origin,
                cover_image,
                banner_image,
                provider_rating,
                nsfw,
                tags_json,
                genres_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
            "#,
            params![
                merged.media_kind.as_str(),
                merged.title_display,
                merged.title_romaji,
                merged.title_english,
                merged.title_native,
                merged.synopsis,
                merged.format,
                merged.status,
                merged.season,
                merged.season_year,
                merged.episodes,
                merged.chapters,
                merged.volumes,
                merged.country_of_origin,
                merged.cover_image,
                merged.banner_image,
                merged.provider_rating,
                i64::from(merged.nsfw as i32),
                tags_json,
                genres_json,
            ],
        )?;
        tx.last_insert_rowid()
    };

    tx.execute(
        "DELETE FROM media_alias WHERE media_id = ?1",
        params![media_id],
    )?;
    for alias in normalize_aliases(&merged.aliases) {
        tx.execute(
            r#"
            INSERT INTO media_alias (media_id, alias, normalized_alias)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(media_id, normalized_alias) DO NOTHING
            "#,
            params![media_id, alias, normalize_for_lookup(&alias)],
        )?;
    }

    for external_id in &merged.external_ids {
        tx.execute(
            r#"
            INSERT INTO media_external_id (media_id, media_kind, source, source_id, url)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(source, media_kind, source_id) DO UPDATE SET
                media_id = excluded.media_id,
                url = excluded.url
            "#,
            params![
                media_id,
                merged.media_kind.as_str(),
                external_id.source.as_str(),
                external_id.source_id,
                external_id.url,
            ],
        )?;
    }

    for payload in &merged.source_payloads {
        let raw_json = payload
            .raw_json
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let payload_hash = raw_json
            .as_ref()
            .map(|value| stable_payload_hash(value))
            .transpose()?;

        tx.execute(
            r#"
            INSERT INTO source_record (
                media_id,
                media_kind,
                source,
                source_id,
                url,
                remote_updated_at,
                fetched_at,
                raw_json,
                payload_hash
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, CURRENT_TIMESTAMP, ?7, ?8)
            ON CONFLICT(source, media_kind, source_id) DO UPDATE SET
                media_id = excluded.media_id,
                url = excluded.url,
                remote_updated_at = excluded.remote_updated_at,
                fetched_at = CURRENT_TIMESTAMP,
                raw_json = excluded.raw_json,
                payload_hash = excluded.payload_hash
            "#,
            params![
                media_id,
                merged.media_kind.as_str(),
                payload.source.as_str(),
                payload.source_id,
                payload.url,
                payload.remote_updated_at,
                raw_json,
                payload_hash,
            ],
        )?;
    }

    tx.execute(
        "DELETE FROM field_provenance WHERE media_id = ?1",
        params![media_id],
    )?;
    for provenance in &merged.field_provenance {
        tx.execute(
            r#"
            INSERT INTO field_provenance (
                media_id,
                field_name,
                source,
                source_id,
                score,
                reason,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                media_id,
                provenance.field_name,
                provenance.source.as_str(),
                provenance.source_id,
                provenance.score,
                provenance.reason,
                provenance.updated_at,
            ],
        )?;
    }

    tx.execute(
        "DELETE FROM media_fts WHERE media_id = ?1",
        params![media_id],
    )?;
    tx.execute(
        r#"
        INSERT INTO media_fts (media_id, title_display, aliases, synopsis)
        VALUES (?1, ?2, ?3, ?4)
        "#,
        params![
            media_id,
            merged.title_display,
            normalize_aliases(&merged.aliases).join(" "),
            merged.synopsis,
        ],
    )?;

    Ok(media_id)
}

fn load_stored_media_in_tx(tx: &Transaction<'_>, media_id: i64) -> Result<StoredMedia> {
    let row = tx
        .query_row(
            r#"
            SELECT
                id,
                media_kind,
                title_display,
                title_romaji,
                title_english,
                title_native,
                synopsis,
                format,
                status,
                season,
                season_year,
                episodes,
                chapters,
                volumes,
                country_of_origin,
                cover_image,
                banner_image,
                provider_rating,
                nsfw,
                tags_json,
                genres_json
            FROM media
            WHERE id = ?1
            "#,
            params![media_id],
            |row| {
                let media_kind = parse_media_kind(row.get_ref(1)?.as_str()?)
                    .map_err(|err| rusqlite_decode_error(1, err))?;
                let tags = serde_json::from_str(&row.get::<_, String>(19)?)
                    .map_err(|err| rusqlite_decode_error(19, err))?;
                let genres = serde_json::from_str(&row.get::<_, String>(20)?)
                    .map_err(|err| rusqlite_decode_error(20, err))?;

                Ok(StoredMedia {
                    id: row.get(0)?,
                    media_kind,
                    title_display: row.get(2)?,
                    title_romaji: row.get(3)?,
                    title_english: row.get(4)?,
                    title_native: row.get(5)?,
                    synopsis: row.get(6)?,
                    format: row.get(7)?,
                    status: row.get(8)?,
                    season: row.get(9)?,
                    season_year: row.get(10)?,
                    episodes: row.get(11)?,
                    chapters: row.get(12)?,
                    volumes: row.get(13)?,
                    country_of_origin: row.get(14)?,
                    cover_image: row.get(15)?,
                    banner_image: row.get(16)?,
                    provider_rating: row.get(17)?,
                    nsfw: row.get::<_, i64>(18)? != 0,
                    aliases: Vec::new(),
                    genres,
                    tags,
                    external_ids: Vec::new(),
                    source_payloads: Vec::new(),
                    field_provenance: Vec::new(),
                })
            },
        )
        .optional()?
        .ok_or(Error::NotFound)?;

    let aliases = {
        let mut stmt =
            tx.prepare("SELECT alias FROM media_alias WHERE media_id = ?1 ORDER BY alias")?;
        let rows = stmt.query_map(params![media_id], |row| row.get::<_, String>(0))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };

    let external_ids = {
        let mut stmt = tx.prepare(
            "SELECT source, source_id, url FROM media_external_id WHERE media_id = ?1 ORDER BY source",
        )?;
        let rows = stmt.query_map(params![media_id], |row| {
            let source = parse_source(row.get_ref(0)?.as_str()?)
                .map_err(|err| rusqlite_decode_error(0, err))?;
            Ok(ExternalId {
                source,
                source_id: row.get(1)?,
                url: row.get(2)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };

    let source_payloads = {
        let mut stmt = tx.prepare(
            r#"
            SELECT source, source_id, url, remote_updated_at, raw_json
            FROM source_record
            WHERE media_id = ?1
            ORDER BY source
            "#,
        )?;
        let rows = stmt.query_map(params![media_id], |row| {
            let raw_json = row
                .get::<_, Option<String>>(4)?
                .map(|value| serde_json::from_str::<Value>(&value))
                .transpose()
                .map_err(|err| rusqlite_decode_error(4, err))?;
            let source = parse_source(row.get_ref(0)?.as_str()?)
                .map_err(|err| rusqlite_decode_error(0, err))?;
            Ok(SourcePayload {
                source,
                source_id: row.get(1)?,
                url: row.get(2)?,
                remote_updated_at: row.get(3)?,
                raw_json,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };

    let field_provenance = {
        let mut stmt = tx.prepare(
            r#"
            SELECT field_name, source, source_id, score, reason, updated_at
            FROM field_provenance
            WHERE media_id = ?1
            ORDER BY field_name
            "#,
        )?;
        let rows = stmt.query_map(params![media_id], |row| {
            let source = parse_source(row.get_ref(1)?.as_str()?)
                .map_err(|err| rusqlite_decode_error(1, err))?;
            Ok(FieldProvenance {
                field_name: row.get(0)?,
                source,
                source_id: row.get(2)?,
                score: row.get(3)?,
                reason: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };

    Ok(StoredMedia {
        aliases,
        external_ids,
        source_payloads,
        field_provenance,
        ..row
    })
}

fn merge_media(existing: Option<&StoredMedia>, incoming: &CanonicalMedia) -> CanonicalMedia {
    let origin = incoming_origin(incoming);
    let existing_scores = existing_score_map(existing);
    let mut provenance = Vec::new();

    let title_display = choose_text(
        "title_display",
        existing.map(|item| item.title_display.as_str()),
        existing_scores.get("title_display"),
        Some(incoming.title_display.as_str()),
        incoming,
        &origin,
        &mut provenance,
    )
    .unwrap_or_else(|| incoming.title_display.clone());

    let title_romaji = choose_text(
        "title_romaji",
        existing.and_then(|item| item.title_romaji.as_deref()),
        existing_scores.get("title_romaji"),
        incoming.title_romaji.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let title_english = choose_text(
        "title_english",
        existing.and_then(|item| item.title_english.as_deref()),
        existing_scores.get("title_english"),
        incoming.title_english.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let title_native = choose_text(
        "title_native",
        existing.and_then(|item| item.title_native.as_deref()),
        existing_scores.get("title_native"),
        incoming.title_native.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let synopsis = choose_text(
        "synopsis",
        existing.and_then(|item| item.synopsis.as_deref()),
        existing_scores.get("synopsis"),
        incoming.synopsis.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let format = choose_text(
        "format",
        existing.and_then(|item| item.format.as_deref()),
        existing_scores.get("format"),
        incoming.format.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let status = choose_text(
        "status",
        existing.and_then(|item| item.status.as_deref()),
        existing_scores.get("status"),
        incoming.status.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let season = choose_text(
        "season",
        existing.and_then(|item| item.season.as_deref()),
        existing_scores.get("season"),
        incoming.season.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let country_of_origin = choose_text(
        "country_of_origin",
        existing.and_then(|item| item.country_of_origin.as_deref()),
        existing_scores.get("country_of_origin"),
        incoming.country_of_origin.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );

    let season_year = choose_i32(
        "season_year",
        existing.and_then(|item| item.season_year),
        existing_scores.get("season_year"),
        incoming.season_year,
        incoming,
        &origin,
        &mut provenance,
    );
    let episodes = choose_i32(
        "episodes",
        existing.and_then(|item| item.episodes),
        existing_scores.get("episodes"),
        incoming.episodes,
        incoming,
        &origin,
        &mut provenance,
    );
    let chapters = choose_i32(
        "chapters",
        existing.and_then(|item| item.chapters),
        existing_scores.get("chapters"),
        incoming.chapters,
        incoming,
        &origin,
        &mut provenance,
    );
    let volumes = choose_i32(
        "volumes",
        existing.and_then(|item| item.volumes),
        existing_scores.get("volumes"),
        incoming.volumes,
        incoming,
        &origin,
        &mut provenance,
    );

    let cover_image = choose_cover(
        "cover_image",
        existing.and_then(|item| item.cover_image.as_deref()),
        existing_scores.get("cover_image"),
        incoming.cover_image.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let banner_image = choose_cover(
        "banner_image",
        existing.and_then(|item| item.banner_image.as_deref()),
        existing_scores.get("banner_image"),
        incoming.banner_image.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let nsfw = choose_bool(
        "nsfw",
        existing.map(|item| item.nsfw),
        existing_scores.get("nsfw"),
        incoming.nsfw,
        incoming,
        &origin,
        &mut provenance,
    );
    let provider_rating = choose_rating(
        existing.and_then(|item| item.provider_rating),
        incoming.provider_rating,
    );

    CanonicalMedia {
        media_kind: existing
            .map(|item| item.media_kind)
            .unwrap_or(incoming.media_kind),
        title_display,
        title_romaji,
        title_english,
        title_native,
        synopsis,
        format,
        status,
        season,
        season_year,
        episodes,
        chapters,
        volumes,
        country_of_origin,
        cover_image,
        banner_image,
        provider_rating,
        nsfw,
        aliases: merge_string_lists(
            existing.map(|item| item.aliases.as_slice()),
            &incoming.aliases,
        ),
        genres: merge_string_lists(
            existing.map(|item| item.genres.as_slice()),
            &incoming.genres,
        ),
        tags: merge_string_lists(existing.map(|item| item.tags.as_slice()), &incoming.tags),
        external_ids: merge_external_ids(
            existing.map(|item| item.external_ids.as_slice()),
            &incoming.external_ids,
        ),
        source_payloads: merge_source_payloads(
            existing.map(|item| item.source_payloads.as_slice()),
            &incoming.source_payloads,
        ),
        field_provenance: provenance,
    }
}

fn resolve_media_id(
    tx: &Transaction<'_>,
    media_kind: MediaKind,
    external_ids: &[ExternalId],
) -> Result<Option<i64>> {
    for external_id in external_ids {
        let media_id = tx
            .query_row(
                "SELECT media_id FROM media_external_id WHERE source = ?1 AND media_kind = ?2 AND source_id = ?3",
                params![
                    external_id.source.as_str(),
                    media_kind.as_str(),
                    external_id.source_id
                ],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        if media_id.is_some() {
            return Ok(media_id);
        }
    }
    Ok(None)
}

fn ensure_no_conflicts(
    tx: &Transaction<'_>,
    media_kind: MediaKind,
    expected_media_id: Option<i64>,
    external_ids: &[ExternalId],
) -> Result<()> {
    for external_id in external_ids {
        let found_media_id = tx
            .query_row(
                "SELECT media_id FROM media_external_id WHERE source = ?1 AND media_kind = ?2 AND source_id = ?3",
                params![
                    external_id.source.as_str(),
                    media_kind.as_str(),
                    external_id.source_id
                ],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;

        if let (Some(expected), Some(found)) = (expected_media_id, found_media_id)
            && expected != found
        {
            return Err(Error::ConflictingExternalId {
                provider: external_id.source.to_string(),
                source_id: external_id.source_id.clone(),
            });
        }
    }

    Ok(())
}

fn incoming_origin(media: &CanonicalMedia) -> (SourceName, String) {
    if let Some(payload) = media.source_payloads.first() {
        return (payload.source, payload.source_id.clone());
    }
    if let Some(external_id) = media.external_ids.first() {
        return (external_id.source, external_id.source_id.clone());
    }
    (SourceName::AniList, "unknown".to_string())
}

fn existing_score_map(existing: Option<&StoredMedia>) -> HashMap<String, FieldProvenance> {
    existing
        .map(|item| {
            item.field_provenance
                .iter()
                .cloned()
                .map(|entry| (entry.field_name.clone(), entry))
                .collect()
        })
        .unwrap_or_default()
}

fn choose_text(
    field_name: &str,
    existing_value: Option<&str>,
    existing_provenance: Option<&FieldProvenance>,
    incoming_value: Option<&str>,
    incoming: &CanonicalMedia,
    origin: &(SourceName, String),
    provenance: &mut Vec<FieldProvenance>,
) -> Option<String> {
    match (existing_value, incoming_value) {
        (Some(existing), Some(candidate)) => {
            let existing_score = existing_provenance.map(|item| item.score).unwrap_or(0.60);
            let incoming_decision = score_text_field(origin.0, field_name, candidate, incoming);
            if incoming_decision.score >= existing_score {
                provenance.push(make_provenance(
                    field_name,
                    origin.0,
                    origin.1.as_str(),
                    incoming_decision.score,
                    incoming_decision.reason,
                ));
                Some(incoming_decision.value)
            } else {
                if let Some(entry) = existing_provenance.cloned() {
                    provenance.push(entry);
                }
                Some(existing.to_string())
            }
        }
        (None, Some(candidate)) => {
            let incoming_decision = score_text_field(origin.0, field_name, candidate, incoming);
            provenance.push(make_provenance(
                field_name,
                origin.0,
                origin.1.as_str(),
                incoming_decision.score,
                incoming_decision.reason,
            ));
            Some(incoming_decision.value)
        }
        (Some(existing), None) => {
            if let Some(entry) = existing_provenance.cloned() {
                provenance.push(entry);
            }
            Some(existing.to_string())
        }
        (None, None) => None,
    }
}

fn choose_i32(
    field_name: &str,
    existing_value: Option<i32>,
    existing_provenance: Option<&FieldProvenance>,
    incoming_value: Option<i32>,
    incoming: &CanonicalMedia,
    origin: &(SourceName, String),
    provenance: &mut Vec<FieldProvenance>,
) -> Option<i32> {
    match (existing_value, incoming_value) {
        (Some(existing), Some(candidate)) => {
            let existing_score = existing_provenance.map(|item| item.score).unwrap_or(0.60);
            let incoming_decision = score_optional_i32(origin.0, candidate, incoming);
            if incoming_decision.score >= existing_score {
                provenance.push(make_provenance(
                    field_name,
                    origin.0,
                    origin.1.as_str(),
                    incoming_decision.score,
                    incoming_decision.reason,
                ));
                Some(incoming_decision.value)
            } else {
                if let Some(entry) = existing_provenance.cloned() {
                    provenance.push(entry);
                }
                Some(existing)
            }
        }
        (None, Some(candidate)) => {
            let incoming_decision = score_optional_i32(origin.0, candidate, incoming);
            provenance.push(make_provenance(
                field_name,
                origin.0,
                origin.1.as_str(),
                incoming_decision.score,
                incoming_decision.reason,
            ));
            Some(incoming_decision.value)
        }
        (Some(existing), None) => {
            if let Some(entry) = existing_provenance.cloned() {
                provenance.push(entry);
            }
            Some(existing)
        }
        (None, None) => None,
    }
}

fn choose_cover(
    field_name: &str,
    existing_value: Option<&str>,
    existing_provenance: Option<&FieldProvenance>,
    incoming_value: Option<&str>,
    incoming: &CanonicalMedia,
    origin: &(SourceName, String),
    provenance: &mut Vec<FieldProvenance>,
) -> Option<String> {
    match (existing_value, incoming_value) {
        (Some(existing), Some(candidate)) => {
            let existing_score = existing_provenance.map(|item| item.score).unwrap_or(0.60);
            let incoming_decision = score_cover_image(origin.0, candidate, incoming);
            if incoming_decision.score >= existing_score {
                provenance.push(make_provenance(
                    field_name,
                    origin.0,
                    origin.1.as_str(),
                    incoming_decision.score,
                    incoming_decision.reason,
                ));
                Some(incoming_decision.value)
            } else {
                if let Some(entry) = existing_provenance.cloned() {
                    provenance.push(entry);
                }
                Some(existing.to_string())
            }
        }
        (None, Some(candidate)) => {
            let incoming_decision = score_cover_image(origin.0, candidate, incoming);
            provenance.push(make_provenance(
                field_name,
                origin.0,
                origin.1.as_str(),
                incoming_decision.score,
                incoming_decision.reason,
            ));
            Some(incoming_decision.value)
        }
        (Some(existing), None) => {
            if let Some(entry) = existing_provenance.cloned() {
                provenance.push(entry);
            }
            Some(existing.to_string())
        }
        (None, None) => None,
    }
}

fn choose_bool(
    field_name: &str,
    existing_value: Option<bool>,
    existing_provenance: Option<&FieldProvenance>,
    incoming_value: bool,
    incoming: &CanonicalMedia,
    origin: &(SourceName, String),
    provenance: &mut Vec<FieldProvenance>,
) -> bool {
    match existing_value {
        Some(existing) => {
            let existing_score = existing_provenance.map(|item| item.score).unwrap_or(0.60);
            let incoming_decision = score_boolean(origin.0, incoming_value, incoming);
            if incoming_decision.score >= existing_score {
                provenance.push(make_provenance(
                    field_name,
                    origin.0,
                    origin.1.as_str(),
                    incoming_decision.score,
                    incoming_decision.reason,
                ));
                incoming_decision.value
            } else {
                if let Some(entry) = existing_provenance.cloned() {
                    provenance.push(entry);
                }
                existing
            }
        }
        None => {
            let incoming_decision = score_boolean(origin.0, incoming_value, incoming);
            provenance.push(make_provenance(
                field_name,
                origin.0,
                origin.1.as_str(),
                incoming_decision.score,
                incoming_decision.reason,
            ));
            incoming_decision.value
        }
    }
}

fn choose_rating(existing_value: Option<f64>, incoming_value: Option<f64>) -> Option<f64> {
    match (existing_value, incoming_value) {
        (Some(existing), Some(candidate)) => Some(existing.max(candidate)),
        (None, Some(candidate)) => Some(candidate),
        (Some(existing), None) => Some(existing),
        (None, None) => None,
    }
}

fn make_provenance(
    field_name: &str,
    source: SourceName,
    source_id: &str,
    score: f64,
    reason: String,
) -> FieldProvenance {
    FieldProvenance {
        field_name: field_name.to_string(),
        source,
        source_id: source_id.to_string(),
        score,
        reason,
        updated_at: now_string(),
    }
}

fn merge_string_lists(existing: Option<&[String]>, incoming: &[String]) -> Vec<String> {
    let mut values = Vec::new();
    for value in existing.into_iter().flatten() {
        if !values
            .iter()
            .any(|item: &String| item.eq_ignore_ascii_case(value))
        {
            values.push(value.clone());
        }
    }
    for value in incoming {
        if !values
            .iter()
            .any(|item: &String| item.eq_ignore_ascii_case(value))
        {
            values.push(value.clone());
        }
    }
    values
}

fn merge_external_ids(existing: Option<&[ExternalId]>, incoming: &[ExternalId]) -> Vec<ExternalId> {
    let mut values = Vec::new();
    for item in existing.into_iter().flatten() {
        if !values.iter().any(|value: &ExternalId| {
            value.source == item.source && value.source_id == item.source_id
        }) {
            values.push(item.clone());
        }
    }
    for item in incoming {
        if !values.iter().any(|value: &ExternalId| {
            value.source == item.source && value.source_id == item.source_id
        }) {
            values.push(item.clone());
        }
    }
    values
}

fn merge_source_payloads(
    existing: Option<&[SourcePayload]>,
    incoming: &[SourcePayload],
) -> Vec<SourcePayload> {
    let mut values = Vec::new();
    for item in existing.into_iter().flatten() {
        if !values.iter().any(|value: &SourcePayload| {
            value.source == item.source && value.source_id == item.source_id
        }) {
            values.push(item.clone());
        }
    }
    for item in incoming {
        if let Some(existing_item) = values.iter_mut().find(|value: &&mut SourcePayload| {
            value.source == item.source && value.source_id == item.source_id
        }) {
            *existing_item = item.clone();
        } else {
            values.push(item.clone());
        }
    }
    values
}

fn parse_media_kind(value: &str) -> Result<MediaKind> {
    value.parse()
}

fn parse_source(value: &str) -> Result<SourceName> {
    value.parse()
}

fn parse_stored_episode(
    row: &rusqlite::Row<'_>,
) -> std::result::Result<StoredEpisode, rusqlite::Error> {
    let source =
        parse_source(row.get_ref(1)?.as_str()?).map_err(|err| rusqlite_decode_error(1, err))?;
    let media_kind =
        parse_media_kind(row.get_ref(4)?.as_str()?).map_err(|err| rusqlite_decode_error(4, err))?;
    let raw_titles_json = row
        .get::<_, Option<String>>(14)?
        .map(|value| serde_json::from_str::<Value>(&value).ok())
        .flatten();
    let raw_json = row
        .get::<_, Option<String>>(15)?
        .map(|value| serde_json::from_str::<Value>(&value).ok())
        .flatten();

    Ok(StoredEpisode {
        id: row.get(0)?,
        source,
        source_id: row.get(2)?,
        media_id: row.get(3)?,
        media_kind,
        season_number: row.get(5)?,
        episode_number: row.get(6)?,
        absolute_number: row.get(7)?,
        title_display: row.get(8)?,
        title_original: row.get(9)?,
        synopsis: row.get(10)?,
        air_date: row.get(11)?,
        runtime_minutes: row.get(12)?,
        thumbnail_url: row.get(13)?,
        raw_titles_json,
        raw_json,
    })
}

fn normalize_aliases(aliases: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for alias in aliases {
        let trimmed = alias.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !result
            .iter()
            .any(|item: &String| item.eq_ignore_ascii_case(trimmed))
        {
            result.push(trimmed.to_string());
        }
    }
    result
}

fn normalize_for_lookup(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch.is_whitespace() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_fts_query(query: &str) -> Result<String> {
    let normalized = normalize_for_lookup(query);
    let mut terms = Vec::new();
    for token in normalized.split_whitespace() {
        if token.is_empty() {
            continue;
        }
        let term = if token.chars().count() > 1 {
            format!("{token}*")
        } else {
            token.to_string()
        };
        terms.push(term);
    }

    if terms.is_empty() {
        return Err(Error::Validation("search query cannot be empty".into()));
    }

    Ok(terms.join(" "))
}

fn stable_payload_hash(payload: &str) -> Result<String> {
    Ok(payload.len().to_string())
}

fn rusqlite_decode_error(
    column: usize,
    err: impl StdError + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(column, rusqlite::types::Type::Text, Box::new(err))
}

fn now_string() -> String {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    unix.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        CanonicalMedia, ExternalId, MediaKind, SearchOptions, SourceName, SourcePayload,
    };

    fn sample_media() -> CanonicalMedia {
        CanonicalMedia {
            media_kind: MediaKind::Anime,
            title_display: "Monster".into(),
            title_romaji: Some("Monster".into()),
            title_english: Some("Monster".into()),
            title_native: Some("MONSTER".into()),
            synopsis: Some("A surgeon chases a serial killer across Europe.".into()),
            format: Some("TV".into()),
            status: Some("FINISHED".into()),
            season: Some("spring".into()),
            season_year: Some(2004),
            episodes: Some(74),
            chapters: None,
            volumes: None,
            country_of_origin: Some("JP".into()),
            cover_image: Some("http://cdn.example/monster.jpg".into()),
            banner_image: Some("https://cdn.example/monster-banner.webp".into()),
            provider_rating: Some(0.55),
            nsfw: false,
            aliases: vec!["Naoki Urasawa's Monster".into()],
            genres: vec!["Mystery".into(), "Thriller".into()],
            tags: vec!["Psychological".into()],
            external_ids: vec![
                ExternalId {
                    source: SourceName::AniList,
                    source_id: "19".into(),
                    url: Some("https://anilist.co/anime/19".into()),
                },
                ExternalId {
                    source: SourceName::MyAnimeList,
                    source_id: "19".into(),
                    url: Some("https://myanimelist.net/anime/19".into()),
                },
            ],
            source_payloads: vec![SourcePayload {
                source: SourceName::AniList,
                source_id: "19".into(),
                url: Some("https://anilist.co/anime/19".into()),
                remote_updated_at: Some("1712440000".into()),
                raw_json: None,
            }],
            field_provenance: Vec::new(),
        }
    }

    fn jikan_variant() -> CanonicalMedia {
        CanonicalMedia {
            media_kind: MediaKind::Anime,
            title_display: "Monster".into(),
            title_romaji: Some("Monster".into()),
            title_english: Some("Monster".into()),
            title_native: Some("MONSTER".into()),
            synopsis: Some(
                "Dr. Kenzo Tenma saves a child who grows into a serial killer, forcing him into a long pursuit across Europe while confronting guilt, identity and systemic corruption.".into(),
            ),
            format: Some("TV".into()),
            status: Some("Finished Airing".into()),
            season: Some("spring".into()),
            season_year: Some(2004),
            episodes: Some(74),
            chapters: None,
            volumes: None,
            country_of_origin: None,
            cover_image: Some("https://cdn.jikan.example/monster-original.webp".into()),
            banner_image: None,
            provider_rating: Some(0.79),
            nsfw: false,
            aliases: vec!["Monster".into(), "Naoki Urasawa's Monster".into()],
            genres: vec!["Mystery".into(), "Suspense".into()],
            tags: vec!["Psychological".into(), "Adult Cast".into()],
            external_ids: vec![
                ExternalId {
                    source: SourceName::Jikan,
                    source_id: "19".into(),
                    url: Some("https://api.jikan.moe/v4/anime/19".into()),
                },
                ExternalId {
                    source: SourceName::MyAnimeList,
                    source_id: "19".into(),
                    url: Some("https://myanimelist.net/anime/19".into()),
                },
            ],
            source_payloads: vec![SourcePayload {
                source: SourceName::Jikan,
                source_id: "19".into(),
                url: Some("https://api.jikan.moe/v4/anime/19".into()),
                remote_updated_at: Some("2026-04-07T00:00:00+00:00".into()),
                raw_json: None,
            }],
            field_provenance: Vec::new(),
        }
    }

    #[test]
    fn upsert_and_lookup_by_external_id() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");
        let media_id = db.upsert_media(&sample_media()).expect("upsert");
        let loaded = db
            .get_by_external_id(SourceName::AniList, "19")
            .expect("lookup");

        assert_eq!(media_id, loaded.id);
        assert_eq!(loaded.title_display, "Monster");
        assert_eq!(loaded.external_ids.len(), 2);
    }

    #[test]
    fn search_uses_fts() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");
        db.upsert_media(&sample_media()).expect("upsert");

        let hits = db
            .search(
                "serial killer europe",
                SearchOptions {
                    limit: 10,
                    offset: 0,
                    media_kind: Some(MediaKind::Anime),
                    format: None,
                },
            )
            .expect("search");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title_display, "Monster");
    }

    #[test]
    fn upsert_show_with_tvmaze_source() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");
        let show = CanonicalMedia {
            media_kind: MediaKind::Show,
            title_display: "Breaking Bad".into(),
            title_romaji: None,
            title_english: Some("Breaking Bad".into()),
            title_native: None,
            synopsis: Some("A chemistry teacher turns to making meth.".into()),
            format: None,
            status: Some("Ended".into()),
            season: Some("2008".into()),
            season_year: Some(2008),
            episodes: Some(62),
            chapters: None,
            volumes: None,
            country_of_origin: Some("US".into()),
            cover_image: Some(
                "https://static.tvmaze.com/uploads/images/original_untouched/0/2000.jpg".into(),
            ),
            banner_image: None,
            provider_rating: Some(0.95),
            nsfw: false,
            aliases: vec!["BB".into()],
            genres: vec!["Drama".into(), "Crime".into()],
            tags: vec![],
            external_ids: vec![
                ExternalId {
                    source: SourceName::Tvmaze,
                    source_id: "169".into(),
                    url: Some("https://www.tvmaze.com/shows/169/breaking-bad".into()),
                },
                ExternalId {
                    source: SourceName::Imdb,
                    source_id: "tt0903747".into(),
                    url: Some("https://www.imdb.com/title/tt0903747".into()),
                },
            ],
            source_payloads: vec![SourcePayload {
                source: SourceName::Tvmaze,
                source_id: "169".into(),
                url: Some("https://www.tvmaze.com/shows/169/breaking-bad".into()),
                remote_updated_at: None,
                raw_json: None,
            }],
            field_provenance: Vec::new(),
        };

        let media_id = db.upsert_media(&show).expect("upsert show");
        let loaded = db.get_media(media_id).expect("load show");

        assert_eq!(loaded.media_kind, MediaKind::Show);
        assert_eq!(loaded.title_display, "Breaking Bad");
        assert_eq!(loaded.season_year, Some(2008));
        assert!(
            loaded
                .external_ids
                .iter()
                .any(|id| id.source == SourceName::Tvmaze)
        );
        assert!(
            loaded
                .external_ids
                .iter()
                .any(|id| id.source == SourceName::Imdb)
        );
    }

    #[test]
    fn upsert_movie_with_imdb_source() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");
        let movie = CanonicalMedia {
            media_kind: MediaKind::Movie,
            title_display: "The Shawshank Redemption".into(),
            title_romaji: None,
            title_english: Some("The Shawshank Redemption".into()),
            title_native: None,
            synopsis: None,
            format: Some("movie".into()),
            status: None,
            season: Some("1994".into()),
            season_year: Some(1994),
            episodes: Some(142),
            chapters: None,
            volumes: None,
            country_of_origin: Some("US".into()),
            cover_image: None,
            banner_image: None,
            provider_rating: Some(0.97),
            nsfw: false,
            aliases: vec!["Shawshank".into()],
            genres: vec!["Drama".into()],
            tags: vec![],
            external_ids: vec![ExternalId {
                source: SourceName::Imdb,
                source_id: "tt0111161".into(),
                url: Some("https://www.imdb.com/title/tt0111161".into()),
            }],
            source_payloads: vec![SourcePayload {
                source: SourceName::Imdb,
                source_id: "tt0111161".into(),
                url: Some("https://www.imdb.com/title/tt0111161".into()),
                remote_updated_at: None,
                raw_json: None,
            }],
            field_provenance: Vec::new(),
        };

        let media_id = db.upsert_media(&movie).expect("upsert movie");
        let loaded = db.get_media(media_id).expect("load movie");

        assert_eq!(loaded.media_kind, MediaKind::Movie);
        assert_eq!(loaded.title_display, "The Shawshank Redemption");
        assert_eq!(loaded.season_year, Some(1994));
    }

    #[test]
    fn search_show_by_kind() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");

        let anime = CanonicalMedia {
            media_kind: MediaKind::Anime,
            title_display: "Monster".into(),
            title_romaji: Some("Monster".into()),
            title_english: Some("Monster".into()),
            title_native: None,
            synopsis: Some("A surgeon chases a killer.".into()),
            format: Some("TV".into()),
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
            aliases: vec![],
            genres: vec![],
            tags: vec![],
            external_ids: vec![ExternalId {
                source: SourceName::AniList,
                source_id: "19".into(),
                url: None,
            }],
            source_payloads: vec![],
            field_provenance: Vec::new(),
        };

        let show = CanonicalMedia {
            media_kind: MediaKind::Show,
            title_display: "Breaking Bad".into(),
            title_romaji: None,
            title_english: None,
            title_native: None,
            synopsis: Some("A chemistry teacher makes meth.".into()),
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
            aliases: vec![],
            genres: vec![],
            tags: vec![],
            external_ids: vec![ExternalId {
                source: SourceName::Tvmaze,
                source_id: "169".into(),
                url: None,
            }],
            source_payloads: vec![],
            field_provenance: Vec::new(),
        };

        db.upsert_media(&anime).expect("upsert anime");
        db.upsert_media(&show).expect("upsert show");

        let show_hits = db
            .search(
                "teacher",
                SearchOptions {
                    limit: 10,
                    offset: 0,
                    media_kind: Some(MediaKind::Show),
                    format: None,
                },
            )
            .expect("search show");

        assert_eq!(show_hits.len(), 1);
        assert_eq!(show_hits[0].title_display, "Breaking Bad");
        assert_eq!(show_hits[0].media_kind, MediaKind::Show);

        let anime_hits = db
            .search(
                "surgeon",
                SearchOptions {
                    limit: 10,
                    offset: 0,
                    media_kind: Some(MediaKind::Anime),
                    format: None,
                },
            )
            .expect("search anime");

        assert_eq!(anime_hits.len(), 1);
        assert_eq!(anime_hits[0].title_display, "Monster");
        assert_eq!(anime_hits[0].media_kind, MediaKind::Anime);
    }

    #[test]
    fn merges_tvmaze_and_imdb_into_one_show() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");

        let tvmaze_show = CanonicalMedia {
            media_kind: MediaKind::Show,
            title_display: "Breaking Bad".into(),
            title_romaji: None,
            title_english: Some("Breaking Bad".into()),
            title_native: None,
            synopsis: Some("A high school chemistry teacher diagnosed with lung cancer turns to manufacturing methamphetamine.".into()),
            format: None,
            status: Some("Ended".into()),
            season: Some("2008".into()),
            season_year: Some(2008),
            episodes: Some(62),
            chapters: None,
            volumes: None,
            country_of_origin: Some("US".into()),
            cover_image: Some("https://static.tvmaze.com/uploads/images/original_untouched/0/2000.jpg".into()),
            banner_image: None,
            provider_rating: Some(0.96),
            nsfw: false,
            aliases: vec!["BB".into()],
            genres: vec!["Drama".into(), "Crime".into()],
            tags: vec![],
            external_ids: vec![
                ExternalId {
                    source: SourceName::Tvmaze,
                    source_id: "169".into(),
                    url: Some("https://www.tvmaze.com/shows/169/breaking-bad".into()),
                },
                ExternalId {
                    source: SourceName::Imdb,
                    source_id: "tt0903747".into(),
                    url: Some("https://www.imdb.com/title/tt0903747".into()),
                },
            ],
            source_payloads: vec![SourcePayload {
                source: SourceName::Tvmaze,
                source_id: "169".into(),
                url: Some("https://www.tvmaze.com/shows/169/breaking-bad".into()),
                remote_updated_at: None,
                raw_json: None,
            }],
            field_provenance: Vec::new(),
        };

        let imdb_show = CanonicalMedia {
            media_kind: MediaKind::Show,
            title_display: "Breaking Bad".into(),
            title_romaji: None,
            title_english: None,
            title_native: None,
            synopsis: None,
            format: Some("tvSeries".into()),
            status: None,
            season: Some("2008".into()),
            season_year: Some(2008),
            episodes: Some(62),
            chapters: None,
            volumes: None,
            country_of_origin: Some("US".into()),
            cover_image: None,
            banner_image: None,
            provider_rating: Some(0.99),
            nsfw: false,
            aliases: vec![],
            genres: vec!["Crime".into(), "Drama".into(), "Thriller".into()],
            tags: vec![],
            external_ids: vec![ExternalId {
                source: SourceName::Imdb,
                source_id: "tt0903747".into(),
                url: Some("https://www.imdb.com/title/tt0903747".into()),
            }],
            source_payloads: vec![SourcePayload {
                source: SourceName::Imdb,
                source_id: "tt0903747".into(),
                url: Some("https://www.imdb.com/title/tt0903747".into()),
                remote_updated_at: None,
                raw_json: None,
            }],
            field_provenance: Vec::new(),
        };

        let first_id = db.upsert_media(&tvmaze_show).expect("upsert tvmaze");
        let second_id = db.upsert_media(&imdb_show).expect("upsert imdb");

        assert_eq!(first_id, second_id);

        let loaded = db
            .get_by_external_id(SourceName::Imdb, "tt0903747")
            .expect("lookup by imdb");

        assert_eq!(loaded.title_display, "Breaking Bad");
        assert_eq!(loaded.media_kind, MediaKind::Show);
        assert!(
            loaded
                .external_ids
                .iter()
                .any(|id| id.source == SourceName::Tvmaze)
        );
        assert!(
            loaded
                .external_ids
                .iter()
                .any(|id| id.source == SourceName::Imdb)
        );
        assert!(loaded.genres.contains(&"Drama".to_string()));
        assert!(loaded.genres.contains(&"Crime".to_string()));
    }

    #[test]
    fn same_title_different_kinds_are_separate_records() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");

        let anime = CanonicalMedia {
            media_kind: MediaKind::Anime,
            title_display: "The Matrix".into(),
            title_english: Some("The Matrix".into()),
            ..minimal_media(SourceName::AniList, "100")
        };

        let movie = CanonicalMedia {
            media_kind: MediaKind::Movie,
            title_display: "The Matrix".into(),
            title_english: Some("The Matrix".into()),
            ..minimal_media(SourceName::Imdb, "tt0133093")
        };

        let anime_id = db.upsert_media(&anime).expect("upsert anime");
        let movie_id = db.upsert_media(&movie).expect("upsert movie");

        assert_ne!(
            anime_id, movie_id,
            "different kinds must be separate records"
        );

        let loaded_anime = db.get_media(anime_id).expect("load anime");
        let loaded_movie = db.get_media(movie_id).expect("load movie");
        assert_eq!(loaded_anime.media_kind, MediaKind::Anime);
        assert_eq!(loaded_movie.media_kind, MediaKind::Movie);
    }

    #[test]
    fn same_imdb_id_different_kinds_does_not_conflict() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");

        let show = CanonicalMedia {
            media_kind: MediaKind::Show,
            title_display: "The Peripheral".into(),
            ..minimal_media_with_external_ids(
                SourceName::Imdb,
                "tt11111",
                vec![
                    ExternalId {
                        source: SourceName::Imdb,
                        source_id: "tt11111".into(),
                        url: None,
                    },
                    ExternalId {
                        source: SourceName::Tvmaze,
                        source_id: "500".into(),
                        url: None,
                    },
                ],
            )
        };

        let movie = CanonicalMedia {
            media_kind: MediaKind::Movie,
            title_display: "The Peripheral".into(),
            ..minimal_media_with_external_ids(
                SourceName::Imdb,
                "tt22222",
                vec![
                    ExternalId {
                        source: SourceName::Imdb,
                        source_id: "tt22222".into(),
                        url: None,
                    },
                    ExternalId {
                        source: SourceName::Tvmaze,
                        source_id: "501".into(),
                        url: None,
                    },
                ],
            )
        };

        let show_id = db.upsert_media(&show).expect("upsert show");
        let movie_id = db.upsert_media(&movie).expect("upsert movie");

        assert_ne!(show_id, movie_id);

        let show_loaded = db
            .get_by_external_id_and_kind(SourceName::Imdb, MediaKind::Show, "tt11111")
            .expect("lookup show by kind");
        assert_eq!(show_loaded.media_kind, MediaKind::Show);

        let movie_loaded = db
            .get_by_external_id_and_kind(SourceName::Imdb, MediaKind::Movie, "tt22222")
            .expect("lookup movie by kind");
        assert_eq!(movie_loaded.media_kind, MediaKind::Movie);
    }

    #[test]
    fn search_movie_by_kind_isolation() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");

        let show = CanonicalMedia {
            media_kind: MediaKind::Show,
            title_display: "The Office".into(),
            synopsis: Some("A mockumentary about a paper company.".into()),
            ..minimal_media(SourceName::Tvmaze, "100")
        };

        let movie = CanonicalMedia {
            media_kind: MediaKind::Movie,
            title_display: "The Office Space".into(),
            synopsis: Some("A comedy about corporate life.".into()),
            ..minimal_media(SourceName::Imdb, "tt01015011")
        };

        db.upsert_media(&show).expect("upsert show");
        db.upsert_media(&movie).expect("upsert movie");

        let movie_hits = db
            .search(
                "office",
                SearchOptions {
                    limit: 10,
                    offset: 0,
                    media_kind: Some(MediaKind::Movie),
                    format: None,
                },
            )
            .expect("search movie");

        assert_eq!(movie_hits.len(), 1);
        assert_eq!(movie_hits[0].media_kind, MediaKind::Movie);
        assert_eq!(movie_hits[0].title_display, "The Office Space");

        let show_hits = db
            .search(
                "office",
                SearchOptions {
                    limit: 10,
                    offset: 0,
                    media_kind: Some(MediaKind::Show),
                    format: None,
                },
            )
            .expect("search show");

        assert_eq!(show_hits.len(), 1);
        assert_eq!(show_hits[0].media_kind, MediaKind::Show);
        assert_eq!(show_hits[0].title_display, "The Office");
    }

    #[test]
    fn search_all_kinds_returns_both() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");

        let show = CanonicalMedia {
            media_kind: MediaKind::Show,
            title_display: "Dark".into(),
            synopsis: Some("A family saga with time travel.".into()),
            ..minimal_media(SourceName::Tvmaze, "200")
        };

        let movie = CanonicalMedia {
            media_kind: MediaKind::Movie,
            title_display: "Dark City".into(),
            synopsis: Some("A man discovers reality is manipulated.".into()),
            ..minimal_media(SourceName::Imdb, "tt011911711")
        };

        db.upsert_media(&show).expect("upsert show");
        db.upsert_media(&movie).expect("upsert movie");

        let all_hits = db
            .search(
                "dark",
                SearchOptions {
                    limit: 10,
                    offset: 0,
                    media_kind: None,
                    format: None,
                },
            )
            .expect("search all kinds");

        assert_eq!(all_hits.len(), 2);
    }

    #[test]
    fn update_show_preserves_kind() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");

        let original = CanonicalMedia {
            media_kind: MediaKind::Show,
            title_display: "Stranger Things".into(),
            synopsis: Some("A girl with psychokinetic powers.".into()),
            ..minimal_media(SourceName::Tvmaze, "300")
        };

        let id_first = db.upsert_media(&original).expect("first upsert");

        let updated = CanonicalMedia {
            media_kind: MediaKind::Show,
            title_display: "Stranger Things".into(),
            synopsis: Some("A girl with powers battles monsters from the Upside Down.".into()),
            ..minimal_media(SourceName::Tvmaze, "300")
        };

        let id_second = db.upsert_media(&updated).expect("second upsert");

        assert_eq!(id_first, id_second);

        let loaded = db.get_media(id_first).expect("load");
        assert_eq!(loaded.media_kind, MediaKind::Show);
        assert_eq!(
            loaded.synopsis.as_deref(),
            Some("A girl with powers battles monsters from the Upside Down.")
        );
    }

    #[test]
    fn nsfw_flag_from_imdb_adult_content() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");

        let adult_movie = CanonicalMedia {
            media_kind: MediaKind::Movie,
            title_display: "Adult Movie".into(),
            nsfw: true,
            ..minimal_media(SourceName::Imdb, "tt9999999")
        };

        let id = db.upsert_media(&adult_movie).expect("upsert");
        let loaded = db.get_media(id).expect("load");
        assert!(loaded.nsfw);
    }

    #[test]
    fn empty_synopsis_and_no_cover_from_imdb() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");

        let sparse_movie = CanonicalMedia {
            media_kind: MediaKind::Movie,
            title_display: "Some Obscure Film".into(),
            synopsis: None,
            cover_image: None,
            banner_image: None,
            provider_rating: None,
            ..minimal_media(SourceName::Imdb, "tt0000001")
        };

        let id = db.upsert_media(&sparse_movie).expect("upsert");
        let loaded = db.get_media(id).expect("load");

        assert!(loaded.synopsis.is_none());
        assert!(loaded.cover_image.is_none());
        assert!(loaded.banner_image.is_none());
        assert_eq!(loaded.provider_rating, None);
    }

    #[test]
    fn merge_prefers_higher_rating_from_imdb_over_tvmaze() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");

        let tvmaze_entry = CanonicalMedia {
            media_kind: MediaKind::Show,
            title_display: "Chernobyl".into(),
            provider_rating: Some(0.95),
            ..minimal_media_with_external_ids(
                SourceName::Tvmaze,
                "455",
                vec![
                    ExternalId {
                        source: SourceName::Tvmaze,
                        source_id: "455".into(),
                        url: None,
                    },
                    ExternalId {
                        source: SourceName::Imdb,
                        source_id: "tt739642".into(),
                        url: None,
                    },
                ],
            )
        };

        let imdb_entry = CanonicalMedia {
            media_kind: MediaKind::Show,
            title_display: "Chernobyl".into(),
            provider_rating: Some(0.99),
            ..minimal_media_with_external_ids(
                SourceName::Imdb,
                "tt739642",
                vec![ExternalId {
                    source: SourceName::Imdb,
                    source_id: "tt739642".into(),
                    url: None,
                }],
            )
        };

        let first = db.upsert_media(&tvmaze_entry).expect("upsert tvmaze");
        let second = db.upsert_media(&imdb_entry).expect("upsert imdb");
        assert_eq!(first, second);

        let loaded = db.get_media(first).expect("load");
        assert_eq!(loaded.provider_rating, Some(0.99));
    }

    #[test]
    fn invalid_media_kind_rejected() {
        let result = "tvshow".parse::<MediaKind>();
        assert!(result.is_err());
        let result = "film".parse::<MediaKind>();
        assert!(result.is_err());
    }

    #[test]
    fn invalid_source_name_rejected() {
        let result = "netflix".parse::<SourceName>();
        assert!(result.is_err());
    }

    fn minimal_media(source: SourceName, source_id: &str) -> CanonicalMedia {
        CanonicalMedia {
            media_kind: MediaKind::Show,
            title_display: "Test Title".into(),
            title_romaji: None,
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
            aliases: vec![],
            genres: vec![],
            tags: vec![],
            external_ids: vec![ExternalId {
                source,
                source_id: source_id.into(),
                url: None,
            }],
            source_payloads: vec![SourcePayload {
                source,
                source_id: source_id.into(),
                url: None,
                remote_updated_at: None,
                raw_json: None,
            }],
            field_provenance: Vec::new(),
        }
    }

    fn minimal_media_with_external_ids(
        source: SourceName,
        source_id: &str,
        external_ids: Vec<ExternalId>,
    ) -> CanonicalMedia {
        CanonicalMedia {
            media_kind: MediaKind::Show,
            title_display: "Test Title".into(),
            title_romaji: None,
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
            aliases: vec![],
            genres: vec![],
            tags: vec![],
            external_ids,
            source_payloads: vec![SourcePayload {
                source,
                source_id: source_id.into(),
                url: None,
                remote_updated_at: None,
                raw_json: None,
            }],
            field_provenance: Vec::new(),
        }
    }

    fn sample_episode() -> CanonicalEpisode {
        CanonicalEpisode {
            source: SourceName::Kitsu,
            source_id: "ep1".into(),
            media_kind: MediaKind::Anime,
            season_number: Some(1),
            episode_number: Some(1),
            absolute_number: Some(1),
            title_display: Some("The Hospital".into()),
            title_original: Some("Byouin".into()),
            synopsis: Some("Tenma operates on a young boy.".into()),
            air_date: Some("2005-04-05".into()),
            runtime_minutes: Some(23),
            thumbnail_url: Some("https://cdn.kitsu.io/ep1_thumb.jpg".into()),
            raw_titles_json: Some(serde_json::json!({"en": "The Hospital", "ja_jp": "Byouin"})),
            raw_json: None,
        }
    }

    #[test]
    fn upsert_episode_and_retrieve_by_media() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");

        let media = CanonicalMedia {
            media_kind: MediaKind::Anime,
            title_display: "Monster".into(),
            ..minimal_media(SourceName::Kitsu, "1")
        };
        let media_id = db.upsert_media(&media).expect("upsert media");

        let episode = CanonicalEpisode {
            source: SourceName::Kitsu,
            source_id: "ep1".into(),
            media_kind: MediaKind::Anime,
            season_number: Some(1),
            episode_number: Some(1),
            absolute_number: Some(1),
            title_display: Some("The Hospital".into()),
            title_original: Some("Byouin".into()),
            synopsis: Some("Tenma operates on a young boy.".into()),
            air_date: Some("2005-04-05".into()),
            runtime_minutes: Some(23),
            thumbnail_url: None,
            raw_titles_json: None,
            raw_json: None,
        };

        let ep_id = db
            .upsert_episode(&episode, media_id)
            .expect("upsert episode");
        assert!(ep_id > 0);

        let episodes = db.episodes_for_media(media_id).expect("list episodes");
        assert_eq!(episodes.len(), 1);
        assert_eq!(episodes[0].source_id, "ep1");
        assert_eq!(episodes[0].episode_number, Some(1));
        assert_eq!(episodes[0].title_display.as_deref(), Some("The Hospital"));
    }

    #[test]
    fn upsert_episode_replaces_existing() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");

        let media = CanonicalMedia {
            media_kind: MediaKind::Anime,
            title_display: "Monster".into(),
            ..minimal_media(SourceName::Kitsu, "1")
        };
        let media_id = db.upsert_media(&media).expect("upsert media");

        let episode1 = CanonicalEpisode {
            source: SourceName::Kitsu,
            source_id: "ep1".into(),
            media_kind: MediaKind::Anime,
            season_number: Some(1),
            episode_number: Some(1),
            absolute_number: Some(1),
            title_display: Some("Original Title".into()),
            title_original: None,
            synopsis: None,
            air_date: None,
            runtime_minutes: None,
            thumbnail_url: None,
            raw_titles_json: None,
            raw_json: None,
        };

        db.upsert_episode(&episode1, media_id)
            .expect("upsert first");

        let episode2 = CanonicalEpisode {
            source: SourceName::Kitsu,
            source_id: "ep1".into(),
            media_kind: MediaKind::Anime,
            season_number: Some(1),
            episode_number: Some(1),
            absolute_number: Some(1),
            title_display: Some("Updated Title".into()),
            title_original: None,
            synopsis: Some("Updated synopsis.".into()),
            air_date: None,
            runtime_minutes: None,
            thumbnail_url: None,
            raw_titles_json: None,
            raw_json: None,
        };

        db.upsert_episode(&episode2, media_id)
            .expect("upsert second");

        let episodes = db.episodes_for_media(media_id).expect("list episodes");
        assert_eq!(episodes.len(), 1);
        assert_eq!(episodes[0].title_display.as_deref(), Some("Updated Title"));
        assert_eq!(episodes[0].synopsis.as_deref(), Some("Updated synopsis."));
    }

    #[test]
    fn episode_by_absolute_number() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");

        let media = CanonicalMedia {
            media_kind: MediaKind::Anime,
            title_display: "Monster".into(),
            ..minimal_media(SourceName::Kitsu, "1")
        };
        let media_id = db.upsert_media(&media).expect("upsert media");

        let ep1 = CanonicalEpisode {
            source: SourceName::Kitsu,
            source_id: "ep1".into(),
            media_kind: MediaKind::Anime,
            season_number: Some(1),
            episode_number: Some(1),
            absolute_number: Some(1),
            title_display: Some("Episode 1".into()),
            title_original: None,
            synopsis: None,
            air_date: None,
            runtime_minutes: None,
            thumbnail_url: None,
            raw_titles_json: None,
            raw_json: None,
        };

        let ep2 = CanonicalEpisode {
            source: SourceName::Kitsu,
            source_id: "ep2".into(),
            media_kind: MediaKind::Anime,
            season_number: Some(1),
            episode_number: Some(2),
            absolute_number: Some(2),
            title_display: Some("Episode 2".into()),
            title_original: None,
            synopsis: None,
            air_date: None,
            runtime_minutes: None,
            thumbnail_url: None,
            raw_titles_json: None,
            raw_json: None,
        };

        db.upsert_episode(&ep1, media_id).expect("upsert ep1");
        db.upsert_episode(&ep2, media_id).expect("upsert ep2");

        let found = db
            .episode_by_absolute_number(media_id, 2)
            .expect("find by absolute");
        assert_eq!(found.episode_number, Some(2));

        let not_found = db.episode_by_absolute_number(media_id, 99);
        assert!(not_found.is_err());
    }

    #[test]
    fn episode_by_season_episode() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");

        let media = CanonicalMedia {
            media_kind: MediaKind::Anime,
            title_display: "Monster".into(),
            ..minimal_media(SourceName::Kitsu, "1")
        };
        let media_id = db.upsert_media(&media).expect("upsert media");

        let ep = CanonicalEpisode {
            source: SourceName::Kitsu,
            source_id: "ep1s1".into(),
            media_kind: MediaKind::Anime,
            season_number: Some(1),
            episode_number: Some(5),
            absolute_number: Some(5),
            title_display: Some("Season 1 Episode 5".into()),
            title_original: None,
            synopsis: None,
            air_date: None,
            runtime_minutes: None,
            thumbnail_url: None,
            raw_titles_json: None,
            raw_json: None,
        };

        db.upsert_episode(&ep, media_id).expect("upsert");

        let found = db
            .episode_by_season_episode(media_id, 1, 5)
            .expect("find by season/episode");
        assert_eq!(found.season_number, Some(1));
        assert_eq!(found.episode_number, Some(5));
    }

    #[test]
    fn media_document_by_id_returns_media_and_episodes() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");

        let media = CanonicalMedia {
            media_kind: MediaKind::Anime,
            title_display: "Monster".into(),
            episodes: Some(74),
            ..minimal_media(SourceName::Kitsu, "1")
        };
        let media_id = db.upsert_media(&media).expect("upsert media");

        let ep1 = CanonicalEpisode {
            source: SourceName::Kitsu,
            source_id: "ep1".into(),
            media_kind: MediaKind::Anime,
            season_number: Some(1),
            episode_number: Some(1),
            absolute_number: Some(1),
            title_display: Some("First Episode".into()),
            title_original: None,
            synopsis: None,
            air_date: None,
            runtime_minutes: None,
            thumbnail_url: None,
            raw_titles_json: None,
            raw_json: None,
        };

        db.upsert_episode(&ep1, media_id).expect("upsert ep");

        let doc = db.media_document_by_id(media_id).expect("get doc");
        assert_eq!(doc.media.title_display, "Monster");
        assert_eq!(doc.media.episodes, Some(74));
        assert_eq!(doc.episodes.len(), 1);
        assert_eq!(
            doc.episodes[0].title_display.as_deref(),
            Some("First Episode")
        );
    }

    #[test]
    fn media_document_by_external_id() {
        let mut db = AnimeDb::open_in_memory().expect("in-memory db");

        let media = CanonicalMedia {
            media_kind: MediaKind::Anime,
            title_display: "Monster".into(),
            ..minimal_media(SourceName::Kitsu, "1")
        };
        db.upsert_media(&media).expect("upsert media");

        let doc = db
            .media_document_by_external_id(SourceName::Kitsu, "1")
            .expect("get doc by external id");
        assert_eq!(doc.media.title_display, "Monster");
        assert!(doc.episodes.is_empty());
    }

    #[test]
    fn provider_without_episode_support_returns_error() {
        use crate::provider::Provider;

        // AniList does not implement fetch_episodes, so it returns a controlled error
        let anilist = AniListProvider::new();
        let result = anilist.fetch_episodes(MediaKind::Anime, "1");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("does not support episode metadata"),
            "got: {err_msg}"
        );

        // Jikan does not implement fetch_episodes
        let jikan = JikanProvider::new();
        let result = jikan.fetch_episodes(MediaKind::Anime, "1");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("does not support episode metadata"),
            "got: {err_msg}"
        );
    }

    #[test]
    fn kitsu_provider_does_implement_fetch_episodes() {
        use crate::provider::Provider;

        // Kitsu DOES implement fetch_episodes, but gets a 404 for non-existent anime
        // which proves the method is implemented (not "not supported")
        let kitsu = KitsuProvider::new();
        let result = kitsu.fetch_episodes(MediaKind::Anime, "99999");
        // 404 is expected for non-existent anime ID; what matters is it's NOT
        // "does not support episode metadata" error
        assert!(
            result.is_err() && !result.unwrap_err().to_string().contains("does not support"),
            "Kitsu should implement fetch_episodes (got network/404 error, not unsupported)"
        );
    }
}
