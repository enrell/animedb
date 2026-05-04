# AnimeDB Reference

Reference for the Rust workspace, the library crates, and the GraphQL service that ships with this repository.

## Workspace

- `crates/animedb`: local-first and remote-first metadata library
- `crates/animedb-api`: GraphQL API built on top of `animedb`

## Crate `animedb`

`animedb` is the main integration surface for media servers and other Rust applications.

It supports two usage modes:

- local-first: create and maintain a SQLite catalog with normalized metadata, full-text search, sync state, provenance, and raw source payloads
- remote-first: query normalized metadata directly from remote providers without managing local persistence

### Local-first API

Open or create a catalog:

```rust
use animedb::AnimeDb;

let db = AnimeDb::open("/tmp/animedb.sqlite")?;
# Ok::<(), animedb::Error>(())
```

Create a catalog and download the default sources:

```rust
use animedb::AnimeDb;

let (db, report) = AnimeDb::generate_database_with_report("/tmp/animedb.sqlite")?;
println!("upserted {}", report.total_upserted_records);
# Ok::<(), animedb::Error>(())
```

Sync the default sources into an existing database:

```rust
use animedb::AnimeDb;

let report = AnimeDb::sync_database("/tmp/animedb.sqlite")?;
println!("upserted {}", report.total_upserted_records);
# Ok::<(), animedb::Error>(())
```

Run provider-specific sync:

```rust
use animedb::{AnimeDb, MediaKind};

let mut db = AnimeDb::open("/tmp/animedb.sqlite")?;
db.sync_anilist(MediaKind::Anime)?;
db.sync_jikan(MediaKind::Anime)?;
db.sync_kitsu(MediaKind::Anime)?;
# Ok::<(), animedb::Error>(())
```

Query the local catalog:

```rust
use animedb::{AnimeDb, SourceName};

let db = AnimeDb::open("/tmp/animedb.sqlite")?;
let monster = db.anime_metadata().by_external_id(SourceName::AniList, "19")?;
println!("{}", monster.name());

let movies = db.movie_metadata().search("paprika")?;
println!("movie hits: {}", movies.len());
# Ok::<(), animedb::Error>(())
```

### Remote-first API

Use a provider-specific facade:

```rust
use animedb::AnimeDb;

let remote = AnimeDb::remote_anilist();
let results = remote.anime_metadata().search("monster")?;
let monster = remote.anime_metadata().by_id("19")?;
# Ok::<(), animedb::Error>(())
```

Or select the provider dynamically:

```rust
use animedb::{AnimeDb, RemoteSource};

let remote = AnimeDb::remote(RemoteSource::Kitsu);
let movies = remote.movie_metadata().search("paprika")?;
# Ok::<(), animedb::Error>(())
```

### Exposed types

Main public types re-exported by the crate:

- `AnimeDb`
- `MetadataCollection`
- `RemoteApi`
- `RemoteCollection`
- `RemoteSource`
- `CanonicalMedia`
- `StoredMedia`
- `CanonicalEpisode`
- `StoredEpisode`
- `MediaDocument`
- `SearchOptions`
- `SearchHit`
- `SyncRequest`
- `SyncReport`
- `SyncOutcome`
- `PersistedSyncState`
- `FieldProvenance`
- `SourcePayload`

### Database schema

The schema is owned by the crate and migrated automatically when the database is opened.

Current logical tables:

- `media`: canonical merged records
- `media_alias`: aliases and synonyms
- `media_external_id`: external IDs by source and media kind
- `source_record`: raw provider payloads and fetch/update metadata
- `field_provenance`: winner-by-field audit trail for merge decisions
- `sync_state`: persisted sync cursors and checkpoints
- `media_fts`: `FTS5` virtual table for title, alias, and synopsis search
- `episode`: episode metadata for anime and shows (season/episode numbers, titles, synopsis, air date, runtime, thumbnail)

### Episodes

Episode metadata can be fetched and stored for anime and shows. Only `KitsuProvider` currently implements `fetch_episodes`; other providers return a "not supported" error.

```rust
use animedb::{AnimeDb, SourceName, KitsuProvider, Provider};

let mut db = AnimeDb::open("/tmp/animedb.sqlite")?;

// The media must already exist in the local catalog
let doc = db.media_document_by_external_id(SourceName::Kitsu, "1")?;

// Fetch episodes from Kitsu and persist them
db.fetch_and_store_episodes(&KitsuProvider::new(), SourceName::Kitsu, "1")?;

// Query episodes by media
let episodes = db.episodes_for_media(doc.media.id)?;

// Or find specific episodes
let ep5 = db.episode_by_absolute_number(doc.media.id, 5)?;
let ep_s1_e3 = db.episode_by_season_episode(doc.media.id, 1, 3)?;
```

`MediaDocument` bundles a `StoredMedia` with its `Vec<StoredEpisode>`:

```rust
let doc = db.media_document_by_id(media_id)?;
for ep in doc.episodes {
    println!("{} - {}", ep.absolute_number.unwrap_or(0), ep.title_display);
}
```

SQLite configuration applied by the crate:

- `PRAGMA journal_mode=WAL`
- `PRAGMA synchronous=NORMAL`
- `PRAGMA foreign_keys=ON`
- `PRAGMA busy_timeout=5000`
- `PRAGMA temp_store=MEMORY`

### Providers

The current remote providers all implement the `Provider` trait:

- `AniListProvider`
- `JikanProvider`
- `KitsuProvider`
- `TvmazeProvider`
- `ImdbProvider`

### Provider trait

```rust
pub trait Provider: Send + Sync {
    fn source(&self) -> SourceName;

    /// Minimum wall-clock delay between successive requests to this provider.
    fn min_interval(&self) -> Duration {
        Duration::ZERO
    }

    fn fetch_page(&self, request: &SyncRequest, cursor: SyncCursor) -> Result<FetchPage>;

    fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<CanonicalMedia>>;

    fn get_by_id(&self, media_kind: MediaKind, source_id: &str) -> Result<Option<CanonicalMedia>>;

    /// Fetches currently trending or popular media, ideal for seeding catalogs.
    fn fetch_trending(&self, media_kind: MediaKind) -> Result<Vec<CanonicalMedia>> { ... }

    /// Fetches recommendations based on a given media item.
    fn fetch_recommendations(&self, media_kind: MediaKind, source_id: &str) -> Result<Vec<CanonicalMedia>> { ... }

    /// Fetches related media (sequels, prequels, spin-offs, etc.) for a given media item.
    fn fetch_related(&self, media_kind: MediaKind, source_id: &str) -> Result<Vec<CanonicalMedia>> { ... }

    /// Fetches episode metadata for a given media item.
    fn fetch_episodes(&self, media_kind: MediaKind, source_id: &str) -> Result<Vec<CanonicalEpisode>> { ... }
}
```

### FetchPage

```rust
pub struct FetchPage {
    pub items: Vec<CanonicalMedia>,
    /// `None` when the provider has no more pages.
    pub next_cursor: Option<SyncCursor>,
}
```

All providers normalize adult content into the canonical `nsfw` field.

## Crate `animedb-api`

`animedb-api` exposes the same model through GraphQL.

Run locally:

```bash
cargo run -p animedb-api
```

Environment variables:

- `ANIMEDB_DATABASE_PATH`: SQLite file path, default `/data/animedb.sqlite`
- `ANIMEDB_LISTEN_ADDR`: bind address, default `0.0.0.0:8080`

Endpoints:

- `GET /`: GraphQL Playground
- `POST /graphql`: GraphQL endpoint
- `GET /healthz`: health check

### Main GraphQL queries

- `health`
- `media(id: ID!)`
- `mediaByExternalId(source: SourceNameObject!, sourceId: String!)`
- `search(query: String!, options: SearchInput)`
- `syncState(source: SourceNameObject!, scope: String!)`
- `remoteSearch(source: SourceNameObject!, query: String!, options: SearchInput)`
- `remoteMedia(source: SourceNameObject!, sourceId: String!, mediaKind: MediaKindObject!)`

### Main GraphQL mutations

- `generateDatabase(maxPages: Int)`
- `syncDatabase(input: SyncInput)`

## Real pipeline

The repository includes a real-provider pipeline with no mocked data:

- `crates/animedb/examples/real_pipeline.rs`
- `scripts/test-real-pipeline.sh`
- `Makefile`

Useful commands:

- `make build`
- `make test`
- `make crate-real`
- `make test-real`
- `make docker-build`
- `make docker-run`
- `make debug-api`

## Advisory

`animedb` normalizes public metadata, but it does not override provider Terms of Service, attribution rules, authentication requirements, rate limits, or mature-content restrictions. Review each source before enabling automated sync in production.
