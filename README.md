# animedb

`animedb` is a Rust-first metadata project for local media servers.

> **Advisory**: `animedb` stores and normalizes public metadata, but it does not override
> provider Terms of Service, attribution requirements, authentication rules, rate limits or
> mature-content restrictions. Before enabling bulk sync for a source, verify that your
> intended usage is allowed by that source and configure conservative sync budgets.

It has two consumption modes that can be used separately or together:

- **local-first**: manage a local SQLite catalog with schema, downloads, sync, FTS5 search and
  JSON source payloads
- **remote-first**: query normalized metadata from remote providers without forcing client
  applications to manage persistence or provider-specific normalization

The project also ships a Rust GraphQL API on top of the same crate.

See [REFERENCE.md](REFERENCE.md) for the current library and API reference.

## Supported providers

| Provider | Media kinds | Data source | Episodes | Licensed under |
|----------|-------------|-------------|----------|---------------|
| AniList | Anime, Manga | GraphQL API | N | [AniList Terms](https://anilist.co/terms) |
| Jikan (MyAnimeList) | Anime, Manga | REST API | Y | Jikan MIT |
| Kitsu | Anime, Manga | REST API | Y | [Kitsu API Policy](https://kitsu.io/terms) |
| TVmaze | Shows | REST API | Y | CC BY-SA 4.0 |
| IMDb | Movies, Shows | Official TSV datasets | Y | [IMDb Conditions](https://www.imdb.com/conditions) |

## Workspace

- `crates/animedb` — library crate with SQLite schema management, sync and query APIs
- `crates/animedb-api` — GraphQL API binary built on top of `animedb`

## Feature flags

```toml
# Full featured (local SQLite + all providers) — default
animedb = "0.6.2"

# Remote-only, no SQLite dependency (safe for sqlx-based projects)
animedb = { version = "0.6.2", default-features = false, features = ["remote"] }
```

- `local-db` (default): local SQLite storage, sync state persistence, and the [`AnimeDb`] type.
  This feature pulls in `rusqlite` with a bundled SQLite.
- `remote` (default): remote provider clients and the normalized data model. Zero native
  dependencies.

**Why feature gates?** `local-db` requires `rusqlite` (bundled SQLite). Many Rust projects
already use `sqlx` with its own SQLite linkage, and Cargo rejects putting both in the same
dependency graph. If your project uses `sqlx`, depend on animedb with only `features = ["remote"]`
to get all provider clients, normalization types, and sync data structures without any SQLite
conflict.

## Current Rust surface

### Local-first

```rust
use animedb::{AnimeDb, SourceName};

let (db, report) = AnimeDb::generate_database_with_report("/tmp/animedb.sqlite")?;
println!("downloaded {} records", report.total_upserted_records);

let updated = AnimeDb::sync_database("/tmp/animedb.sqlite")?;
println!("synced {} records", updated.total_upserted_records);

let monster = db.anime_metadata().by_external_id(SourceName::AniList, "19")?;
println!("{}", monster.name());

let show = db.show_metadata().search("breaking bad")?;
let show = db.get_by_external_id(SourceName::Imdb, "tt0903747")?;

let movies = db.movie_metadata().search("spirited away")?;
println!("movie hits: {}", movies.len());

# Ok::<(), animedb::Error>(())
```

### Remote-first

```rust
use animedb::AnimeDb;

let remote = AnimeDb::remote_anilist();
let results = remote.anime_metadata().search("monster")?;
let media = remote.anime_metadata().by_id("19")?;
# Ok::<(), animedb::Error>(())
```

Or choose the provider dynamically:

```rust
use animedb::{AnimeDb, RemoteSource};

let remote = AnimeDb::remote(RemoteSource::Tvmaze);
let results = remote.show_metadata().search("breaking bad")?;
# Ok::<(), animedb::Error>(())
```

## Media kinds

All providers map to one of four supported kinds:

```rust
pub enum MediaKind {
    Anime,  // Japanese animation
    Manga,  // Japanese comics
    Show,   // TV series (from TVmaze / IMDb)
    Movie,  // Films (from IMDb)
}
```

## SQLite schema

The SQLite catalog is created and migrated by the crate itself. The current schema includes:

- `media` — canonical normalized records
- `media_alias` — normalized aliases and synonyms
- `media_external_id` — source-specific identifiers
- `source_record` — raw per-source JSON payloads and source update metadata
- `field_provenance` — winner-per-field audit trail for canonical merge decisions
- `sync_state` — persisted sync checkpoints/cursors
- `media_fts` — `FTS5` index for title, alias and synopsis search
- `episode` — episode metadata for anime and shows

### Episodes

The `episode` table stores enriched episode data fetched from providers. Key fields:

- `season_number`, `episode_number`, `absolute_number` — episode numbering
- `title_display`, `title_original` — localized titles
- `synopsis`, `air_date`, `runtime_minutes`, `thumbnail_url` — metadata

#### Single Media Sync

Query and merge episodes for a media record:

```rust
use animedb::{AnimeDb, SourceName};

let mut db = AnimeDb::open("/tmp/animedb.sqlite")?;

// Finds the stored media by Kitsu ID, then tries every episode-capable
// external ID attached to that merged media record (Jikan/MAL, Kitsu, TVmaze).
db.fetch_and_store_episodes_by_external_id(SourceName::Kitsu, "1")?;

// Retrieve the media document with its episode list
let doc = db.media_document_by_external_id(SourceName::Kitsu, "1")?;
println!("{} has {} episodes", doc.media.title_display, doc.episodes.len());
```

For remote-only callers that want one merged episode record per flat episode number, use the
merged aggregation helper. It fetches from every episode-capable external ID, groups by
`absolute_number.or(episode_number)`, skips records with no episode number, and selects each
field from the highest-priority provider that supplied a value. For anime episode data, Jikan
wins over Kitsu when both have a value, while Kitsu can still fill fields missing from Jikan.

```rust
use animedb::{MediaKind, RemoteApi};

let media = RemoteApi::jikan().anime_metadata().by_id("19")?.unwrap();
let episodes = RemoteApi::fetch_merged_episodes_from_external_ids(
    MediaKind::Anime,
    &media.external_ids,
)?;
println!("fetched {} merged episode records", episodes.len());
```

If you need the raw per-provider records instead, call the lower-level aggregation API directly:

```rust
use animedb::{MediaKind, RemoteApi};

let media = RemoteApi::jikan().anime_metadata().by_id("19")?.unwrap();
let provider_records = RemoteApi::fetch_episodes_from_external_ids(
    MediaKind::Anime,
    &media.external_ids,
)?;
println!("fetched {} provider episode records", provider_records.len());
```

If you intentionally want one provider only, call the provider facade directly:

```rust
let episodes = RemoteApi::jikan().anime_metadata().episodes("19")?;
```

#### Bulk Seeding

To seed the entire database with episode metadata (including high-performance IMDb bulk dump ingestion):

```rust
use animedb::AnimeDb;

let mut db = AnimeDb::open("/tmp/animedb.sqlite")?;

// Ingest IMDb episode dumps and query APIs for all other providers
let total = db.sync_service().sync_all_episodes()?;
println!("Synced {total} episode records across all providers");
```

Note: `media.episodes` is the total episode count from provider metadata.
`MediaDocument.episodes` is the enriched list of persisted episode records
fetched from a specific provider.

The connection is configured with:

- `PRAGMA journal_mode=WAL`
- `PRAGMA synchronous=NORMAL`
- `PRAGMA foreign_keys=ON`
- `PRAGMA busy_timeout=5000`
- `PRAGMA temp_store=MEMORY`

## GraphQL API

The GraphQL API is provided by `animedb-api`. Run it locally:

```bash
cargo run -p animedb-api
```

Environment variables:

- `ANIMEDB_DATABASE_PATH` — SQLite file path, default `/data/animedb.sqlite`
- `ANIMEDB_LISTEN_ADDR` — bind address, default `0.0.0.0:8080`

Query example (search shows and movies):

```graphql
{
  search(query: "breaking bad", options: { limit: 5, mediaKind: SHOW }) {
    mediaId
    titleDisplay
    mediaKind
    genres
    externalIds { source sourceId }
  }
}
```

## Docker

Build and run the Rust GraphQL API:

```bash
docker build -t animedb .
docker run --rm -p 8080:8080 -v $(pwd)/data:/data animedb
```

## Make targets

The repository includes a `Makefile` for common workflows:

- `make build` — compile the workspace
- `make test` — run the Rust test suite
- `make test-e2e` — run the end-to-end integration test (`scripts/e2e_test.sh`)
- `make docker-build` — build the API image
- `make docker-run` — run the API image locally
- `make debug-api` — run the GraphQL API directly with `cargo run`
