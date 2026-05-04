use crate::db::AnimeDb;
use crate::error::{Error, Result};
use crate::model::*;
use crate::provider::*;
use std::path::Path;
use std::thread;

pub struct SyncService<'a> {
    pub db: &'a mut AnimeDb,
}

impl<'a> SyncService<'a> {
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

    pub fn sync_database(path: impl AsRef<Path>) -> Result<SyncReport> {
        let mut db = AnimeDb::open(path)?;
        db.sync_default_sources()
    }

    pub fn sync_anilist(&mut self, media_kind: MediaKind) -> Result<SyncOutcome> {
        self.sync_from(
            &AniListProvider::default(),
            SyncRequest::new(SourceName::AniList).with_media_kind(media_kind),
        )
    }

    pub fn sync_jikan(&mut self, media_kind: MediaKind) -> Result<SyncOutcome> {
        self.sync_from(
            &JikanProvider::default(),
            SyncRequest::new(SourceName::Jikan).with_media_kind(media_kind),
        )
    }

    pub fn sync_kitsu(&mut self, media_kind: MediaKind) -> Result<SyncOutcome> {
        self.sync_from(
            &KitsuProvider::default(),
            SyncRequest::new(SourceName::Kitsu).with_media_kind(media_kind),
        )
    }

    pub fn sync_tvmaze(&mut self) -> Result<SyncOutcome> {
        self.sync_from(
            &TvmazeProvider::default(),
            SyncRequest::new(SourceName::Tvmaze).with_media_kind(MediaKind::Show),
        )
    }

    pub fn sync_imdb(&mut self, media_kind: MediaKind) -> Result<SyncOutcome> {
        self.sync_from(
            &ImdbProvider::default(),
            SyncRequest::new(SourceName::Imdb).with_media_kind(media_kind),
        )
    }
}

fn now_string() -> String {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    unix.to_string()
}
