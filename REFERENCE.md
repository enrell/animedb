# AnimeDB Reference

Reference for the Rust workspace, the library crates, and the GraphQL service
that ships with this repository.

## Workspace

- `crates/animedb`: Local-first and remote-first metadata library.
- `crates/animedb-api`: GraphQL API built on top of `animedb`.

## Crate `animedb`

`animedb` is the core library. It now follows a layered architecture:

- **Facade**: `AnimeDb` acts as the stable public facade.
- **Repositories**: `repository/` handles all SQLite persistence.
- **Merge Engine**: `merge/` contains pure domain logic for normalizing and merging metadata.
- **Sync Service**: `sync/` orchestrates provider-to-catalog data flow.
- **Provider Registry**: Dynamic registry for managing metadata sources.

### Local-first API

The `AnimeDb` struct remains your primary entry point:

```rust
use animedb::AnimeDb;

let mut db = AnimeDb::open("/tmp/animedb.sqlite")?;
```

The catalog provides access to repositories for granular control:

```rust
// Access media catalog
let media = db.media().get_media(id)?;

// Access episode storage
let episodes = db.episodes().episodes_for_media(media_id)?;

// Access sync and orchestration
db.sync_anilist(MediaKind::Anime)?;

// Use the specialized SyncService for advanced operations
let stats = db.sync_service().sync_all_episodes()?;
```

### Remote-first API

Use the `ProviderRegistry` or the `AnimeDb::remote` facade:

```rust
use animedb::{AnimeDb, SourceName};
use animedb::provider::default_registry;

let registry = default_registry();
let provider = registry.get(SourceName::AniList)?;
let results = provider.search("monster", Default::default())?;
```

Episode metadata can be fetched through one provider or aggregated from the
external IDs on a merged media record:

```rust
use animedb::{AnimeDb, MediaKind, RemoteApi};

let media = RemoteApi::jikan().anime_metadata().by_id("19")?.unwrap();
let provider_records =
    RemoteApi::fetch_episodes_from_external_ids(MediaKind::Anime, &media.external_ids)?;

let direct = RemoteApi::jikan().anime_metadata().episodes("19")?;
```

For local catalogs, use the unified storage path to fetch all episode-capable
sources and merge them into canonical `StoredEpisode` rows:

```rust
use animedb::{AnimeDb, SourceName};

let mut db = AnimeDb::open("/tmp/animedb.sqlite")?;
let episodes =
    db.fetch_and_store_episodes_by_external_id(SourceName::Jikan, "19")?;
```

## Migration Note

The library handles SQLite migrations automatically in `AnimeDb::open`.
The database version is tracked via `user_version` in the SQLite file.
Migrations are now encapsulated in `crates/animedb/src/schema.rs`.
