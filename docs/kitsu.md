# Kitsu API Implementation Status

**Base URL:** `https://kitsu.io/api/edge`
**Base API Path:** `/api/edge`

This document tracks the implementation status of all Kitsu API endpoints in `crates/animedb/src/provider.rs`.

---

## Overview

The `KitsuProvider` struct implements the `RemoteProvider` trait with three core methods:

- `fetch_page()` — paginated listing of anime/manga
- `search()` — text search with `filter[text]`
- `get_by_id()` — fetch single resource by ID

**Currently implemented endpoints:**

| Method | Path | Status |
|--------|------|--------|
| GET | `/anime` | ✅ Full |
| GET | `/manga` | ✅ Full |
| GET | `/anime/{id}` | ✅ Full |
| GET | `/manga/{id}` | ✅ Full |
| GET | `/anime` + `filter[text]` | ✅ Full |
| GET | `/manga` + `filter[text]` | ✅ Full |
| GET | `/trending/anime` | ✅ Full |
| GET | `/trending/manga` | ✅ Full |
| GET | `/anime/{id}/episodes` | ✅ Full |
| GET | `/episodes/{id}` | ✅ Full |
| GET | `/manga/{id}/chapters` | ✅ Full |
| GET | `/chapters/{id}` | ✅ Full |
| GET | `/categories` | ✅ Full |
| GET | `/categories/{id}` | ✅ Full |
| GET | `/characters` | ✅ Full |
| GET | `/characters/{id}` | ✅ Full |
| GET | `/people` | ✅ Full |
| GET | `/people/{id}` | ✅ Full |
| GET | `/castings` | ✅ Full |
| GET | `/castings/{id}` | ✅ Full |
| GET | `/media-relationships` | ✅ Full |
| GET | `/media-relationships/{id}` | ✅ Full |
| GET | `/mappings` | ✅ Full |
| GET | `/mappings/{id}` | ✅ Full |
| GET | `/streamers` | ✅ Full |
| GET | `/streamers/{id}` | ✅ Full |

---

## Group: Anime

### Anime [/anime/{id}]

| Operation | Method | Path | Query Params | Status |
|-----------|--------|------|--------------|--------|
| Fetch Collection | GET | `/anime` | `page[limit]`, `page[offset]`, `sort`, `include` | ✅ Full |
| Fetch Resource | GET | `/anime/{id}` | `include` | ✅ Full |

**Not Implemented:**

- [ ] `POST /anime` — Create new anime entry
- [ ] `PATCH /anime/{id}` — Update existing anime entry
- [ ] `DELETE /anime/{id}` — Delete anime entry

### Trending Anime [/trending/anime]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/trending/anime` | ✅ Full |

### Episodes [/episodes/{id}]

| Operation | Method | Path | Query Params | Status |
|-----------|--------|------|--------------|--------|
| Fetch Collection | GET | `/episodes` | `page[limit]`, `page[offset]` | ✅ Full |
| Fetch Resource | GET | `/episodes/{id}` | — | ✅ Full |

**Not Implemented:**

- [ ] `POST /episodes` — Create episode entry
- [ ] `PATCH /episodes/{id}` — Update episode entry
- [ ] `DELETE /episodes/{id}` — Delete episode entry

---

## Group: Manga

### Manga [/manga/{id}]

| Operation | Method | Path | Query Params | Status |
|-----------|--------|------|--------------|--------|
| Fetch Collection | GET | `/manga` | `page[limit]`, `page[offset]`, `sort`, `include` | ✅ Full |
| Fetch Resource | GET | `/manga/{id}` | `include` | ✅ Full |

**Not Implemented:**

- [ ] `POST /manga` — Create new manga entry
- [ ] `PATCH /manga/{id}` — Update existing manga entry
- [ ] `DELETE /manga/{id}` — Delete manga entry

### Trending Manga [/trending/manga]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/trending/manga` | ✅ Full |

### Chapters [/chapters/{id}]

| Operation | Method | Path | Query Params | Status |
|-----------|--------|------|--------------|--------|
| Fetch Collection | GET | `/chapters` | `page[limit]`, `page[offset]` | ✅ Full |
| Fetch Resource | GET | `/chapters/{id}` | — | ✅ Full |

**Not Implemented:**

- [ ] `POST /chapters` — Create chapter entry
- [ ] `PATCH /chapters/{id}` — Update chapter entry
- [ ] `DELETE /chapters/{id}` — Delete chapter entry

---

## Group: Categories

### Categories [/categories/{id}]

| Operation | Method | Path | Query Params | Status |
|-----------|--------|------|--------------|--------|
| Fetch Collection | GET | `/categories` | `page[limit]`, `page[offset]` | ✅ Full |
| Fetch Resource | GET | `/categories/{id}` | — | ✅ Full |

### Category Favorites [/category-favorites/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/category-favorites` | ❌ Not Implemented |
| Fetch Resource | GET | `/category-favorites/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/category-favorites` | ❌ Not Implemented |
| Update Resource | PATCH | `/category-favorites/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/category-favorites/{id}` | ❌ Not Implemented |

### Category Recommendations [/category-recommendations/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/category-recommendations` | ❌ Not Implemented |
| Fetch Resource | GET | `/category-recommendations/{id}` | ❌ Not Implemented |

---

## Group: Characters

### Anime Characters [/anime-characters/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/anime-characters` | ❌ Not Implemented |
| Fetch Resource | GET | `/anime-characters/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/anime-characters` | ❌ Not Implemented |
| Update Resource | PATCH | `/anime-characters/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/anime-characters/{id}` | ❌ Not Implemented |

### Manga Characters [/manga-characters/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/manga-characters` | ❌ Not Implemented |
| Fetch Resource | GET | `/manga-characters/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/manga-characters` | ❌ Not Implemented |
| Update Resource | PATCH | `/manga-characters/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/manga-characters/{id}` | ❌ Not Implemented |

### Characters [/characters/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/characters` | ✅ Full |
| Fetch Resource | GET | `/characters/{id}` | ✅ Full |
| Create Resource | POST | `/characters` | ❌ Not Implemented |
| Update Resource | PATCH | `/characters/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/characters/{id}` | ❌ Not Implemented |

---

## Group: Producers & Staff

### Anime Productions [/anime-productions/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/anime-productions` | ❌ Not Implemented |
| Fetch Resource | GET | `/anime-productions/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/anime-productions` | ❌ Not Implemented |
| Update Resource | PATCH | `/anime-productions/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/anime-productions/{id}` | ❌ Not Implemented |

### Anime Staff [/anime-staff/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/anime-staff` | ❌ Not Implemented |
| Fetch Resource | GET | `/anime-staff/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/anime-staff` | ❌ Not Implemented |
| Update Resource | PATCH | `/anime-staff/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/anime-staff/{id}` | ❌ Not Implemented |

### Manga Staff [/manga-staff/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/manga-staff` | ❌ Not Implemented |
| Fetch Resource | GET | `/manga-staff/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/manga-staff` | ❌ Not Implemented |
| Update Resource | PATCH | `/manga-staff/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/manga-staff/{id}` | ❌ Not Implemented |

### Producers [/producers/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/producers` | ❌ Not Implemented |
| Fetch Resource | GET | `/producers/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/producers` | ❌ Not Implemented |
| Update Resource | PATCH | `/producers/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/producers/{id}` | ❌ Not Implemented |

### People [/people/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/people` | ✅ Full |
| Fetch Resource | GET | `/people/{id}` | ✅ Full |
| Create Resource | POST | `/people` | ❌ Not Implemented |
| Update Resource | PATCH | `/people/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/people/{id}` | ❌ Not Implemented |

### Castings [/castings/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/castings` | ✅ Full |
| Fetch Resource | GET | `/castings/{id}` | ✅ Full |
| Create Resource | POST | `/castings` | ❌ Not Implemented |
| Update Resource | PATCH | `/castings/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/castings/{id}` | ❌ Not Implemented |

---

## Group: Media Relations

### Media Relationships [/media-relationships/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/media-relationships` | ✅ Full |
| Fetch Resource | GET | `/media-relationships/{id}` | ✅ Full |
| Create Resource | POST | `/media-relationships` | ❌ Not Implemented |
| Update Resource | PATCH | `/media-relationships/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/media-relationships/{id}` | ❌ Not Implemented |

### Mappings [/mappings/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/mappings` | ✅ Full |
| Fetch Resource | GET | `/mappings/{id}` | ✅ Full |
| Create Resource | POST | `/mappings` | ❌ Not Implemented |
| Update Resource | PATCH | `/mappings/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/mappings/{id}` | ❌ Not Implemented |

### Franchises [/franchises/{id}] (Deprecated)

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/franchises` | ❌ Not Implemented |
| Fetch Resource | GET | `/franchises/{id}` | ❌ Not Implemented |

### Installments [/installments/{id}] (Deprecated)

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/installments` | ❌ Not Implemented |
| Fetch Resource | GET | `/installments/{id}` | ❌ Not Implemented |

---

## Group: Streamers

### Streamers [/streamers/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/streamers` | ✅ Full |
| Fetch Resource | GET | `/streamers/{id}` | ✅ Full |
| Create Resource | POST | `/streamers` | ❌ Not Implemented |
| Update Resource | PATCH | `/streamers/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/streamers/{id}` | ❌ Not Implemented |

### Streaming Links [/streaming-links/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/streaming-links` | ❌ Not Implemented |
| Fetch Resource | GET | `/streaming-links/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/streaming-links` | ❌ Not Implemented |
| Update Resource | PATCH | `/streaming-links/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/streaming-links/{id}` | ❌ Not Implemented |

---

## Group: Users

### Users [/users/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/users` | ❌ Not Implemented |
| Fetch Resource | GET | `/users/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/users` | ❌ Not Implemented |
| Update Resource | PATCH | `/users/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/users/{id}` | ❌ Not Implemented |

### Blocks [/blocks/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/blocks` | ❌ Not Implemented |
| Fetch Resource | GET | `/blocks/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/blocks` | ❌ Not Implemented |
| Update Resource | PATCH | `/blocks/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/blocks/{id}` | ❌ Not Implemented |

### Favorites [/favorites/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/favorites` | ❌ Not Implemented |
| Fetch Resource | GET | `/favorites/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/favorites` | ❌ Not Implemented |
| Update Resource | PATCH | `/favorites/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/favorites/{id}` | ❌ Not Implemented |

### Follows [/follows/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/follows` | ❌ Not Implemented |
| Fetch Resource | GET | `/follows/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/follows` | ❌ Not Implemented |
| Update Resource | PATCH | `/follows/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/follows/{id}` | ❌ Not Implemented |

### Linked Accounts [/linked-accounts/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/linked-accounts` | ❌ Not Implemented |
| Fetch Resource | GET | `/linked-accounts/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/linked-accounts` | ❌ Not Implemented |
| Update Resource | PATCH | `/linked-accounts/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/linked-accounts/{id}` | ❌ Not Implemented |

### Profile Link Sites [/profile-link-sites/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/profile-link-sites` | ❌ Not Implemented |
| Fetch Resource | GET | `/profile-link-sites/{id}` | ❌ Not Implemented |

### Profile Links [/profile-links/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/profile-links` | ❌ Not Implemented |
| Fetch Resource | GET | `/profile-links/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/profile-links` | ❌ Not Implemented |
| Update Resource | PATCH | `/profile-links/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/profile-links/{id}` | ❌ Not Implemented |

### Roles [/roles/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/roles` | ❌ Not Implemented |
| Fetch Resource | GET | `/roles/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/roles` | ❌ Not Implemented |
| Update Resource | PATCH | `/roles/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/roles/{id}` | ❌ Not Implemented |

### Stats [/stats/{id}] (In Development)

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/stats` | ❌ Not Implemented |
| Fetch Resource | GET | `/stats/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/stats/{id}` | ❌ Not Implemented |

### User Roles [/user-roles/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/user-roles` | ❌ Not Implemented |
| Fetch Resource | GET | `/user-roles/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/user-roles` | ❌ Not Implemented |
| Delete Resource | DELETE | `/user-roles/{id}` | ❌ Not Implemented |

---

## Group: User Libraries

### Library Entries [/library-entries/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/library-entries` | ❌ Not Implemented |
| Fetch Resource | GET | `/library-entries/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/library-entries` | ❌ Not Implemented |
| Update Resource | PATCH | `/library-entries/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/library-entries/{id}` | ❌ Not Implemented |

### Library Entry Logs [/library-entry-logs/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/library-entry-logs` | ❌ Not Implemented |
| Fetch Resource | GET | `/library-entry-logs/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/library-entry-logs` | ❌ Not Implemented |
| Update Resource | PATCH | `/library-entry-logs/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/library-entry-logs/{id}` | ❌ Not Implemented |

### Library Events [/library-events/{id}] (In Development)

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/library-events` | ❌ Not Implemented |
| Fetch Resource | GET | `/library-events/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/library-events/{id}` | ❌ Not Implemented |

### List Imports [/list-imports/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/list-imports` | ❌ Not Implemented |
| Fetch Resource | GET | `/list-imports/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/list-imports` | ❌ Not Implemented |

---

## Group: Reactions

### Media Reactions [/media-reactions/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/media-reactions` | ❌ Not Implemented |
| Fetch Resource | GET | `/media-reactions/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/media-reactions` | ❌ Not Implemented |
| Update Resource | PATCH | `/media-reactions/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/media-reactions/{id}` | ❌ Not Implemented |

### Media Reaction Votes [/media-reaction-votes/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/media-reaction-votes` | ❌ Not Implemented |
| Fetch Resource | GET | `/media-reaction-votes/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/media-reaction-votes` | ❌ Not Implemented |
| Update Resource | PATCH | `/media-reaction-votes/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/media-reaction-votes/{id}` | ❌ Not Implemented |

### Reviews [/reviews/{id}] (Deprecated)

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/reviews` | ❌ Not Implemented |
| Fetch Resource | GET | `/reviews/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/reviews` | ❌ Not Implemented |
| Update Resource | PATCH | `/reviews/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/reviews/{id}` | ❌ Not Implemented |

### Review Likes [/review-likes/{id}] (Deprecated)

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/review-likes` | ❌ Not Implemented |
| Fetch Resource | GET | `/review-likes/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/review-likes` | ❌ Not Implemented |
| Update Resource | PATCH | `/review-likes/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/review-likes/{id}` | ❌ Not Implemented |

---

## Group: Posts

### Posts [/posts/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/posts` | ❌ Not Implemented |
| Fetch Resource | GET | `/posts/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/posts` | ❌ Not Implemented |
| Update Resource | PATCH | `/posts/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/posts/{id}` | ❌ Not Implemented |

### Post Likes [/post-likes/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/post-likes` | ❌ Not Implemented |
| Fetch Resource | GET | `/post-likes/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/post-likes` | ❌ Not Implemented |
| Delete Resource | DELETE | `/post-likes/{id}` | ❌ Not Implemented |

### Post Follows [/post-follows/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/post-follows` | ❌ Not Implemented |
| Fetch Resource | GET | `/post-follows/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/post-follows` | ❌ Not Implemented |
| Delete Resource | DELETE | `/post-follows/{id}` | ❌ Not Implemented |

---

## Group: Comments

### Comments [/comments/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/comments` | ❌ Not Implemented |
| Fetch Resource | GET | `/comments/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/comments` | ❌ Not Implemented |
| Update Resource | PATCH | `/comments/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/comments/{id}` | ❌ Not Implemented |

### Comment Likes [/comment-likes/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/comment-likes` | ❌ Not Implemented |
| Fetch Resource | GET | `/comment-likes/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/comment-likes` | ❌ Not Implemented |
| Delete Resource | DELETE | `/comment-likes/{id}` | ❌ Not Implemented |

---

## Group: Groups

### Groups [/groups/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/groups` | ❌ Not Implemented |
| Fetch Resource | GET | `/groups/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/groups` | ❌ Not Implemented |
| Update Resource | PATCH | `/groups/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/groups/{id}` | ❌ Not Implemented |

### Group Action Logs [/group-action-logs/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/group-action-logs` | ❌ Not Implemented |
| Fetch Resource | GET | `/group-action-logs/{id}` | ❌ Not Implemented |

### Group Bans [/group-bans/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/group-bans` | ❌ Not Implemented |
| Fetch Resource | GET | `/group-bans/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/group-bans` | ❌ Not Implemented |
| Delete Resource | DELETE | `/group-bans/{id}` | ❌ Not Implemented |

### Group Categories [/group-categories/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/group-categories` | ❌ Not Implemented |
| Fetch Resource | GET | `/group-categories/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/group-categories` | ❌ Not Implemented |
| Update Resource | PATCH | `/group-categories/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/group-categories/{id}` | ❌ Not Implemented |

### Group Invites [/group-invites/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/group-invites` | ❌ Not Implemented |
| Fetch Resource | GET | `/group-invites/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/group-invites` | ❌ Not Implemented |
| Update Resource | PATCH | `/group-invites/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/group-invites/{id}` | ❌ Not Implemented |

### Group Member Notes [/group-member-notes/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/group-member-notes` | ❌ Not Implemented |
| Fetch Resource | GET | `/group-member-notes/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/group-member-notes` | ❌ Not Implemented |
| Update Resource | PATCH | `/group-member-notes/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/group-member-notes/{id}` | ❌ Not Implemented |

### Group Members [/group-members/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/group-members` | ❌ Not Implemented |
| Fetch Resource | GET | `/group-members/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/group-members` | ❌ Not Implemented |
| Update Resource | PATCH | `/group-members/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/group-members/{id}` | ❌ Not Implemented |

### Group Neighbors [/group-neighbors/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/group-neighbors` | ❌ Not Implemented |
| Fetch Resource | GET | `/group-neighbors/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/group-neighbors` | ❌ Not Implemented |
| Delete Resource | DELETE | `/group-neighbors/{id}` | ❌ Not Implemented |

### Group Permissions [/group-permissions/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/group-permissions` | ❌ Not Implemented |
| Fetch Resource | GET | `/group-permissions/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/group-permissions` | ❌ Not Implemented |
| Delete Resource | DELETE | `/group-permissions/{id}` | ❌ Not Implemented |

### Group Reports [/group-reports/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/group-reports` | ❌ Not Implemented |
| Fetch Resource | GET | `/group-reports/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/group-reports` | ❌ Not Implemented |
| Update Resource | PATCH | `/group-reports/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/group-reports/{id}` | ❌ Not Implemented |

### Group Ticket Messages [/group-ticket-messages/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/group-ticket-messages` | ❌ Not Implemented |
| Fetch Resource | GET | `/group-ticket-messages/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/group-ticket-messages` | ❌ Not Implemented |
| Update Resource | PATCH | `/group-ticket-messages/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/group-ticket-messages/{id}` | ❌ Not Implemented |

### Group Tickets [/group-tickets/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/group-tickets` | ❌ Not Implemented |
| Fetch Resource | GET | `/group-tickets/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/group-tickets` | ❌ Not Implemented |
| Update Resource | PATCH | `/group-tickets/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/group-tickets/{id}` | ❌ Not Implemented |

### Leader Chat Messages [/leader-chat-messages/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/leader-chat-messages` | ❌ Not Implemented |
| Fetch Resource | GET | `/leader-chat-messages/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/leader-chat-messages` | ❌ Not Implemented |
| Update Resource | PATCH | `/leader-chat-messages/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/leader-chat-messages/{id}` | ❌ Not Implemented |

---

## Group: Reports

### Reports [/reports/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/reports` | ❌ Not Implemented |
| Fetch Resource | GET | `/reports/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/reports` | ❌ Not Implemented |
| Update Resource | PATCH | `/reports/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/reports/{id}` | ❌ Not Implemented |

---

## Group: Site Announcements

### Site Announcements [/site-announcements/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/site-announcements` | ❌ Not Implemented |
| Fetch Resource | GET | `/site-announcements/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/site-announcements` | ❌ Not Implemented |
| Update Resource | PATCH | `/site-announcements/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/site-announcements/{id}` | ❌ Not Implemented |

---

## Group: Media Follows

### Media Follows [/media-follows/{id}]

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/media-follows` | ❌ Not Implemented |
| Fetch Resource | GET | `/media-follows/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/media-follows` | ❌ Not Implemented |
| Update Resource | PATCH | `/media-follows/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/media-follows/{id}` | ❌ Not Implemented |

### Media Attributes [/media-attributes/{id}] (In Development)

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/media-attributes` | ❌ Not Implemented |
| Fetch Resource | GET | `/media-attributes/{id}` | ❌ Not Implemented |

### Media Attribute Votes [/media-attribute-votes/{id}] (In Development)

| Operation | Method | Path | Status |
|-----------|--------|------|--------|
| Fetch Collection | GET | `/media-attribute-votes` | ❌ Not Implemented |
| Fetch Resource | GET | `/media-attribute-votes/{id}` | ❌ Not Implemented |
| Create Resource | POST | `/media-attribute-votes` | ❌ Not Implemented |
| Update Resource | PATCH | `/media-attribute-votes/{id}` | ❌ Not Implemented |
| Delete Resource | DELETE | `/media-attribute-votes/{id}` | ❌ Not Implemented |

---

## Implementation Summary

| Category | Total Endpoints | Implemented | Not Implemented |
|----------|-----------------|-------------|-----------------|
| Anime | 7 | 4 | 3 |
| Manga | 7 | 4 | 3 |
| Categories | 3 | 0 | 3 |
| Characters | 3 groups | 0 | all |
| Producers & Staff | 4 groups | 0 | all |
| Media Relations | 3 | 0 | all |
| Streamers | 2 | 0 | all |
| Users | 8 groups | 0 | all |
| User Libraries | 4 | 0 | all |
| Reactions | 4 | 0 | all |
| Posts | 3 | 0 | all |
| Comments | 2 | 0 | all |
| Groups | 13 groups | 0 | all |
| Reports | 1 | 0 | all |
| Site Announcements | 1 | 0 | all |
| Media Follows | 3 | 0 | all |

---

## Notes

- The `KitsuProvider` only implements **GET** endpoints for anime/manga listing, search, and single-resource fetch.
- All `POST`, `PATCH`, and `DELETE` operations are **not implemented**.
- Authentication (OAuth2) is **not implemented** — all requests are anonymous.
- Rate limiting is enforced at **900ms minimum interval**.
- The project focuses on metadata aggregation for local anime/manga databases, not user-generated content.
