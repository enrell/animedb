//! Orchestrates paginated provider-to-catalog sync runs.
//!
//! ## Sync flow
//!
//! 1. Load persisted [`PersistedSyncState`] for the provider + scope (if any)
//! 2. Fetch one page from the provider via [`Provider::fetch_page`]
//! 3. Upsert each media item into the catalog (merge engine runs automatically)
//! 4. Persist the updated cursor to `sync_state`
//! 5. Repeat until the provider returns an empty page or `max_pages` is reached
//!
//! ## Provider rate limiting
//!
//! Each provider declares a [`Provider::min_interval`]; the service enforces it
//! by calling `thread::sleep` between page fetches. The sync loop is therefore
//! cooperative and will not hammer a provider's API.

use crate::db::AnimeDb;
use crate::error::{Error, Result};
use crate::model::*;
use crate::provider::*;
use std::path::Path;
use std::thread;

/// Orchestrates sync operations against one open [`AnimeDb`] instance.
///
/// Obtain via [`AnimeDb::sync_service`](crate::db::AnimeDb::sync_service).
pub struct SyncService<'a> {
    /// The database instance to sync into.
    pub db: &'a mut AnimeDb,
}

impl<'a> SyncService<'a> {
    /// Syncs from a single provider using a custom [`SyncRequest`].
    ///
    /// Validates that `provider.source()` matches `request.source`, then
    /// iterates over pages until the provider is exhausted or `max_pages` is reached.
    ///
    /// Cursor state is persisted after every page, so interrupted syncs can be
    /// resumed by calling this again with the same provider + request.
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
                self.db
                    .sync_state()
                    .load_sync_state(request.source, &scope)
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
                self.db.sync_state().save_sync_state(PersistedSyncState {
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
                self.db.upsert_media(item)?;
                upserted_records += 1;
            }

            fetched_pages += 1;
            last_cursor = Some(cursor.clone());

            self.db.sync_state().save_sync_state(PersistedSyncState {
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

    /// Syncs all default providers: AniList, Jikan, Kitsu (anime + manga),
    /// TVmaze (shows), and IMDb (shows + movies).
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

    /// Convenience — opens a database at `path`, runs [`SyncService::sync_default_sources`],
    /// and returns the report.
    pub fn sync_database(path: impl AsRef<Path>) -> Result<SyncReport> {
        let mut db = AnimeDb::open(path)?;
        db.sync_default_sources()
    }

    /// Syncs AniList for one media kind.
    pub fn sync_anilist(&mut self, media_kind: MediaKind) -> Result<SyncOutcome> {
        self.sync_from(
            &AniListProvider::default(),
            SyncRequest::new(SourceName::AniList).with_media_kind(media_kind),
        )
    }

    /// Syncs Jikan for one media kind.
    pub fn sync_jikan(&mut self, media_kind: MediaKind) -> Result<SyncOutcome> {
        self.sync_from(
            &JikanProvider::default(),
            SyncRequest::new(SourceName::Jikan).with_media_kind(media_kind),
        )
    }

    /// Syncs Kitsu for one media kind.
    pub fn sync_kitsu(&mut self, media_kind: MediaKind) -> Result<SyncOutcome> {
        self.sync_from(
            &KitsuProvider::default(),
            SyncRequest::new(SourceName::Kitsu).with_media_kind(media_kind),
        )
    }

    /// Syncs TVmaze (shows only).
    pub fn sync_tvmaze(&mut self) -> Result<SyncOutcome> {
        self.sync_from(
            &TvmazeProvider::default(),
            SyncRequest::new(SourceName::Tvmaze).with_media_kind(MediaKind::Show),
        )
    }

    /// Syncs IMDb for one media kind.
    pub fn sync_imdb(&mut self, media_kind: MediaKind) -> Result<SyncOutcome> {
        self.sync_from(
            &ImdbProvider::default(),
            SyncRequest::new(SourceName::Imdb).with_media_kind(media_kind),
        )
    }

    /// Syncs episodes for all media items currently in the catalog.
    ///
    /// For IMDb, downloads the full `.tsv.gz` episode dumps and maps them efficiently.
    /// For TVMaze, Jikan, and Kitsu, queries the REST API per-media item.
    pub fn sync_all_episodes(&mut self) -> Result<usize> {
        let mut total_upserted = 0;

        let mut imdb_parents = std::collections::HashMap::new();
        let mut api_targets = Vec::new();

        {
            let mut stmt = self
                .db
                .connection()
                .prepare("SELECT media_id, media_kind, source, source_id FROM media_external_id")?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;

            for row in rows {
                let (media_id, kind_str, source_str, source_id) = row?;
                if let (Ok(source), Ok(kind)) = (
                    source_str.parse::<SourceName>(),
                    kind_str.parse::<MediaKind>(),
                ) {
                    if source == SourceName::Imdb {
                        imdb_parents.insert(source_id, media_id);
                    } else if matches!(
                        source,
                        SourceName::Tvmaze | SourceName::Jikan | SourceName::Kitsu
                    ) {
                        api_targets.push((source, source_id, media_id, kind));
                    }
                }
            }
        }

        let registry = crate::provider::registry::default_registry();
        for (source, source_id, media_id, kind) in api_targets {
            if let Ok(provider) = registry.get(source) {
                if let Ok(episodes) = provider.fetch_episodes(kind, &source_id) {
                    for ep in episodes {
                        if self
                            .db
                            .episodes()
                            .upsert_episode_source_record_no_merge(&ep, media_id)
                            .is_ok()
                        {
                            total_upserted += 1;
                        }
                    }
                    let _ = self.db.episodes().merge_episodes_for_media(media_id);
                }

                let sleep_for = provider.min_interval();
                if !sleep_for.is_zero() {
                    thread::sleep(sleep_for);
                }
            }
        }

        if !imdb_parents.is_empty() {
            let imdb = ImdbProvider::default();
            let valid_parents: std::collections::HashSet<String> =
                imdb_parents.keys().cloned().collect();
            let mut media_ids_to_merge = std::collections::HashSet::new();

            imdb.fetch_all_episodes(&valid_parents, |ep| {
                if let Some(parent_id) = ep
                    .raw_json
                    .as_ref()
                    .and_then(|j| j.get("parentTconst"))
                    .and_then(|v| v.as_str())
                {
                    if let Some(&media_id) = imdb_parents.get(parent_id) {
                        if self
                            .db
                            .episodes()
                            .upsert_episode_source_record_no_merge(&ep, media_id)
                            .is_ok()
                        {
                            total_upserted += 1;
                        }
                        media_ids_to_merge.insert(media_id);
                    }
                }
            })?;

            for media_id in media_ids_to_merge {
                let _ = self.db.episodes().merge_episodes_for_media(media_id);
            }
        }

        Ok(total_upserted)
    }
}

fn now_string() -> String {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    unix.to_string()
}
