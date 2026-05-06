use crate::error::{Error, Result};
use crate::model::{
    CanonicalEpisode, CanonicalMedia, EpisodeSourceRecord, MediaDocument, MediaKind, SearchHit,
    SearchOptions, SourceName, StoredEpisode, StoredMedia, SyncOutcome, SyncReport, SyncRequest,
};
use crate::provider::Provider;
use crate::remote::{RemoteApi, RemoteSource};
use rusqlite::Connection;
use std::path::Path;

/// Local-first SQLite-backed catalog entry point.
///
/// `AnimeDb` owns:
/// - SQLite schema creation and migration (automatic on [`open`](AnimeDb::open))
/// - Provider sync orchestration via [`sync_service`](AnimeDb::sync_service)
/// - Merge materialization during upsert
/// - Typed repository facades for media, episodes, search, and sync state
/// - Access to the raw SQLite connection for advanced use cases
///
/// # Example — fresh catalog with bootstrap sync
/// ```ignore
/// let (mut db, report) = AnimeDb::generate_database_with_report("/tmp/animedb.sqlite")?;
/// println!("synced {} records", report.total_upserted_records);
/// ```
///
/// # Example — open existing and query
/// ```ignore
/// let db = AnimeDb::open("/tmp/animedb.sqlite")?;
/// let monster = db.anime_metadata().by_external_id(SourceName::AniList, "19")?;
/// ```
///
/// # Feature gating
///
/// Requires the `local-db` feature (enabled by default). Without it, only the
/// remote-first API via [`RemoteApi`](crate::remote::RemoteApi) is available.
pub struct AnimeDb {
    conn: Connection,
}

impl AnimeDb {
    /// Upserts a canonical media record. Returns the local `media.id`.
    ///
    /// If an existing record matches the same external ID + media kind, the merge
    /// engine scores incoming fields against stored provenance and keeps the higher.
    pub fn upsert_media(&mut self, media: &CanonicalMedia) -> Result<i64> {
        crate::repository::MediaRepository::upsert_media(&mut self.conn, media)
    }
    /// Fetches a media record by its local primary key.
    pub fn get_media(&self, media_id: i64) -> Result<StoredMedia> {
        self.media().get_media(media_id)
    }
    /// Fetches a media record by provider + source ID. Raises [`Error::ConflictingExternalId`]
    /// if the same ID resolves to multiple media kinds.
    pub fn get_by_external_id(&self, source: SourceName, source_id: &str) -> Result<StoredMedia> {
        self.media().get_by_external_id(source, source_id)
    }
    /// Kind-specific variant of [`get_by_external_id`](AnimeDb::get_by_external_id).
    pub fn get_by_external_id_and_kind(
        &self,
        source: SourceName,
        kind: MediaKind,
        source_id: &str,
    ) -> Result<StoredMedia> {
        self.media()
            .get_by_external_id_and_kind(source, kind, source_id)
    }
    /// Full-text search over title, aliases, and synopsis.
    pub fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<SearchHit>> {
        self.search_repo().search(query, options)
    }
    /// Upserts a canonical episode record linked to a media item.
    pub fn upsert_episode(&mut self, episode: &CanonicalEpisode, media_id: i64) -> Result<i64> {
        self.episodes().upsert_episode(episode, media_id)
    }
    /// Lists all canonical episode records for a media item.
    pub fn episodes_for_media(&self, media_id: i64) -> Result<Vec<StoredEpisode>> {
        self.episodes().episodes_for_media(media_id)
    }
    /// Lists all raw source records for a media item (for audit or re-merging).
    pub fn episode_source_records_for_media(
        &self,
        media_id: i64,
    ) -> Result<Vec<EpisodeSourceRecord>> {
        self.episodes().episode_source_records_for_media(media_id)
    }
    /// Looks up a single episode by its absolute number across all seasons.
    pub fn episode_by_absolute_number(
        &self,
        media_id: i64,
        abs_num: i32,
    ) -> Result<Option<StoredEpisode>> {
        self.episodes()
            .episode_by_absolute_number(media_id, abs_num)
    }
    /// Looks up a single episode by season + episode number.
    pub fn episode_by_season_episode(
        &self,
        media_id: i64,
        s: i32,
        e: i32,
    ) -> Result<Option<StoredEpisode>> {
        self.episodes().episode_by_season_episode(media_id, s, e)
    }
    /// Returns the media record and its full episode list.
    pub fn media_document_by_id(&self, media_id: i64) -> Result<MediaDocument> {
        self.search_repo().media_document_by_id(media_id)
    }
    /// Returns the media record and its full episode list by external ID.
    pub fn media_document_by_external_id(
        &self,
        source: SourceName,
        source_id: &str,
    ) -> Result<MediaDocument> {
        self.search_repo()
            .media_document_by_external_id(source, source_id)
    }

    /// Looks up a list of episodes by external provider ID.
    pub fn get_episodes_by_external_id(
        &self,
        source: SourceName,
        source_id: &str,
    ) -> Result<Vec<StoredEpisode>> {
        let media = self.media().get_by_external_id(source, source_id)?;
        self.episodes_for_media(media.id)
    }

    /// Returns a [`crate::sync::SyncService`] reference for orchestrating provider syncs.
    pub fn sync_service(&mut self) -> crate::sync::SyncService<'_> {
        crate::sync::SyncService { db: self }
    }

    /// Syncs from a single provider using a custom [`SyncRequest`].
    pub fn sync_from<P: Provider>(
        &mut self,
        provider: &P,
        request: SyncRequest,
    ) -> Result<SyncOutcome> {
        self.sync_service().sync_from(provider, request)
    }

    /// Syncs all default providers (AniList, Jikan, Kitsu, TVmaze, IMDb).
    pub fn sync_default_sources(&mut self) -> Result<SyncReport> {
        self.sync_service().sync_default_sources()
    }

    /// Opens a database at `path` and runs the full default sync. Returns only the [`AnimeDb`].
    pub fn sync_database(path: impl AsRef<Path>) -> Result<SyncReport> {
        crate::sync::SyncService::sync_database(path)
    }

    /// Syncs AniList for one media kind.
    pub fn sync_anilist(&mut self, media_kind: MediaKind) -> Result<SyncOutcome> {
        self.sync_service().sync_anilist(media_kind)
    }

    /// Syncs Jikan for one media kind.
    pub fn sync_jikan(&mut self, media_kind: MediaKind) -> Result<SyncOutcome> {
        self.sync_service().sync_jikan(media_kind)
    }

    /// Syncs Kitsu for one media kind.
    pub fn sync_kitsu(&mut self, media_kind: MediaKind) -> Result<SyncOutcome> {
        self.sync_service().sync_kitsu(media_kind)
    }

    /// Syncs TVmaze (shows only).
    pub fn sync_tvmaze(&mut self) -> Result<SyncOutcome> {
        self.sync_service().sync_tvmaze()
    }

    /// Syncs IMDb for one media kind.
    pub fn sync_imdb(&mut self, media_kind: MediaKind) -> Result<SyncOutcome> {
        self.sync_service().sync_imdb(media_kind)
    }

    /// Returns a [`crate::repository::MediaRepository`] for direct media access.
    pub fn media(&self) -> crate::repository::MediaRepository<'_> {
        crate::repository::MediaRepository { conn: &self.conn }
    }

    /// Returns a [`crate::repository::EpisodeRepository`] for direct episode access.
    pub fn episodes(&self) -> crate::repository::EpisodeRepository<'_> {
        crate::repository::EpisodeRepository { conn: &self.conn }
    }

    /// Returns a [`crate::repository::SearchRepository`] for direct search access.
    pub fn search_repo(&self) -> crate::repository::SearchRepository<'_> {
        crate::repository::SearchRepository { conn: &self.conn }
    }

    /// Returns a [`crate::repository::SyncStateRepository`] for sync checkpoint management.
    pub fn sync_state(&self) -> crate::repository::SyncStateRepository<'_> {
        crate::repository::SyncStateRepository { conn: &self.conn }
    }

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

    /// Syncs AniList records for one media kind into the local database.

    /// Syncs Jikan records for one media kind into the local database.

    /// Syncs Kitsu records for one media kind into the local database.

    /// Syncs TVmaze show records into the local database.

    /// Syncs IMDb records for one media kind into the local database.

    /// Opens or creates a SQLite catalog, applies runtime pragmas, and runs migrations.
    ///
    /// Migration is automatic. The schema version is stored in `PRAGMA user_version`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        crate::schema::configure(&conn)?;
        crate::schema::migrate(&conn)?;
        Ok(Self { conn })
    }

    /// Opens an in-memory SQLite catalog with the same pragmas and migrations as a file-backed DB.
    /// Useful for tests.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        crate::schema::configure(&conn)?;
        crate::schema::migrate(&conn)?;
        Ok(Self { conn })
    }

    /// Exposes the underlying SQLite connection for advanced integrations.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Returns a filtered query facade for anime records.
    pub fn anime_metadata(&self) -> MetadataCollection<'_> {
        MetadataCollection::new(
            self,
            SearchOptions::default().with_media_kind(MediaKind::Anime),
        )
    }

    /// Returns a filtered query facade for manga records.
    pub fn manga_metadata(&self) -> MetadataCollection<'_> {
        MetadataCollection::new(
            self,
            SearchOptions::default().with_media_kind(MediaKind::Manga),
        )
    }

    /// Returns a filtered query facade for anime movies.
    pub fn movie_metadata(&self) -> MetadataCollection<'_> {
        MetadataCollection::new(
            self,
            SearchOptions::default()
                .with_media_kind(MediaKind::Anime)
                .with_format("MOVIE"),
        )
    }

    /// Returns a filtered query facade for TV show records.
    pub fn show_metadata(&self) -> MetadataCollection<'_> {
        MetadataCollection::new(
            self,
            SearchOptions::default().with_media_kind(MediaKind::Show),
        )
    }

    /// Returns a filtered query facade for movie records.
    pub fn tv_movie_metadata(&self) -> MetadataCollection<'_> {
        MetadataCollection::new(
            self,
            SearchOptions::default().with_media_kind(MediaKind::Movie),
        )
    }

    /// Inserts a source episode record, then merges to update canonical episodes.

    /// Backward-compatible alias for `upsert_episode_source_record`.
    #[allow(dead_code)]

    /// Merges all source records for a media item into canonical episodes.
    ///
    /// Groups by `media_id + absolute_number` (fallback: `media_id + season_number + episode_number`),
    /// picks field values from highest-priority provider (AniList > IMDb > TVmaze > Jikan > Kitsu).

    /// Inserts or updates a canonical episode.
    /// Uses simple "find or insert" strategy since canonical episodes don't have a strong identity key.

    /// Returns all source episode records for a media item (for audit/debug).

    /// Fetches episodes from a registered provider and stores them for a media item.
    ///
    /// # Example
    /// ```ignore
    /// db.fetch_and_store_episodes(SourceName::Kitsu, "1")?;
    /// let doc = db.media_document_by_external_id(SourceName::Kitsu, "1")?;
    /// for ep in doc.episodes {
    ///     println!("{} - {}", ep.absolute_number, ep.title_display);
    /// }
    /// ```
    pub fn fetch_and_store_episodes(
        &mut self,
        source: SourceName,
        source_id: &str,
    ) -> Result<Vec<StoredEpisode>> {
        let provider = crate::provider::registry::default_registry().get(source)?;
        self.fetch_and_store_episodes_from(&*provider, source, source_id)
    }

    /// Same as [`fetch_and_store_episodes`](AnimeDb::fetch_and_store_episodes) but accepts a
    /// generic [`Provider`] for testing or custom provider use.
    pub fn fetch_and_store_episodes_from(
        &mut self,
        provider: &dyn Provider,
        source: SourceName,
        source_id: &str,
    ) -> Result<Vec<StoredEpisode>> {
        let media = self
            .media()
            .get_by_external_id_and_kind(source, MediaKind::Anime, source_id)
            .or_else(|_| {
                self.media()
                    .get_by_external_id_and_kind(source, MediaKind::Show, source_id)
            })?;

        let episodes = provider.fetch_episodes(media.media_kind, source_id)?;

        self.store_episode_source_records(media.id, &episodes)
    }

    /// Fetches episodes from every episode-capable provider ID on a stored media record.
    ///
    /// This is the unified local-first episode enrichment path. It uses the media record's
    /// merged external IDs to query all supported episode sources, stores each successful
    /// provider response as an episode source record, then runs one canonical merge.
    pub fn fetch_and_store_episodes_for_media(
        &mut self,
        media_id: i64,
    ) -> Result<Vec<StoredEpisode>> {
        let media = self.get_media(media_id)?;
        let episodes =
            RemoteApi::fetch_episodes_from_external_ids(media.media_kind, &media.external_ids)?;

        self.store_episode_source_records(media.id, &episodes)
    }

    /// Finds a media record by external ID, then fetches and stores episodes from all
    /// episode-capable source IDs attached to that merged record.
    pub fn fetch_and_store_episodes_by_external_id(
        &mut self,
        source: SourceName,
        source_id: &str,
    ) -> Result<Vec<StoredEpisode>> {
        let media = self
            .media()
            .get_by_external_id_and_kind(source, MediaKind::Anime, source_id)
            .or_else(|_| {
                self.media()
                    .get_by_external_id_and_kind(source, MediaKind::Show, source_id)
            })?;

        let episodes =
            RemoteApi::fetch_episodes_from_external_ids(media.media_kind, &media.external_ids)?;

        self.store_episode_source_records(media.id, &episodes)
    }

    fn store_episode_source_records(
        &mut self,
        media_id: i64,
        episodes: &[CanonicalEpisode],
    ) -> Result<Vec<StoredEpisode>> {
        for episode in episodes {
            self.episodes()
                .upsert_episode_source_record_no_merge(episode, media_id)?;
        }

        self.episodes().merge_episodes_for_media(media_id)?;
        self.episodes().episodes_for_media(media_id)
    }
}

/// Typed query facade over one local media slice.
///
/// Use via [`AnimeDb::anime_metadata`](AnimeDb::anime_metadata),
/// [`AnimeDb::manga_metadata`](AnimeDb::manga_metadata), or
/// [`AnimeDb::movie_metadata`](AnimeDb::movie_metadata) to scope searches
/// and lookups to a specific media kind without repeating filter options.
pub struct MetadataCollection<'a> {
    db: &'a AnimeDb,
    options: SearchOptions,
}

impl<'a> MetadataCollection<'a> {
    fn new(db: &'a AnimeDb, options: SearchOptions) -> Self {
        Self { db, options }
    }

    /// Full-text search within this media kind slice.
    pub fn search(&self, query: &str) -> Result<Vec<SearchHit>> {
        self.db.search(query, self.options.clone())
    }

    /// Fetch a stored media record by local ID. Returns [`Error::NotFound`] if the ID
    /// exists but belongs to a different media kind.
    pub fn get(&self, media_id: i64) -> Result<StoredMedia> {
        let media = self.db.get_media(media_id)?;
        if self.matches_media(&media) {
            Ok(media)
        } else {
            Err(Error::NotFound)
        }
    }

    /// Fetch a stored media record by provider + source ID, scoped to this kind.
    pub fn by_external_id(&self, source: SourceName, source_id: &str) -> Result<StoredMedia> {
        let media = if let Some(kind) = self.options.media_kind {
            self.db
                .media()
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

#[cfg(test)]
mod tests {
    use super::*;

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

    fn now_string() -> String {
        let unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        unix.to_string()
    }

    use crate::model::{
        CanonicalEpisode, CanonicalMedia, ExternalId, FieldProvenance, MediaKind, SearchOptions,
        SourceName, SourcePayload,
    };
    use crate::provider::{
        AniListProvider, ImdbProvider, JikanProvider, KitsuProvider, Provider, TvmazeProvider,
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
                "Dr. Kenzo Tenma saves a child who grows into a serial killer, forcing him into a \
                 long pursuit across Europe while confronting guilt, identity and systemic \
                 corruption."
                    .into(),
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
            .media()
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
            .search_repo()
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
            .search_repo()
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
            .search_repo()
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
            synopsis: Some(
                "A high school chemistry teacher diagnosed with lung cancer turns to \
                 manufacturing methamphetamine."
                    .into(),
            ),
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
            .media()
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
            .media()
            .get_by_external_id_and_kind(SourceName::Imdb, MediaKind::Show, "tt11111")
            .expect("lookup show by kind");
        assert_eq!(show_loaded.media_kind, MediaKind::Show);

        let movie_loaded = db
            .media()
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
            .search_repo()
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
            .search_repo()
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
            .search_repo()
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
            .episodes()
            .upsert_episode(&episode, media_id)
            .expect("upsert episode");
        assert!(ep_id > 0);

        let episodes = db
            .episodes()
            .episodes_for_media(media_id)
            .expect("list episodes");
        assert_eq!(episodes.len(), 1);
        assert_eq!(episodes[0].episode_number, Some(1));
        assert_eq!(episodes[0].title_display.as_deref(), Some("The Hospital"));

        // Verify source records are stored
        let source_records = db
            .episodes()
            .episode_source_records_for_media(media_id)
            .expect("source records");
        assert_eq!(source_records.len(), 1);
        assert_eq!(source_records[0].source_id, "ep1");
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

        db.episodes()
            .upsert_episode(&episode1, media_id)
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

        db.episodes()
            .upsert_episode(&episode2, media_id)
            .expect("upsert second");

        let episodes = db
            .episodes()
            .episodes_for_media(media_id)
            .expect("list episodes");
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

        db.episodes()
            .upsert_episode(&ep1, media_id)
            .expect("upsert ep1");
        db.episodes()
            .upsert_episode(&ep2, media_id)
            .expect("upsert ep2");

        let found = db
            .episodes()
            .episode_by_absolute_number(media_id, 2)
            .expect("find by absolute");
        assert_eq!(found.unwrap().episode_number, Some(2));

        let not_found = db.episode_by_absolute_number(media_id, 99);
        assert!(not_found.unwrap().is_none());
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
            .episodes()
            .episode_by_season_episode(media_id, 1, 5)
            .expect("find by season/episode");
        let found = found.unwrap();
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

        db.episodes()
            .upsert_episode(&ep1, media_id)
            .expect("upsert ep");

        let doc = db
            .search_repo()
            .media_document_by_id(media_id)
            .expect("get doc");
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
            .search_repo()
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
