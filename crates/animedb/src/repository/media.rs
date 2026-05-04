//! Media record persistence — `media`, `media_alias`, `media_external_id`, `source_record`, `field_provenance`.

use super::common::*;
use crate::error::{Error, Result};
use crate::merge::merge_media;
use crate::model::*;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::Value;

/// Repository for media record persistence and lookup.
///
/// All methods are thin wrappers over SQLite queries. The `upsert_media_in_tx`
/// method is the core write path — it calls [`merge_media`] internally to
/// resolve field-level conflicts before writing.
pub struct MediaRepository<'a> {
    pub conn: &'a Connection,
}

impl<'a> MediaRepository<'a> {
    /// Inserts a new media record or merges it with an existing record sharing the same external ID.
    ///
    /// Returns the local `media.id` of the inserted or updated record.
    ///
    /// # Merge behavior
    ///
    /// If a record with a matching external ID + media kind already exists, the
    /// merge engine scores each incoming field against stored provenance and
    /// keeps the higher-scoring value per field. All tables (`media`, `media_alias`,
    /// `media_external_id`, `source_record`, `field_provenance`, `media_fts`) are
    /// updated atomically inside a transaction.
    pub fn upsert_media(conn: &mut Connection, media: &CanonicalMedia) -> Result<i64> {
        media.validate()?;
        let tx = conn.transaction()?;
        let media_id = Self::upsert_media_in_tx(&tx, media)?;
        tx.commit()?;
        Ok(media_id)
    }

    /// Fetches a stored media record by local primary key.
    /// Returns [`Error::NotFound`] if the ID does not exist.
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

    fn upsert_media_in_tx(tx: &Transaction<'_>, media: &CanonicalMedia) -> Result<i64> {
        let existing_media_id = Self::resolve_media_id(tx, media.media_kind, &media.external_ids)?;
        Self::ensure_no_conflicts(tx, media.media_kind, existing_media_id, &media.external_ids)?;
        let existing = existing_media_id
            .map(|media_id| Self::load_stored_media_in_tx(tx, media_id))
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
}
