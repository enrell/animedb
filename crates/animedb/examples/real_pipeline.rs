use animedb::{
    AniListProvider, AnimeDb, JikanProvider, KitsuProvider, MediaKind, RemoteApi, RemoteSource,
    Result, SearchOptions, SourceName, SyncRequest,
};
use rusqlite::params;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<()> {
    let database_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join("animedb-real-pipeline.sqlite"));
    let max_pages = env::var("ANIMEDB_REAL_MAX_PAGES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1);
    let page_size = env::var("ANIMEDB_REAL_PAGE_SIZE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(5);

    if database_path.exists() {
        fs::remove_file(&database_path)
            .map_err(|err| animedb::Error::Validation(format!("failed to reset db: {err}")))?;
    }

    let mut db = AnimeDb::open(&database_path)?;
    let anilist = AniListProvider::default();
    let jikan = JikanProvider::default();
    let kitsu = KitsuProvider::default();

    for media_kind in [MediaKind::Anime, MediaKind::Manga] {
        db.sync_from(
            &anilist,
            SyncRequest::new(SourceName::AniList)
                .with_media_kind(media_kind)
                .with_page_size(page_size)
                .with_max_pages(max_pages),
        )?;
        db.sync_from(
            &jikan,
            SyncRequest::new(SourceName::Jikan)
                .with_media_kind(media_kind)
                .with_page_size(page_size)
                .with_max_pages(max_pages),
        )?;
        db.sync_from(
            &kitsu,
            SyncRequest::new(SourceName::Kitsu)
                .with_media_kind(media_kind)
                .with_page_size(page_size)
                .with_max_pages(max_pages),
        )?;
    }

    let (media_id, title_display): (i64, String) = db.connection().query_row(
        "SELECT id, title_display FROM media ORDER BY id LIMIT 1",
        params![],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let loaded = db.get_media(media_id)?;
    let search_term = first_search_token(&title_display);
    let local_hits = db.anime_metadata().search(&search_term)?;

    if local_hits.is_empty() {
        return Err(animedb::Error::Validation(format!(
            "local search returned no hits for token {search_term}"
        )));
    }

    let remote_anilist = RemoteApi::anilist();
    let remote_jikan = AnimeDb::remote(RemoteSource::Jikan);
    let remote_kitsu = AnimeDb::remote(RemoteSource::Kitsu);
    let anilist_results =
        remote_anilist.search("monster", SearchOptions::default().with_media_kind(MediaKind::Anime))?;
    let jikan_results =
        remote_jikan.search("monster", SearchOptions::default().with_media_kind(MediaKind::Anime))?;
    let kitsu_results =
        remote_kitsu.search("monster", SearchOptions::default().with_media_kind(MediaKind::Anime))?;

    if anilist_results.is_empty() || jikan_results.is_empty() || kitsu_results.is_empty() {
        return Err(animedb::Error::Validation(
            "remote search returned no results for one of the providers".into(),
        ));
    }

    let remote_movie_hits = remote_jikan.movie_metadata().search("paprika")?;
    if remote_movie_hits.is_empty() {
        return Err(animedb::Error::Validation(
            "remote movie search returned no results".into(),
        ));
    }

    println!("database_path={}", database_path.display());
    println!("seed_media_id={media_id}");
    println!("seed_title={}", loaded.name());
    println!("local_search_hits={}", local_hits.len());
    println!("remote_anilist_hits={}", anilist_results.len());
    println!("remote_jikan_hits={}", jikan_results.len());
    println!("remote_kitsu_hits={}", kitsu_results.len());
    println!("remote_movie_hits={}", remote_movie_hits.len());

    Ok(())
}

fn first_search_token(title: &str) -> String {
    title
        .split_whitespace()
        .find(|token| token.chars().count() >= 3)
        .unwrap_or(title)
        .to_string()
}
