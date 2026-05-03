# AniList API Implementation Status

**Base URL:** `https://graphql.anilist.co`
**API Type:** GraphQL (POST only)

This document tracks the implementation status of all AniList GraphQL API operations in `crates/animedb/src/provider.rs`.

---

## Overview

The `AniListProvider` struct implements the `RemoteProvider` trait with three core methods:

- `fetch_page()` — paginated listing via `Page { media }` query
- `search()` — text search via `Page { media(search: ...) }` query
- `get_by_id()` — fetch single resource via `Media(id: ...)` query

**Currently implemented operations:**

| Operation | GraphQL Path | Status |
|-----------|--------------|--------|
| `Page media` (paginated listing) | `Page(page, perPage) { media }` | ✅ Full |
| `Page media` + `search` filter | `Page { media(search: ...) }` | ✅ Full |
| `Media(id)` (single by ID) | `Media(id: ..., type: ...)` | ✅ Full |

---

## Group: Media (Anime/Manga)

### Page.media

| Operation | GraphQL Type | Arguments | Status |
|-----------|--------------|-----------|--------|
| Paginated listing | `Page` | `page`, `perPage`, `type`, `sort` | ✅ Full |
| Text search | `Page` | `search`, `type` | ✅ Full |
| Filter by format | `Page` | `format_in` | ❌ Not Implemented |
| Filter by status | `Page` | `status` | ❌ Not Implemented |
| Filter by season | `Page` | `season`, `seasonYear` | ❌ Not Implemented |
| Filter by genres | `Page` | `genre_in`, `genre_not_in` | ❌ Not Implemented |
| Filter by tags | `Page` | `tag_in`, `tag_not_in` | ❌ Not Implemented |
| Filter by year | `Page` | `startDate_lesser`, `startDate_greater` | ❌ Not Implemented |
| Filter by episodes | `Page` | `episodes_lesser`, `episodes_greater` | ❌ Not Implemented |
| Filter by duration | `Page` | `duration_lesser`, `duration_greater` | ❌ Not Implemented |
| Filter by country | `Page` | `countryOfOrigin` | ❌ Not Implemented |
| Filter by source | `Page` | `source` | ❌ Not Implemented |
| Filter by licensing | `Page` | `licensedById_in`, `isLicensed` | ❌ Not Implemented |
| Sort options | `Page` | `sort` (many values) | ❌ Not Implemented |
| Adult content filter | `Page` | `isAdult` | ❌ Not Implemented |

### Media (single resource)

| Operation | GraphQL Type | Arguments | Status |
|-----------|--------------|-----------|--------|
| Fetch by ID | `Media` | `id`, `type` | ✅ Full |
| Full details with relations | `Media` | `id`, type`, include` | ❌ Not Implemented |
| Characters | `Media` | `id`, `type` + characters query | ❌ Not Implemented |
| Staff | `Media` | `id`, `type` + staff query | ❌ Not Implemented |
| Studios | `Media` | `id`, `type` + studios query | ❌ Not Implemented |
| Trends | `Media` | `id` + `MediaTrend` query | ❌ Not Implemented |
| Airing schedule | `Media` | `id` + `AiringSchedule` query | ❌ Not Implemented |
| External links | `Media` | `id` + `externalLinks` | ❌ Not Implemented |
| Streaming episodes | `Media` | `id` + `streamingEpisodes` | ❌ Not Implemented |
| Reviews | `Media` | `id` + `reviews` query | ❌ Not Implemented |
| Recommendations | `Media` | `id` + `recommendations` query | ❌ Not Implemented |

---

## Group: Characters

### Character queries

| Operation | GraphQL Type | Arguments | Status |
|-----------|--------------|-----------|--------|
| Fetch character by ID | `Character` | `id` | ❌ Not Implemented |
| Character page listing | `Page` | `page`, `perPage` + `characters` | ❌ Not Implemented |
| Character search | `Page` | `search`, type filter | ❌ Not Implemented |
| Character media | `Character` | `id` + `media` | ❌ Not Implemented |
| Character voice actors | `Character` | `id` + `voiceActors` | ❌ Not Implemented |
| Character images | `Character` | `id` + `image` | ❌ Not Implemented |

---

## Group: Staff (People)

### Staff queries

| Operation | GraphQL Type | Arguments | Status |
|-----------|--------------|-----------|--------|
| Fetch staff by ID | `Staff` | `id` | ❌ Not Implemented |
| Staff page listing | `Page` | `page`, `perPage` + `staff` | ❌ Not Implemented |
| Staff search | `Page` | `search` + `staff` | ❌ Not Implemented |
| Staff media | `Staff` | `id` + `media` | ❌ Not Implemented |
| Staff voice roles | `Staff` | `id` + `voiceActors` | ❌ Not Implemented |
| Staff images | `Staff` | `id` + `image` | ❌ Not Implemented |

---

## Group: Studios

### Studio queries

| Operation | GraphQL Type | Arguments | Status |
|-----------|--------------|-----------|--------|
| Fetch studio by ID | `Studio` | `id` | ❌ Not Implemented |
| Studio page listing | `Page` | `page`, `perPage` + `studios` | ❌ Not Implemented |
| Studio media | `Studio` | `id` + `media` | ❌ Not Implemented |

---

## Group: Users

### User queries

| Operation | GraphQL Type | Arguments | Status |
|-----------|--------------|-----------|--------|
| Current viewer (auth) | `Viewer` | — | ❌ Not Implemented |
| User profile by name | `User` | `name` | ❌ Not Implemented |
| User by ID | `User` | `id` | ❌ Not Implemented |
| User favourites | `User` | `id` + `favourites` | ❌ Not Implemented |
| User statistics | `User` | `id` + `statistics` | ❌ Not Implemented |
| User anime list | `User` | `id` + `animeStatistics` | ❌ Not Implemented |
| User manga list | `User` | `id` + `mangaStatistics` | ❌ Not Implemented |
| User activity | `User` | `id` + `activities` | ❌ Not Implemented |
| User followers | `User` | `id` + `followers` | ❌ Not Implemented |
| User following | `User` | `id` + `following` | ❌ Not Implemented |
| User statistics (site) | SiteStatistics | — | ❌ Not Implemented |

---

## Group: MediaList (User Lists)

### MediaList queries

| Operation | GraphQL Type | Arguments | Status |
|-----------|--------------|-----------|--------|
| User's anime list | `MediaList` | `userId`, `type`, `status` | ❌ Not Implemented |
| User's manga list | `MediaList` | `userId`, `type`, `status` | ❌ Not Implemented |
| List entry by ID | `MediaList` | `id` | ❌ Not Implemented |
| List entries for media | `Page` | `mediaId` | ❌ Not Implemented |

---

## Group: Reviews

### Review queries

| Operation | GraphQL Type | Arguments | Status |
|-----------|--------------|-----------|--------|
| All reviews page | `Page` | `page`, `perPage` + `reviews` | ❌ Not Implemented |
| Reviews for media | `Media` | `id` + `reviews` | ❌ Not Implemented |
| Review by ID | `Review` | `id` | ❌ Not Implemented |
| User reviews | `User` | `id` + `reviews` | ❌ Not Implemented |

---

## Group: Recommendations

### Recommendation queries

| Operation | GraphQL Type | Arguments | Status |
|-----------|--------------|-----------|--------|
| All recommendations page | `Page` | `page`, `perPage` + `recommendations` | ❌ Not Implemented |
| Recommendations for media | `Media` | `id` + `recommendations` | ❌ Not Implemented |
| Recommendation by ID | `Recommendation` | `id` | ❌ Not Implemented |

---

## Group: Airing Schedule

### AiringSchedule queries

| Operation | GraphQL Type | Arguments | Status |
|-----------|--------------|-----------|--------|
| Airing schedules page | `Page` | `page`, `perPage` + `airingSchedules` | ❌ Not Implemented |
| Filter by media | `Page` | `mediaId` | ❌ Not Implemented |
| Filter by episode | `Page` | `episode` | ❌ Not Implemented |
| Filter by time range | `Page` | `airingAt_lesser`, `airingAt_greater` | ❌ Not Implemented |
| Notifcation query | `Notification` | — | ❌ Not Implemented |

---

## Group: Trends

### Trend queries

| Operation | GraphQL Type | Arguments | Status |
|-----------|--------------|-----------|--------|
| Media trends | `MediaTrend` | `mediaId`, `sort` | ❌ Not Implemented |
| Site trends | SiteStatistics | — | ❌ Not Implemented |

---

## Group: Mutations

### Write operations

| Operation | GraphQL Mutation | Status |
|-----------|-----------------|--------|
| Save media list entry | `SaveMediaListEntry` | ❌ Not Implemented |
| Delete media list entry | `DeleteMediaListEntry` | ❌ Not Implemented |
| Update media list entries (batch) | `UpdateMediaListEntries` | ❌ Not Implemented |
| Add anime to list | `SaveMediaListEntry` | ❌ Not Implemented |
| Add manga to list | `SaveMediaListEntry` | ❌ Not Implemented |
| Toggle favourite | `ToggleFavourite` | ❌ Not Implemented |
| Rate review | `RateReview` | ❌ Not Implemented |
| Delete review | `DeleteReview` | ❌ Not Implemented |

---

## Notes

- **Rate Limit:** AniList allows 90 requests per minute. The `min_interval()` is set to 700ms.
- **API Type:** AniList is a GraphQL API — all requests are POST to `https://graphql.anilist.co` with a JSON body containing `query` and `variables`.
- **Auth:** Currently no authentication is implemented (public queries only).
- **Pagination:** Cursor-based pagination via `Page.pageInfo.hasNextPage`.
- **MediaType enum:** Values are `ANIME`, `MANGA`.
- **MediaSort enum:** Values include `ID`, `TITLE_ROMAJI`, `TITLE_ENGLISH`, `START_DATE`, `POPULARITY_DESC`, `SCORE_DESC`, `TRENDING_DESC`, etc.