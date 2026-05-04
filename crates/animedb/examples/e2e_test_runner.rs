use animedb::{
    AnimeDb, ImdbProvider, MediaKind, RemoteApi, SearchOptions, SourceName, SyncRequest,
    TvmazeProvider,
};
use std::env;
use std::path::PathBuf;

fn main() -> animedb::Result<()> {
    let args: Vec<String> = env::args().collect();
    let db_path = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join("animedb-e2e-test.sqlite"));

    let mode = args.get(2).map(|s| s.as_str()).unwrap_or("--run-all");

    match mode {
        "--create-only" => {
            println!("Creating database at {}", db_path.display());
            let _db = AnimeDb::open(&db_path)?;
            println!("Database created successfully");
        }
        "--sync-tvmaze" => {
            let pages = args
                .iter()
                .position(|a| a == "--pages")
                .and_then(|i| args.get(i + 1)?.parse::<usize>().ok())
                .unwrap_or(2);
            let page_size = args
                .iter()
                .position(|a| a == "--page-size")
                .and_then(|i| args.get(i + 1)?.parse::<usize>().ok())
                .unwrap_or(10);

            println!("Syncing TVmaze: {} pages, {} per page", pages, page_size);

            let mut db = AnimeDb::open(&db_path)?;
            let provider = TvmazeProvider::default();

            let outcome = db.sync_from(
                &provider,
                SyncRequest::new(SourceName::Tvmaze)
                    .with_media_kind(MediaKind::Show)
                    .with_page_size(page_size)
                    .with_max_pages(pages),
            )?;

            println!(
                "TVmaze sync complete: {} records, {} pages",
                outcome.upserted_records, outcome.fetched_pages
            );
        }
        "--sync-imdb" => {
            let media_kind_str = args
                .iter()
                .position(|a| a == "--media-kind")
                .and_then(|i| args.get(i + 1)?.parse::<MediaKind>().ok())
                .unwrap_or(MediaKind::Movie);
            let pages = args
                .iter()
                .position(|a| a == "--pages")
                .and_then(|i| args.get(i + 1)?.parse::<usize>().ok())
                .unwrap_or(1);
            let page_size = args
                .iter()
                .position(|a| a == "--page-size")
                .and_then(|i| args.get(i + 1)?.parse::<usize>().ok())
                .unwrap_or(50);

            println!(
                "Syncing IMDb: {} pages, {} per page, kind: {:?}",
                pages, page_size, media_kind_str
            );

            let mut db = AnimeDb::open(&db_path)?;
            let provider = ImdbProvider::default();

            let outcome = db.sync_from(
                &provider,
                SyncRequest::new(SourceName::Imdb)
                    .with_media_kind(media_kind_str)
                    .with_page_size(page_size)
                    .with_max_pages(pages),
            )?;

            println!(
                "IMDb sync complete: {} records, {} pages",
                outcome.upserted_records, outcome.fetched_pages
            );
        }
        "--remote-search" => {
            let default_query = "breaking bad".to_string();
            let query = args.get(3).unwrap_or(&default_query);
            println!("Remote searching TVmaze for: {}", query);

            let api = RemoteApi::tvmaze();
            let results = api.search(
                query,
                SearchOptions::default().with_media_kind(MediaKind::Show),
            )?;

            println!("Found {} results:", results.len());
            for media in results.iter().take(5) {
                println!("  - {} (kind: {:?})", media.title_display, media.media_kind);
            }
        }
        _ => {
            println!("Usage:");
            println!("  e2e_test_runner <db_path> --create-only");
            println!("  e2e_test_runner <db_path> --sync-tvmaze [--pages N] [--page-size N]");
            println!(
                "  e2e_test_runner <db_path> --sync-imdb [--media-kind movie|show] [--pages N] [--page-size N]"
            );
            println!("  e2e_test_runner <db_path> --remote-search <query>");
        }
    }

    Ok(())
}
