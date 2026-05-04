// Test file to reproduce the InvalidColumnType error
use rusqlite::{Connection, params, Result};

fn main() -> Result<()> {
    let conn = Connection::open_in_memory()?;

    // Run the exact v6 migration
    conn.execute_batch(r#"
        CREATE TABLE episode (
            id INTEGER PRIMARY KEY,
            media_id INTEGER NOT NULL,
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
            media_id INTEGER NOT NULL,
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

        CREATE TABLE media (
            id INTEGER PRIMARY KEY,
            media_kind TEXT NOT NULL,
            title_display TEXT NOT NULL
        );

        INSERT INTO media VALUES (1, 'anime', 'Test');
    "#)?;

    // Now insert the same way the code does
    let titles_json: Option<String> = None;
    let raw_json: Option<String> = None;

    conn.execute(
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
            "kitsu",
            "ep1",
            1i64,
            "anime",
            Some(1i32),
            Some(1i32),
            Some(1i32),
            Some("The Hospital"),
            Some("Byouin"),
            &titles_json,
            Some("Tenma operates on a young boy."),
            Some("2005-04-05"),
            Some(23i32),
            Option::<String>::None,
            &raw_json,
        ],
    )?;

    println!("Insert successful!");

    // Now try to read it back using the EXACT same query as merge_episodes_for_media
    let mut stmt = conn.prepare(
        r#"
        SELECT
            id, episode_id, source, source_id, media_id, media_kind,
            season_number, episode_number, absolute_number,
            title_display, title_original, titles_json,
            synopsis, air_date, runtime_minutes, thumbnail_url,
            raw_json, fetched_at
        FROM episode_source_record
        WHERE media_id = ?1
        "#
    )?;

    let rows = stmt.query_map(params![1i64], |row| {
        let source = row.get_ref(2)?.as_str()?;
        let media_kind = row.get_ref(5)?.as_str()?;
        println!("source at col 2: {}", source);
        println!("media_kind at col 5: {}", media_kind);

        let titles_json: Option<String> = row.get(10)?;
        println!("titles_json at col 10: {:?}", titles_json);

        let raw_json: Option<String> = row.get(16)?;
        println!("raw_json at col 16: {:?}", raw_json);

        // Try to get air_date (col 13) as Option<String>
        let air_date: Option<String> = row.get(13)?;
        println!("air_date at col 13: {:?}", air_date);

        // Try to get runtime_minutes (col 14) as Option<i32>
        let runtime_minutes: Option<i32> = row.get(14)?;
        println!("runtime_minutes at col 14: {:?}", runtime_minutes);

        Ok(())
    })?;

    for row in rows {
        row?;
    }

    Ok(())
}