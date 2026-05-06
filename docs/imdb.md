# IMDb Dataset (Dump) Implementation Status

**Source:** IMDb plain text dataset files (TSV)
**Download:** `https://datasets.imdbws.com/`

This document tracks the implementation status of all IMDb dataset operations in
`crates/animedb/src/provider.rs`.

---

## Overview

The `ImdbProvider` does **not** use a traditional REST API. Instead, it downloads
and parses IMDb's publicly available dataset files (TSV format, gzip-compressed).
These files are published by IMDb and updated regularly.

The `RemoteProvider` trait is implemented with:

- `fetch_page()` — scans the `title.basics.tsv.gz` dataset sequentially with page/offset pagination
- `search()` — scans the same file matching against primary title
- `get_by_id()` — looks up by `tconst` (IMDb ID) in the dataset

**Currently implemented operations:**

| Operation | Method | Status |
|-----------|--------|--------|
| Sequential scan of title.basics | `fetch_page()` | ✅ Full |
| Text search in title.basics | `search()` | ✅ Full |
| Direct ID lookup by tconst | `get_by_id()` | ✅ Full |

---

## Available Dataset Files

IMDb publishes several dataset files. Only a subset is currently used.

### Currently Used

| File | Contents | Status |
|------|----------|--------|
| `title.basics.tsv.gz` | Primary title data: type, title, year, runtime, genres | ✅ Parsed |
| `title.ratings.tsv.gz` | Average rating and vote count per title | ✅ Parsed (ratings only) |

### Not Yet Used

| File | Contents | Status |
|------|----------|--------|
| `title.akas.tsv.gz` | Alternative titles and AKA entries | ❌ Not Implemented |
| `title.crew.tsv.gz` | Director/writer information | ❌ Not Implemented |
| `title.principals.tsv.gz` | Cast/crew principals (ordering, category, job) | ❌ Not Implemented |
| `title.episode.tsv.gz` | Episode relationships (parent series, season, episode) | ❌ Not Implemented |
| `name.basics.tsv.gz` | Person names and known titles | ❌ Not Implemented |

---

## Title Types (Media Kinds)

The `title.basics.tsv.gz` file contains multiple title types. Currently mapped to `MediaKind`:

| titleType | MediaKind | Notes |
|-----------|-----------|-------|
| `movie` | `Movie` | Feature films |
| `tvMovie` | `Movie` | TV movies |
| `short` | `Movie` | Short films |
| `tvSeries` | `Show` | TV series |
| `tvMiniSeries` | `Show` | Mini series |
| `tvSpecial` | `Show` | TV specials |
| `video` | `Movie` | Videos |
| `videoGame` | — | Not mapped (game entries) |

The following types are **skipped** during import:

- `tvEpisode` — Episodes are linked to parent series; stored as part of show data
- `tvShort` — Short TV content
- `radioEpisode`, `radioMix`, `radioRail` — Legacy/unsupported

---

## Dataset Fields (title.basics.tsv)

| Column | Field | Status |
|--------|-------|--------|
| `tconst` | IMDb ID / source_id | ✅ Full |
| `titleType` | format (media kind hint) | ✅ Full |
| `primaryTitle` | title_display / title_english | ✅ Full |
| `originalTitle` | title_english (if different) | ✅ Full |
| `isAdult` | nsfw flag | ✅ Full |
| `startYear` | season_year | ✅ Full |
| `endYear` | (not stored) | — |
| `runtimeMinutes` | episodes (as proxy for runtime) | ✅ Full |
| `genres` | genres array | ✅ Full |

---

## Dataset Fields (title.ratings.tsv)

| Column | Field | Status |
|--------|-------|--------|
| `tconst` | IMDb ID (join key) | ✅ Full |
| `averageRating` | provider_rating (scaled 0-1) | ✅ Full |
| `numVotes` | (not stored) | — |

---

## Not Implemented Operations

### title.akas.tsv (Alternative Titles)

| Operation | Description | Status |
|-----------|-------------|--------|
| Parse AKA entries | Load alternative titles per title | ❌ Not Implemented |
| Map language/country variants | Store as `aliases` or `title_native` | ❌ Not Implemented |

### title.crew.tsv (Director/Writer)

| Operation | Description | Status |
|-----------|-------------|--------|
| Parse director/writer | Extract `directors` and `writers` | ❌ Not Implemented |
| Map to staff/people | Cross-reference with name.basics | ❌ Not Implemented |

### title.principals.tsv (Cast/Crew Principals)

| Operation | Description | Status |
|-----------|-------------|--------|
| Parse cast/crew principals | Load cast ordering and roles | ❌ Not Implemented |
| Map characters | Map `characters` field to Character entries | ❌ Not Implemented |
| Map job categories | Map `category` (actor, director, etc.) | ❌ Not Implemented |

### title.episode.tsv (Episode Relationships)

| Operation | Description | Status |
|-----------|-------------|--------|
| Parse episode data | Load parent series ID, season, episode number | ✅ Full |
| Build episode hierarchy | Map episodes to parent show | ✅ Full |
| Handle specials | Map `seasonNumber`, `episodeNumber` | ❌ Not Implemented |

### name.basics.tsv (People)

| Operation | Description | Status |
|-----------|-------------|--------|
| Parse person data | Load person names and known credits | ❌ Not Implemented |
| Cross-reference with crew/cast | Link people to their credits | ❌ Not Implemented |

---

## Notes

- **No rate limit** — IMDb datasets are downloaded files, not an API. `min_interval()` is `ZERO`.
- **Dataset refresh:** IMDb updates their dataset files daily. New downloads reflect current data.
- **Pagination:** Sequential line-based pagination over the gzip-compressed TSV
  file. No efficient seeking; each page re-downloads and re-scans from the
  beginning of `title.basics.tsv.gz`.
- **Search performance:** O(n) linear scan — search reads through the entire dataset for every query.
- **Adult content filter:** The `isAdult` flag is mapped to the `nsfw` field.
- **External IDs:** The `tconst` (e.g., `tt1234567`) is stored as the source ID,
  with IMDb URL `https://www.imdb.com/title/{tconst}/`.
- **Data freshness:** IMDb dataset files are refreshed daily but do not reflect real-time changes.
