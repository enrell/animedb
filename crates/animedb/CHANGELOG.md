# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.2] - 2026-05-07

### Added

- **`remote.rs`**: Added `fetch_merged_episodes_from_external_ids` and related
  candidate-based variants for remote-only callers that want aggregated episode
  fetch plus deterministic merge in one API call.
- **`merge.rs`**: Added `merge_canonical_episodes_by_effective_number` for
  persistence-free deduplication of raw remote episode records by flat effective
  episode number with field-level provider priority.

### Fixed

- **`lib.rs`**: Feature-gated the SQLite repository module behind `local-db` so
  remote-only consumers can build with `default-features = false, features = ["remote"]`.

### Documentation

- **`README.md`**: Documented remote-only merged episode aggregation and updated
  dependency examples for `0.6.2`.

## [0.6.1] - 2026-05-06

### Added

- **`remote.rs`**: Added `RemoteApi::fetch_episodes`, `RemoteCollection::episodes`,
  and remote episode aggregation via `fetch_episodes_from_external_ids`.
- **`remote.rs`**: Added `EpisodeFetchCandidate` and candidate derivation from
  merged media `external_ids`, including MyAnimeList IDs routed through Jikan.
- **`db.rs`**: Added `fetch_and_store_episodes_for_media` and
  `fetch_and_store_episodes_by_external_id` to fetch all episode-capable source IDs
  for a stored media record, persist source records, and run a single canonical
  episode merge.
- **`db.rs` / `remote.rs`**: Added `show_metadata` and `tv_movie_metadata`
  facades for show and movie-scoped local/remote queries.

### Documentation

- **`README.md` / `REFERENCE.md`**: Documented unified episode aggregation and
  direct single-provider episode fetches.

## [0.6.0] - 2026-05-06

### Performance

- **`sync/service.rs`**: `sync_from` no longer fetches episodes inline per media
  item. This eliminates an N+1 HTTP pattern that caused severe slowdown when
  seeding large catalogs. Episode seeding is now explicitly done via
  `sync_all_episodes`.

### Added

- **`sync/service.rs`**: 7 new unit tests covering pagination, rate limiting,
  cursor persistence, and the absence of inline episode fetching.

## [0.5.0] - 2026-05-05

### Added

- **`provider/http.rs`**: Added `proxy` field to `HttpClient` and implemented
  `with_proxy` method to support programmatic proxy configuration for all providers.
- **`bin/seed_dump.rs`**: Added a new standalone CLI tool
  (`cargo run --bin seed_dump`) to fetch and compress full metadata catalogs using
  provider APIs.
- **`sync/service.rs`**: `sync_all_episodes` method for bulk episode metadata seeding across all providers.
- **`provider/imdb.rs`**: `fetch_all_episodes` method that streams and parses
  the IMDb `title.episode.tsv.gz` and `title.basics.tsv.gz` datasets efficiently.
- **`provider/tvmaze.rs`**: `fetch_episodes` implementation using the TVMaze REST API.
- **`provider/jikan.rs`**: `fetch_episodes` implementation using the Jikan (MyAnimeList) REST API.
- **`repository/episodes.rs`**: `upsert_episode_source_record_no_merge` for optimized batch insertions.
- **`db.rs`**: `get_episodes_by_external_id` for direct episode lookup by provider ID.

### Changed

- **`db.rs`**: `fetch_and_store_episodes_from` optimized to use batch upserts
  and a single merge per media item.
- **`repository/episodes.rs`**: `upsert_episode_source_record` refactored to use the new batch logic.

## [0.3.6] - 2026-05-05

### Fixed

- **`provider/anilist.rs`**: `fetch_page` now gracefully handles AniList's
  100-page hard limit. When a sync cursor exceeds page 100, the provider returns
  an empty page with no next cursor instead of propagating the GraphQL
  "Page must be between 1 and 100" error as a fatal failure.
- **`sync/service.rs`**: same fix, same file; `sync_anilist` calls `fetch_page`
  which now handles the page-limit error.

## [0.3.4] - 2026-05-04

### Fixed

- **`provider/http.rs`**: `HttpClient` now defers `reqwest::blocking::Client`
  construction to first HTTP call instead of `new()`. This fixes a panic that
  occurred when a provider was constructed inside a `#[tokio::test]` runtime.
- **`provider/anilist.rs`**: Updated `post_with_retry` to use the new
  `HttpClient::client()` method instead of the removed `.inner` field.
- **`error.rs`**: Added `Error::Sync` variant to handle `Arc<Mutex>` poisoning in the lazy HTTP client.

### Added

- **`provider/http.rs`**: Regression tests confirming `HttpClient` construction
  is safe inside both `#[tokio::test]` and `#[test]` environments.
- **`Cargo.toml`**: Added `tokio` as a dev-dependency for the regression tests.
- **`lib.rs`**: Re-exported `merge_media`, `merge_episode_source_records`,
  `provider_weight`, and `MergeDecision` publicly.
