use crate::error::Result;
use rusqlite::Connection;

pub fn configure(conn: &Connection) -> Result<()> {
    conn.execute_batch(
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

pub fn migrate(conn: &Connection) -> Result<()> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

    if version >= 6 {
        return Ok(());
    }

    if version == 0 {
        conn.execute_batch(
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
        conn.execute_batch(
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
        conn.execute_batch(
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
        conn.execute_batch(
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
        conn.execute_batch(
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

    if version < 6 {
        conn.execute_batch(
            r#"
            BEGIN;

            -- The v5 episode table used source-specific rows directly.
            -- The new design has canonical episode + episode_source_record.
            -- We must recreate episode with the canonical schema.
            DROP TABLE IF EXISTS episode;
            DROP TABLE IF EXISTS episode_source_record;

            CREATE TABLE episode (
                id INTEGER PRIMARY KEY,
                media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
                season_number INTEGER,
                episode_number INTEGER,
                absolute_number INTEGER,
                title_display TEXT,
                title_original TEXT,
                titles_json TEXT CHECK(titles_json IS NULL OR json_valid(titles_json)),
                synopsis TEXT,
                air_date TEXT,
                runtime_minutes INTEGER,
                thumbnail_url TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE episode_source_record (
                id INTEGER PRIMARY KEY,
                episode_id INTEGER REFERENCES episode(id) ON DELETE CASCADE,
                source TEXT NOT NULL,
                source_id TEXT NOT NULL,
                media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
                media_kind TEXT NOT NULL CHECK(media_kind IN ('anime', 'manga', 'show', 'movie')),
                season_number INTEGER,
                episode_number INTEGER,
                absolute_number INTEGER,
                title_display TEXT,
                title_original TEXT,
                titles_json TEXT CHECK(titles_json IS NULL OR json_valid(titles_json)),
                synopsis TEXT,
                air_date TEXT,
                runtime_minutes INTEGER,
                thumbnail_url TEXT,
                raw_json TEXT CHECK(raw_json IS NULL OR json_valid(raw_json)),
                fetched_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(source, source_id, media_id)
            );

            CREATE INDEX episode_source_record_episode_id_idx ON episode_source_record(episode_id);
            CREATE INDEX episode_source_record_media_id_idx ON episode_source_record(media_id);
            CREATE INDEX episode_source_record_source_idx ON episode_source_record(source);
            CREATE INDEX episode_source_record_abs_num_idx ON episode_source_record(media_id, absolute_number);
            CREATE INDEX episode_source_record_season_ep_idx ON episode_source_record(media_id, season_number, episode_number);
            CREATE INDEX episode_media_id_idx ON episode(media_id);
            CREATE INDEX episode_absolute_number_idx ON episode(media_id, absolute_number);
            CREATE INDEX episode_season_episode_idx ON episode(media_id, season_number, episode_number);

            PRAGMA user_version = 6;
            COMMIT;
            "#,
        )?;
    }

    Ok(())
}
