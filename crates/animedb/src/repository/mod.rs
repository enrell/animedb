//! SQLite persistence layer.
//!
//! All database operations are isolated in repositories. Each repository owns
//! one SQLite table (or set of related tables) and exposes only domain-typed
//! methods — no raw SQL leaks through the public API.
//!
//! ## Repository overview
//!
//! | Repository | Table(s) | Responsibility |
//! |------------|----------|----------------|
//! | `MediaRepository` | `media`, `media_alias`, `media_external_id` | Media record upsert and lookup by ID or external ID |
//! | `EpisodeRepository` | `episode`, `episode_source_record` | Canonical and source episode upsert and lookup |
//! | `SearchRepository` | `media_fts` + all media tables | FTS5 search, `MediaDocument` assembly |
//! | `SyncStateRepository` | `sync_state` | Cursor persistence across sync runs |
//!
//! ## Transaction semantics
//!
//! [`MediaRepository::upsert_media`] runs inside a transaction; all related
//! tables (`media`, `media_alias`, `media_external_id`, `source_record`,
//! `field_provenance`, `media_fts`) are updated atomically.

pub mod common;
pub mod episodes;
pub mod media;
pub mod search;
pub mod sync_state;

pub use episodes::EpisodeRepository;
pub use media::MediaRepository;
pub use search::SearchRepository;
pub use sync_state::SyncStateRepository;
