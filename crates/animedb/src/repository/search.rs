use super::common::*;
use crate::error::{Error, Result};
use crate::model::*;
use rusqlite::{Connection, OptionalExtension, params};

pub struct SearchRepository<'a> {
    pub conn: &'a Connection,
}

impl<'a> SearchRepository<'a> {
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

    pub fn media_document_by_id(&self, media_id: i64) -> Result<MediaDocument> {
        let media = crate::repository::MediaRepository { conn: self.conn }.get_media(media_id)?;
        let episodes = crate::repository::EpisodeRepository { conn: self.conn }
            .episodes_for_media(media_id)?;
        Ok(MediaDocument { media, episodes })
    }

    pub fn media_document_by_external_id(
        &self,
        source: SourceName,
        source_id: &str,
    ) -> Result<MediaDocument> {
        let media = crate::repository::MediaRepository { conn: self.conn }
            .get_by_external_id(source, source_id)?;
        let episodes = crate::repository::EpisodeRepository { conn: self.conn }
            .episodes_for_media(media.id)?;
        Ok(MediaDocument { media, episodes })
    }

    pub fn media_document_by_external_id_and_kind(
        &self,
        source: SourceName,
        media_kind: MediaKind,
        source_id: &str,
    ) -> Result<MediaDocument> {
        let media = crate::repository::MediaRepository { conn: self.conn }
            .get_by_external_id_and_kind(source, media_kind, source_id)?;
        let episodes = crate::repository::EpisodeRepository { conn: self.conn }
            .episodes_for_media(media.id)?;
        Ok(MediaDocument { media, episodes })
    }
}
