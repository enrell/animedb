# animedb

`animedb` is a Rust-first metadata project for local media servers.

> Advisory
> `animedb` stores and normalizes public metadata, but it does not override provider Terms of Service, attribution requirements, authentication rules, rate limits or mature-content restrictions. Before enabling bulk sync for a source, verify that your intended usage is allowed by that source and configure conservative sync budgets.

It has two consumption modes that can be used separately or together:

- local-first: manage a local SQLite catalog with schema, downloads, sync, FTS5 search and JSON source payloads
- remote-first: query normalized metadata from remote providers without forcing client applications to manage persistence or provider-specific normalization

The project also ships a Rust GraphQL API on top of the same crate.

## Workspace

- `crates/animedb` - library crate with SQLite schema management, sync and query APIs
- `crates/animedb-api` - GraphQL API binary built on top of `animedb`

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

let remote = AnimeDb::remote(RemoteSource::Jikan);
let results = remote.movie_metadata().search("paprika")?;
# Ok::<(), animedb::Error>(())
```

## Database design

The SQLite catalog is created and migrated by the crate itself. The current schema includes:

- `media` - canonical normalized records
- `media_alias` - normalized aliases and synonyms
- `media_external_id` - source-specific identifiers
- `source_record` - raw per-source JSON payloads and source update metadata
- `field_provenance` - winner-per-field audit trail for canonical merge decisions
- `sync_state` - persisted sync checkpoints/cursors
- `media_fts` - `FTS5` index for title, alias and synopsis search

The connection is configured with:

- `PRAGMA journal_mode=WAL`
- `PRAGMA synchronous=NORMAL`
- `PRAGMA foreign_keys=ON`
- `PRAGMA busy_timeout=5000`
- `PRAGMA temp_store=MEMORY`

## GraphQL API

The GraphQL API is provided by `animedb-api`.

Run it locally:

```bash
cargo run -p animedb-api
```

Environment variables:

- `ANIMEDB_DATABASE_PATH` - SQLite file path, default `/data/animedb.sqlite`
- `ANIMEDB_LISTEN_ADDR` - bind address, default `0.0.0.0:8080`

Endpoints:

- `GET /` - GraphQL Playground
- `POST /graphql` - GraphQL endpoint
- `GET /healthz` - health check

## Docker

Build and run the Rust GraphQL API:

```bash
docker build -t animedb .
docker run --rm -p 8080:8080 -v $(pwd)/data:/data animedb
```

## Make targets

The repository now includes a `Makefile` for the common workflows:

- `make build` - compile the workspace
- `make test` - run the Rust test suite
- `make crate-real` - run the real-provider crate example against AniList, Jikan and Kitsu
- `make test-real` - run the end-to-end real pipeline: crate example + Docker API + GraphQL checks
- `make docker-build` - build the API image
- `make docker-run` - run the API image locally
- `make debug-api` - run the GraphQL API directly with `cargo run`
