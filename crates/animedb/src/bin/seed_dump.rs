use animedb::{
    AniListProvider, JikanProvider, KitsuProvider, MediaKind, Provider, SourceName, SyncCursor,
    SyncRequest, TvmazeProvider,
};
use flate2::Compression;
use flate2::write::GzEncoder;
use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::thread;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <provider>", args[0]);
        eprintln!("Providers: anilist, jikan, kitsu, tvmaze");
        std::process::exit(1);
    }

    let source = args[1].parse::<SourceName>()?;
    let provider: Box<dyn Provider> = match source {
        SourceName::AniList => Box::new(AniListProvider::default()),
        SourceName::Jikan => Box::new(JikanProvider::default()),
        SourceName::Kitsu => Box::new(KitsuProvider::default()),
        SourceName::Tvmaze => Box::new(TvmazeProvider::default()),
        _ => {
            eprintln!("Dump not supported/needed for {}", source);
            std::process::exit(1);
        }
    };

    let filename = format!("{}_dump.jsonl.gz", source);
    let file = File::create(&filename)?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut writer = BufWriter::new(encoder);

    let media_kinds = if source == SourceName::Tvmaze {
        vec![MediaKind::Show]
    } else {
        vec![MediaKind::Anime, MediaKind::Manga]
    };

    let mut total_items = 0;

    for kind in media_kinds {
        let mut cursor = SyncCursor::default();
        let request = SyncRequest::new(source)
            .with_media_kind(kind)
            .with_page_size(50);

        println!("Dumping {} {}...", source, kind.as_str());

        loop {
            let page = provider.fetch_page(&request, cursor.clone())?;
            let items_count = page.items.len();

            for item in page.items {
                // Fetch episodes if supported
                if kind == MediaKind::Show || kind == MediaKind::Anime {
                    if let Ok(episodes) =
                        provider.fetch_episodes(item.media_kind, &item.external_ids[0].source_id)
                    {
                        // Note: To properly save this, we either store it in a unified struct
                        // or just dump the CanonicalMedia with an ad-hoc extra field.
                        // Here we just write it separately or inline it. For simplicity, we inline as an extension.
                        let mut item_val = serde_json::to_value(&item)?;
                        if !episodes.is_empty() {
                            item_val["episodes"] = serde_json::to_value(episodes)?;
                        }
                        let json = serde_json::to_string(&item_val)?;
                        writeln!(writer, "{}", json)?;
                    } else {
                        let json = serde_json::to_string(&item)?;
                        writeln!(writer, "{}", json)?;
                    }
                } else {
                    let json = serde_json::to_string(&item)?;
                    writeln!(writer, "{}", json)?;
                }
            }

            total_items += items_count;
            println!(
                "  Page {}: fetched {} items (Total: {})",
                cursor.page, items_count, total_items
            );

            let Some(next_cursor) = page.next_cursor else {
                break;
            };

            cursor = next_cursor;

            let sleep_for = provider.min_interval();
            if !sleep_for.is_zero() {
                thread::sleep(sleep_for);
            }
        }
    }

    println!("Finished dumping {}. Total items: {}", source, total_items);
    Ok(())
}
