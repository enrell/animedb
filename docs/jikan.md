# Jikan API Implementation Status

**Base URL:** `https://api.jikan.moe/v4`
**Base API Path:** `/v4`

This document tracks the implementation status of all Jikan API endpoints in
`crates/animedb/src/provider.rs`.

---

## Overview

The `JikanProvider` struct implements the `RemoteProvider` trait with three core methods:

- `fetch_page()` — paginated listing of anime/manga
- `search()` — text search with `q` parameter
- `get_by_id()` — fetch single resource by ID

**Currently implemented endpoints:**

| Method | Path | Status |
|--------|------|--------|
| GET | `/anime` | ✅ Full |
| GET | `/manga` | ✅ Full |
| GET | `/anime/{id}` | ✅ Full |
| GET | `/manga/{id}` | ✅ Full |
| GET | `/anime` + `q` | ✅ Full |
| GET | `/manga` + `q` | ✅ Full |

---

## Group: Anime

### Anime [/anime/{id}]

| Operation | Method | Path | Query Params | Status |
|-----------|--------|------|--------------|--------|
| Fetch Collection | GET | `/anime` | `page`, `limit`, `sfw`, `order_by`, `sort` | ✅ Full |
| Fetch Resource | GET | `/anime/{id}` | `extension` | ✅ Full |
| Full Details | GET | `/anime/{id}/full` | — | ❌ Not Implemented |
| Character Cast | GET | `/anime/{id}/characters` | — | ❌ Not Implemented |
| Staff List | GET | `/anime/{id}/staff` | — | ❌ Not Implemented |
| Episode List | GET | `/anime/{id}/episodes` | `page` | ✅ Full |
| Specific Episode | GET | `/anime/{id}/episodes/{episodeId}` | — | ❌ Not Implemented |
| News Articles | GET | `/anime/{id}/news` | — | ❌ Not Implemented |
| Forum Threads | GET | `/anime/{id}/forum` | — | ❌ Not Implemented |
| Videos Page | GET | `/anime/{id}/videos` | — | ❌ Not Implemented |
| Episode Videos | GET | `/anime/{id}/videos/episodes` | — | ❌ Not Implemented |
| Image Gallery | GET | `/anime/{id}/pictures` | — | ❌ Not Implemented |
| Viewing Statistics | GET | `/anime/{id}/statistics` | — | ❌ Not Implemented |
| Additional Info | GET | `/anime/{id}/moreinfo` | — | ❌ Not Implemented |
| Recommendations | GET | `/anime/{id}/recommendations` | — | ❌ Not Implemented |
| User Updates | GET | `/anime/{id}/userupdates` | — | ❌ Not Implemented |
| Reviews | GET | `/anime/{id}/reviews` | — | ❌ Not Implemented |
| Related Anime/Manga | GET | `/anime/{id}/relations` | — | ❌ Not Implemented |
| Opening/Ending Themes | GET | `/anime/{id}/themes` | — | ❌ Not Implemented |
| External Links | GET | `/anime/{id}/external` | — | ❌ Not Implemented |
| Streaming Sources | GET | `/anime/{id}/streaming` | — | ❌ Not Implemented |

---

## Group: Manga

### Manga [/manga/{id}]

| Operation | Method | Path | Query Params | Status |
|-----------|--------|------|--------------|--------|
| Fetch Collection | GET | `/manga` | `page`, `limit`, `sfw`, `order_by`, `sort` | ✅ Full |
| Fetch Resource | GET | `/manga/{id}` | `extension` | ✅ Full |
| Full Details | GET | `/manga/{id}/full` | — | ❌ Not Implemented |
| Character List | GET | `/manga/{id}/characters` | — | ❌ Not Implemented |
| News Articles | GET | `/manga/{id}/news` | — | ❌ Not Implemented |
| Forum Threads | GET | `/manga/{id}/forum` | — | ❌ Not Implemented |
| Image Gallery | GET | `/manga/{id}/pictures` | — | ❌ Not Implemented |
| Reading Statistics | GET | `/manga/{id}/statistics` | — | ❌ Not Implemented |
| Additional Info | GET | `/manga/{id}/moreinfo` | — | ❌ Not Implemented |
| Recommendations | GET | `/manga/{id}/recommendations` | — | ❌ Not Implemented |
| User Updates | GET | `/manga/{id}/userupdates` | — | ❌ Not Implemented |
| Reviews | GET | `/manga/{id}/reviews` | — | ❌ Not Implemented |
| Related Works | GET | `/manga/{id}/relations` | — | ❌ Not Implemented |
| External Links | GET | `/manga/{id}/external` | — | ❌ Not Implemented |

---

## Group: Characters

### Characters [/characters/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Character Info | GET | `/characters/{id}` | ❌ Not Implemented |
| Full Details | GET | `/characters/{id}/full` | ❌ Not Implemented |
| Anime Appearances | GET | `/characters/{id}/anime` | ❌ Not Implemented |
| Voice Actor Roles | GET | `/characters/{id}/voices` | ❌ Not Implemented |
| Manga Appearances | GET | `/characters/{id}/manga` | ❌ Not Implemented |
| Images | GET | `/characters/{id}/pictures` | ❌ Not Implemented |

---

## Group: People

### People [/people/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Person Info | GET | `/people/{id}` | ❌ Not Implemented |
| Full Details | GET | `/people/{id}/full` | ❌ Not Implemented |
| Anime Work History | GET | `/people/{id}/anime` | ❌ Not Implemented |
| Voice Roles | GET | `/people/{id}/voices` | ❌ Not Implemented |
| Manga Work History | GET | `/people/{id}/manga` | ❌ Not Implemented |
| Images | GET | `/people/{id}/pictures` | ❌ Not Implemented |

---

## Group: Seasonal

### Seasons [/seasons]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Season Archive | GET | `/seasons` | ❌ Not Implemented |
| Current Season | GET | `/seasons/now` | ❌ Not Implemented |
| Upcoming Season | GET | `/seasons/upcoming` | ❌ Not Implemented |
| Specific Season | GET | `/seasons/{year}/{season}` | ❌ Not Implemented |

### Schedules [/schedules]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Full Schedule | GET | `/schedules` | ❌ Not Implemented |
| Filtered by Day | GET | `/schedules/{filter}` | ❌ Not Implemented |

---

## Group: Top & Discovery

### Top [/top]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Top Anime | GET | `/top/anime` | ❌ Not Implemented |
| Top Manga | GET | `/top/manga` | ❌ Not Implemented |
| Top Characters | GET | `/top/characters` | ❌ Not Implemented |
| Top People | GET | `/top/people` | ❌ Not Implemented |
| Top Reviews | GET | `/top/reviews` | ❌ Not Implemented |

### Genres [/genres]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Anime Genre List | GET | `/genres/anime` | ❌ Not Implemented |
| Manga Genre List | GET | `/genres/manga` | ❌ Not Implemented |

---

## Group: Producers & Magazines

### Producers [/producers]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Search Producers | GET | `/producers` | ❌ Not Implemented |
| Producer Info | GET | `/producers/{id}` | ❌ Not Implemented |
| Full Producer Details | GET | `/producers/{id}/full` | ❌ Not Implemented |
| External Links | GET | `/producers/{id}/external` | ❌ Not Implemented |

### Magazines [/magazines]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Magazine List | GET | `/magazines` | ❌ Not Implemented |

---

## Group: Users

### Users [/users]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Search Users | GET | `/users` | ❌ Not Implemented |
| Recently Online | GET | `/users/recentlyonline` | ❌ Not Implemented |
| User by MAL ID | GET | `/users/userbyid/{id}` | ❌ Not Implemented |
| Profile | GET | `/users/{username}` | ❌ Not Implemented |
| Full Profile | GET | `/users/{username}/full` | ❌ Not Implemented |
| Statistics | GET | `/users/{username}/statistics` | ❌ Not Implemented |
| Favorites | GET | `/users/{username}/favorites` | ❌ Not Implemented |
| Updates | GET | `/users/{username}/userupdates` | ❌ Not Implemented |
| About Section | GET | `/users/{username}/about` | ❌ Not Implemented |
| History (all) | GET | `/users/{username}/history` | ❌ Not Implemented |
| Filtered History | GET | `/users/{username}/history/{type}` | ❌ Not Implemented |
| Friends List | GET | `/users/{username}/friends` | ❌ Not Implemented |
| Anime List | GET | `/users/{username}/animelist` | ❌ Not Implemented |
| Anime List by Status | GET | `/users/{username}/animelist/{status}` | ❌ Not Implemented |
| Manga List | GET | `/users/{username}/mangalist` | ❌ Not Implemented |
| Manga List by Status | GET | `/users/{username}/mangalist/{status}` | ❌ Not Implemented |
| Recommendations | GET | `/users/{username}/recommendations` | ❌ Not Implemented |
| User Reviews | GET | `/users/{username}/reviews` | ❌ Not Implemented |
| User Clubs | GET | `/users/{username}/clubs` | ❌ Not Implemented |
| External Links | GET | `/users/{username}/external` | ❌ Not Implemented |

---

## Group: Clubs

### Clubs [/clubs]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Search Clubs | GET | `/clubs` | ❌ Not Implemented |
| Club Info | GET | `/clubs/{id}` | ❌ Not Implemented |
| Member List | GET | `/clubs/{id}/members` | ❌ Not Implemented |
| Staff List | GET | `/clubs/{id}/staff` | ❌ Not Implemented |
| Related Clubs | GET | `/clubs/{id}/relations` | ❌ Not Implemented |

---

## Group: Reviews & Recommendations

### Reviews [/reviews]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Anime Reviews | GET | `/reviews/anime` | ❌ Not Implemented |
| Manga Reviews | GET | `/reviews/manga` | ❌ Not Implemented |

### Recommendations [/recommendations]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Anime Recommendations | GET | `/recommendations/anime` | ❌ Not Implemented |
| Manga Recommendations | GET | `/recommendations/manga` | ❌ Not Implemented |

---

## Group: Watch

### Watch [/watch]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Recent Episodes | GET | `/watch/episodes` | ❌ Not Implemented |
| Popular Episodes | GET | `/watch/episodes/popular` | ❌ Not Implemented |
| Recent Promos | GET | `/watch/promos` | ❌ Not Implemented |
| Popular Promos | GET | `/watch/promos/popular` | ❌ Not Implemented |

---

## Group: Random & Insights

### Random [/random]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Random Anime | GET | `/random/anime` | ❌ Not Implemented |
| Random Manga | GET | `/random/manga` | ❌ Not Implemented |
| Random Character | GET | `/random/characters` | ❌ Not Implemented |
| Random Person | GET | `/random/people` | ❌ Not Implemented |
| Random User | GET | `/random/users` | ❌ Not Implemented |

### Insights [/insights]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Site Insights | GET | `/insights` | ❌ Not Implemented |
| Current Trends | GET | `/insights/trends` | ❌ Not Implemented |

---

## Notes

- **Rate Limit:** Jikan has a limit of 3 requests per second and 60 requests per
  minute per IP. The `min_interval()` is set to 1100ms to respect this.
- **sfw parameter:** The current implementation sets `sfw=false` to allow adult content. This should be configurable.
- **Base URL:** The official public instance is `https://api.jikan.moe/v4`. Self-hosted instances are also available.
