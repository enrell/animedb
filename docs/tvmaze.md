# TVmaze API Implementation Status

**Base URL:** `https://api.tvmaze.com`
**API Type:** REST (JSON)

This document tracks the implementation status of all TVmaze API endpoints in
`crates/animedb/src/provider.rs`.

---

## Overview

The `TvmazeProvider` struct implements the `RemoteProvider` trait with three core methods:

- `fetch_page()` — paginated listing via `/shows?page=N`
- `search()` — text search via `/search/shows?q=query`
- `get_by_id()` — fetch single resource by ID via `/shows/{id}`

**Currently implemented endpoints:**

| Method | Path | Status |
|--------|------|--------|
| GET | `/shows` (paginated) | ✅ Full |
| GET | `/search/shows` | ✅ Full |
| GET | `/shows/{id}` | ✅ Full |

---

## Group: Shows

### Shows [/shows]

| Operation | Method | Path | Query Params | Status |
|-----------|--------|------|--------------|--------|
| Paginated listing | GET | `/shows` | `page` | ✅ Full |
| Fetch by ID | GET | `/shows/{id}` | — | ✅ Full |
| Show full details | GET | `/shows/{id}?embed=*` | embed params | ❌ Not Implemented |
| Show episodes | GET | `/shows/{id}/episodes` | `page`, `specials` | ✅ Full |
| Episode by ID | GET | `/episodes/{id}` | — | ❌ Not Implemented |
| Show cast | GET | `/shows/{id}/cast` | — | ❌ Not Implemented |
| Show crew | GET | `/shows/{id}/crew` | — | ❌ Not Implemented |
| Showakis (aka shows) | GET | `/shows/{id}/akas` | — | ❌ Not Implemented |
| Show images | GET | `/shows/{id}/images` | — | ❌ Not Implemented |
| Show schedule | GET | `/shows/{id}/schedule` | — | ❌ Not Implemented |
| Show season list | GET | `/shows/{id}/seasons` | — | ❌ Not Implemented |

### Episodes [/episodes]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch episode by ID | GET | `/episodes/{id}` | ❌ Not Implemented |
| Episode main index | GET | `/episodes/{id}/main` | ❌ Not Implemented |

---

## Group: Search

### Search [/search]

| Operation | Method | Path | Query Params | Status |
|-----------|--------|------|--------------|--------|
| Search shows | GET | `/search/shows` | `q`, `limit` | ✅ Full |
| Search people | GET | `/search/people` | `q` | ❌ Not Implemented |
| Search quotes | GET | `/search/quotes` | `q` | ❌ Not Implemented |

---

## Group: Schedules

### Schedule [/schedule]

| Operation | Method | Path | Query Params | Status |
|-----------|--------|------|--------------|--------|
| Full schedule | GET | `/schedule` | `country`, `date` | ❌ Not Implemented |
| Schedule by country | GET | `/schedule?country={code}` | `country`, `date` | ❌ Not Implemented |
| Schedule by date | GET | `/schedule?date={YYYY-MM-DD}` | `country`, `date` | ❌ Not Implemented |
| Schedule web only | GET | `/schedule?country=US&web=true` | `country`, `web` | ❌ Not Implemented |

---

## Group: People

### People [/people]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch person by ID | GET | `/people/{id}` | ❌ Not Implemented |
| Person cast credits | GET | `/people/{id}/castcredits` | ❌ Not Implemented |
| Person crew credits | GET | `/people/{id}/crewcredits` | ❌ Not Implemented |

---

## Group: Boards (Clubs)

### Boards (Clubs) [/boards]

TVmaze's equivalent of forums/groups.

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Board by ID | GET | `/boards/{id}` | ❌ Not Implemented |
| Board posts | GET | `/boards/{id}/posts` | ❌ Not Implemented |

---

## Group: Updates

### Updates [/updates]

| Operation | Method | Path | Query Params | Status |
|-----------|--------|------|--------------|--------|
| Show updates | GET | `/updates/shows` | — | ❌ Not Implemented |

---

## Group: Index

### Index [/indexes]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Web channels index | GET | `/indexes/webchannels` | ❌ Not Implemented |
| Networks index | GET | `/indexes/networks` | ❌ Not Implemented |
| Streaming scheduler index | GET | `/indexes/webstreaming` | ❌ Not Implemented |
| Streaming shows index | GET | `/web-shows` | ❌ Not Implemented |

---

## Group: Top

### Top lists [/top]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Top shows | GET | `/top/shows` | ❌ Not Implemented |

---

## Group: Genres

### Genres [/genres]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Genre list | GET | `/genres` | ❌ Not Implemented |

---

## Notes

- **Rate Limit:** TVmaze has no official rate limit, but `min_interval()` is set to 500ms as a courtesy.
- **Base URL:** Official public instance is `https://api.tvmaze.com`.
- **Embedding:** TVmaze supports embedding related resources via
  `?embed=cast,seasons,episodes` query params on show endpoints.
- **External IDs:** TVmaze provides IMDb and TVRage IDs in the `externals` field of shows.
- **Pagination:** TVmaze uses page-based pagination for `/shows` (not cursor-based).
- **Adult content:** No built-in SFW filter — all content is returned as-is.
