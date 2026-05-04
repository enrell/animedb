//! Episode record persistence — `episode` (canonical) and `episode_source_record` (per-provider raw).
//!
//! Episode storage follows a two-tier design:
//!
//! 1. [`EpisodeSourceRecord`] — raw per-provider episode data stored verbatim for audit
//! 2. [`StoredEpisode`] — canonical merged episode (one per media+numbering slot)
//!
//! The merge runs automatically after every source record upsert via
//! [`merge_episodes_for_media`](EpisodeRepository::merge_episodes_for_media).

use super::common::*;
use crate::error::{Error, Result};
use crate::merge::merge_episode_source_records;
use crate::model::*;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use std::collections::HashMap;

/// Repository for episode record upsert and lookup.
pub struct EpisodeRepository<'a> {
    pub conn: &'a Connection,
}

impl<'a> EpisodeRepository<'a> {
    /// Inserts or updates a raw source episode record.
    ///
    /// After inserting, automatically runs [`EpisodeRepository::merge_episodes_for_media`] to
    /// update the canonical `episode` table from all accumulated source records.
    /// Returns the `episode_source_record.id` of the inserted/updated row.
    pub fn upsert_episode_source_record(
        &mut self,
        episode: &CanonicalEpisode,
        media_id: i64,
    ) -> Result<i64> {
        let titles_json = episode
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
        INSERT INTO episode_source_record (
            source, source_id, media_id, media_kind,
            season_number, episode_number, absolute_number,
            title_display, title_original, titles_json,
            synopsis, air_date, runtime_minutes, thumbnail_url,
            raw_json, fetched_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, CURRENT_TIMESTAMP)
        ON CONFLICT(source, source_id, media_id) DO UPDATE SET
            season_number = excluded.season_number,
            episode_number = excluded.episode_number,
            absolute_number = excluded.absolute_number,
            title_display = excluded.title_display,
            title_original = excluded.title_original,
            titles_json = excluded.titles_json,
            synopsis = excluded.synopsis,
            air_date = excluded.air_date,
            runtime_minutes = excluded.runtime_minutes,
            thumbnail_url = excluded.thumbnail_url,
            raw_json = excluded.raw_json,
            fetched_at = CURRENT_TIMESTAMP
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
            titles_json,
            episode.synopsis,
            episode.air_date,
            episode.runtime_minutes,
            episode.thumbnail_url,
            raw_json,
        ],
    )?;

        let source_record_id = self.conn.last_insert_rowid();

        // Merge source records into canonical episodes
        self.merge_episodes_for_media(media_id)?;

        Ok(source_record_id)
    }

    /// Alias for [`upsert_episode_source_record`](EpisodeRepository::upsert_episode_source_record).
    pub fn upsert_episode(&mut self, episode: &CanonicalEpisode, media_id: i64) -> Result<i64> {
        self.upsert_episode_source_record(episode, media_id)
    }

    /// Loads all source records for a media item, merges them, and upserts canonical episodes.
    ///
    /// Grouping key is `(media_id, absolute_number)` or fallback
    /// `(media_id, season_number, episode_number)`. Provider priority for field
    /// selection is AniList > IMDb > TVmaze > MyAnimeList > Jikan > Kitsu.
    pub fn merge_episodes_for_media(&mut self, media_id: i64) -> Result<()> {
        // Load all source records for this media
        let records = {
            let mut stmt = self.conn.prepare(
                r#"
            SELECT
                id, episode_id, source, source_id, media_id, media_kind,
                season_number, episode_number, absolute_number,
                title_display, title_original, titles_json,
                synopsis, air_date, runtime_minutes, thumbnail_url,
                raw_json, fetched_at
            FROM episode_source_record
            WHERE media_id = ?1
            "#,
            )?;
            stmt.query_map(params![media_id], |row| {
                let source = parse_source(row.get_ref(2)?.as_str()?)
                    .map_err(|e| rusqlite_decode_error(2, e))?;
                let media_kind = parse_media_kind(row.get_ref(5)?.as_str()?)
                    .map_err(|e| rusqlite_decode_error(5, e))?;
                let titles_json = row
                    .get::<_, Option<String>>(11)?
                    .map(|value| serde_json::from_str::<Value>(&value).ok())
                    .flatten();
                let raw_json = row
                    .get::<_, Option<String>>(16)?
                    .map(|value| serde_json::from_str::<Value>(&value).ok())
                    .flatten();
                Ok(EpisodeSourceRecord {
                    id: row.get(0)?,
                    episode_id: row.get(1)?,
                    source,
                    source_id: row.get(3)?,
                    media_id: row.get(4)?,
                    media_kind,
                    season_number: row.get(6)?,
                    episode_number: row.get(7)?,
                    absolute_number: row.get(8)?,
                    title_display: row.get(9)?,
                    title_original: row.get(10)?,
                    titles_json,
                    synopsis: row.get(12)?,
                    air_date: row.get(13)?,
                    runtime_minutes: row.get(14)?,
                    thumbnail_url: row.get(15)?,
                    raw_json,
                    fetched_at: row.get(17)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)?
        };

        if records.is_empty() {
            return Ok(());
        }

        // Group records by episode identity
        let mut groups: HashMap<String, Vec<EpisodeSourceRecord>> = HashMap::new();
        for record in records {
            let key = if let Some(abs) = record.absolute_number {
                format!("{}:abs:{}", record.media_id, abs)
            } else {
                format!(
                    "{}:se:{}:{}",
                    record.media_id,
                    record.season_number.unwrap_or(0),
                    record.episode_number.unwrap_or(0)
                )
            };
            groups.entry(key).or_insert_with(Vec::new).push(record);
        }

        // For each group, merge and upsert canonical
        for (_, group) in groups {
            let canonical = merge_episode_source_records(&group);
            let episode_id = self.upsert_canonical_episode(&canonical, group[0].media_id)?;

            // Update episode_id back-references in source records
            for record in &group {
                self.conn.execute(
                    "UPDATE episode_source_record SET episode_id = ?1 WHERE id = ?2",
                    params![episode_id, record.id],
                )?;
            }
        }

        Ok(())
    }

    fn upsert_canonical_episode(&mut self, episode: &StoredEpisode, media_id: i64) -> Result<i64> {
        let titles_json = episode
            .titles_json
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        // Check if episode exists for this media with same numbering
        let existing_id: Option<i64> = self
            .conn
            .query_row(
                r#"
            SELECT id FROM episode
            WHERE media_id = ?1
              AND COALESCE(season_number, 0) = COALESCE(?2, 0)
              AND COALESCE(episode_number, 0) = COALESCE(?3, 0)
              AND COALESCE(absolute_number, 0) = COALESCE(?4, 0)
            LIMIT 1
            "#,
                params![
                    media_id,
                    episode.season_number,
                    episode.episode_number,
                    episode.absolute_number
                ],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = existing_id {
            self.conn.execute(
                r#"
            UPDATE episode SET
                season_number = ?2,
                episode_number = ?3,
                absolute_number = ?4,
                title_display = ?5,
                title_original = ?6,
                titles_json = ?7,
                synopsis = ?8,
                air_date = ?9,
                runtime_minutes = ?10,
                thumbnail_url = ?11,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            "#,
                params![
                    id,
                    episode.season_number,
                    episode.episode_number,
                    episode.absolute_number,
                    episode.title_display,
                    episode.title_original,
                    titles_json,
                    episode.synopsis,
                    episode.air_date,
                    episode.runtime_minutes,
                    episode.thumbnail_url,
                ],
            )?;
            Ok(id)
        } else {
            self.conn.execute(
                r#"
            INSERT INTO episode (
                media_id, season_number, episode_number, absolute_number,
                title_display, title_original, titles_json,
                synopsis, air_date, runtime_minutes, thumbnail_url,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, CURRENT_TIMESTAMP)
            "#,
                params![
                    media_id,
                    episode.season_number,
                    episode.episode_number,
                    episode.absolute_number,
                    episode.title_display,
                    episode.title_original,
                    titles_json,
                    episode.synopsis,
                    episode.air_date,
                    episode.runtime_minutes,
                    episode.thumbnail_url,
                ],
            )?;
            Ok(self.conn.last_insert_rowid())
        }
    }

    /// Returns all canonical episodes for a media item, ordered by season + episode number.
    pub fn episodes_for_media(&self, media_id: i64) -> Result<Vec<StoredEpisode>> {
        let mut stmt = self.conn.prepare(
            r#"
        SELECT
            id, media_id, season_number, episode_number, absolute_number,
            title_display, title_original, titles_json,
            synopsis, air_date, runtime_minutes, thumbnail_url
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

    pub fn episode_source_records_for_media(
        &self,
        media_id: i64,
    ) -> Result<Vec<EpisodeSourceRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
        SELECT
            id, episode_id, source, source_id, media_id, media_kind,
            season_number, episode_number, absolute_number,
            title_display, title_original, titles_json,
            synopsis, air_date, runtime_minutes, thumbnail_url,
            raw_json, fetched_at
        FROM episode_source_record
        WHERE media_id = ?1
        ORDER BY source, absolute_number
        "#,
        )?;
        let rows = stmt.query_map(params![media_id], |row| {
            let source =
                parse_source(row.get_ref(2)?.as_str()?).map_err(|e| rusqlite_decode_error(2, e))?;
            let media_kind = parse_media_kind(row.get_ref(5)?.as_str()?)
                .map_err(|e| rusqlite_decode_error(5, e))?;
            let titles_json = row
                .get::<_, Option<String>>(11)?
                .map(|value| serde_json::from_str::<Value>(&value).ok())
                .flatten();
            let raw_json = row
                .get::<_, Option<String>>(16)?
                .map(|value| serde_json::from_str::<Value>(&value).ok())
                .flatten();
            Ok(EpisodeSourceRecord {
                id: row.get(0)?,
                episode_id: row.get(1)?,
                source,
                source_id: row.get(3)?,
                media_id: row.get(4)?,
                media_kind,
                season_number: row.get(6)?,
                episode_number: row.get(7)?,
                absolute_number: row.get(8)?,
                title_display: row.get(9)?,
                title_original: row.get(10)?,
                titles_json,
                synopsis: row.get(12)?,
                air_date: row.get(13)?,
                runtime_minutes: row.get(14)?,
                thumbnail_url: row.get(15)?,
                raw_json,
                fetched_at: row.get(17)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    pub fn episode_by_absolute_number(
        &self,
        media_id: i64,
        absolute_number: i32,
    ) -> Result<Option<StoredEpisode>> {
        self.conn
            .query_row(
                r#"
            SELECT
                id, media_id, season_number, episode_number, absolute_number,
                title_display, title_original, titles_json,
                synopsis, air_date, runtime_minutes, thumbnail_url
            FROM episode
            WHERE media_id = ?1 AND absolute_number = ?2
            "#,
                params![media_id, absolute_number],
                parse_stored_episode,
            )
            .optional()
            .map_err(Error::from)
    }

    pub fn episode_by_season_episode(
        &self,
        media_id: i64,
        season_number: i32,
        episode_number: i32,
    ) -> Result<Option<StoredEpisode>> {
        self.conn
            .query_row(
                r#"
            SELECT
                id, media_id, season_number, episode_number, absolute_number,
                title_display, title_original, titles_json,
                synopsis, air_date, runtime_minutes, thumbnail_url
            FROM episode
            WHERE media_id = ?1 AND season_number = ?2 AND episode_number = ?3
            "#,
                params![media_id, season_number, episode_number],
                parse_stored_episode,
            )
            .optional()
            .map_err(Error::from)
    }
}
