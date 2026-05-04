# AnimeDB Reference

Reference for the Rust workspace, the library crates, and the GraphQL service that ships with this repository.

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

## Migration Note

The library handles SQLite migrations automatically in `AnimeDb::open`. The database version is tracked via `user_version` in the SQLite file. Migrations are now encapsulated in `crates/animedb/src/schema.rs`.
