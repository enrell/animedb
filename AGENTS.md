# AGENTS.md

Agent guidance for animedb development.

## Project Overview

`animedb` is a Rust workspace for anime/manga metadata — local SQLite catalog + remote provider clients + a GraphQL API. Two crates:

- `crates/animedb` — core library (local-first + remote-first)
- `crates/animedb-api` — GraphQL API binary

## Workspace Structure

```
crates/animedb/src/
├── lib.rs                    # Public exports + feature-gated modules
├── main.rs                   # Binary entry (unused; animedb-api is the binary)
├── catalog.rs                # RemoteCatalog, RemoteMetadataCollection
├── db.rs                     # AnimeDb facade, MetadataCollection
├── error.rs                  # Error enum (thiserror)
├── merge.rs                  # Merge engine (merge_media, MergeDecision, etc.)
├── model.rs                  # Canonical types (CanonicalMedia, SearchOptions, etc.)
├── remote.rs                 # RemoteApi, RemoteCollection, RemoteSource
├── schema.rs                 # SQLite migrations
├── provider/                 # HTTP provider clients
│   ├── mod.rs               # Provider trait + default_registry
│   ├── http.rs              # HttpClient (lazy reqwest::blocking::Client)
│   ├── anilist.rs           # AniList GraphQL client
│   ├── jikan.rs             # Jikan (MyAnimeList) REST client
│   ├── kitsu.rs             # Kitsu REST client
│   ├── tvmaze.rs            # TVmaze REST client
│   ├── imdb.rs              # IMDb TSV client
│   ├── registry.rs          # ProviderRegistry
│   └── service.rs           # FetchPage trait
├── repository/              # SQLite persistence
│   ├── media.rs            # MediaRepository
│   ├── episodes.rs          # EpisodeRepository
│   ├── search.rs            # FTS5 search
│   ├── sync_state.rs       # SyncStateRepository
│   └── mod.rs
└── sync/                    # Sync orchestration
    ├── common.rs            # SyncReport, SyncOutcome, SyncRequest
    ├── episodes.rs         # Episode sync logic
    ├── media.rs            # Media sync logic
    └── mod.rs

crates/animedb-api/src/
├── lib.rs                   # GraphQL schema + Axum server
└── main.rs                  # Binary entry point
```

## Key Conventions

### Feature Flags

- `local-db` (default): SQLite storage via rusqlite.
- `remote` (default): provider clients and normalized types.

Both are on by default. Many `#[cfg(feature = "local-db")]` guards throughout the codebase.

### Error Handling

All errors flow through `animedb::Error` (from `error.rs`, derived via `thiserror::Error`). Variants:
- `Http(reqwest::Error)` — HTTP client errors
- `Sql(rusqlite::Error)` — SQLite errors
- `Provider(String)` — provider logic errors
- `Sync(String)` — mutex poisoning / sync state errors

### Provider HTTP Clients

`HttpClient` in `provider/http.rs` uses lazy initialization — the `reqwest::blocking::Client` is constructed on first HTTP call, not in `new()`. This prevents panics when `HttpClient` is constructed inside a `#[tokio::test]` runtime (which does not support blocking operations). Always use `.client()` to get the client.

### Merge Engine

`merge/` is pure domain logic — no SQLite, no async. Key public exports from `lib.rs`:

```rust
pub use merge::{merge_media, merge_episode_source_records, provider_weight, MergeDecision};
#[cfg(feature = "local-db")]
pub use merge::make_provenance;
```

### SQLite Schema

Schema migrations live in `schema.rs`. The DB version is tracked via SQLite `user_version`. Migrations run automatically on `AnimeDb::open`.

### Test Conventions

- `#[tokio::test]` uses `CurrentThread` runtime — no blocking operations allowed.
- Regression tests for `HttpClient` live inside `#[cfg(test)]` in `provider/http.rs`.
- `tokio` is a dev-dependency (added for regression tests).

## Version & Changelog

- Version is set in `[workspace.package]` in `Cargo.toml` (`version = "0.3.5"`).
- Each crate has its own `CHANGELOG.md` (Keep a Changelog format).
- On version bump: bump `workspace.package.version`, update both `CHANGELOG.md` files, then `cargo publish` each crate.

## Common Tasks

### Run tests
```bash
cargo test
```

### Run the GraphQL API locally
```bash
cargo run -p animedb-api
```

### Build Docker image
```bash
docker build -t animedb .
docker run --rm -p 8080:8080 -v $(pwd)/data:/data animedb
```

### Publish to crates.io
```bash
cargo publish -p animedb
cargo publish -p animedb-api
```

### Add a new provider
1. Add provider struct in `provider/` (e.g., `provider/foo.rs`).
2. Implement `Provider` trait from `provider/mod.rs`.
3. Register in `provider/registry.rs` (`default_registry()`).
4. Add module to `provider/mod.rs`.
5. Add `FooProvider` export to `lib.rs` `provider/` re-exports.

## Provider-Specific Notes

- **AniList**: GraphQL API. Uses `post_with_retry` for mutations. Rate-limited (90 req/min).
- **Jikan**: REST (MyAnimeList). Free, no auth. Rate-limited (3 req/sec).
- **Kitsu**: REST. Requires auth token for some endpoints. Uses JSON:API format.
- **TVmaze**: REST. Free, no auth. Good for show/movie metadata.
- **IMDb**: TSV datasets. Requires download + periodic refresh. Not real-time.

See `docs/` for provider-specific behavior and data coverage notes.
