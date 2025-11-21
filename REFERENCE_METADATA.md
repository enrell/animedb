# Metadata API Reference

This reference covers the metadata providers (AniList and MyAnimeList) and the unified realtime search.

## Table of Contents

1. [Realtime Search](#realtime-search)
2. [AniList Catalog](#anilist-catalog)
3. [MyAnimeList Catalog](#myanimelist-catalog)

## Realtime Search

| Method | Path                | Description |
|--------|---------------------|-------------|
| GET    | `/search/realtime` | Global unified search across both AniList and MyAnimeList with configurable source filtering. Returns top K results ranked by BM25. |

### `/search/realtime` Query Parameters

| Name     | Type   | Default | Notes |
|----------|--------|---------|-------|
| `q`      | string | –       | **Required.** Search query. |
| `limit`  | int    | `10`    | Max results to return. Capped at 50. |
| `source` | string | `both`  | Filter by source: `anilist`, `myanimelist`, or `both`. |

**Response:**
```jsonc
[
  {
    "id": 1535,
    "title": "Tensei Shitara Slime Datta Ken",
    "romaji": "Tensei Shitara Slime Datta Ken",
    "english": "That Time I Got Reincarnated as a Slime",
    "native": "転生したらスライムだった件",
    "source": "anilist",
    "score": 0.84
  }
]
```

**Example:**
```bash
curl 'http://localhost:8081/search/realtime?q=attack%20titan&source=both&limit=10'
```

## AniList Catalog

| Method | Path                | Description |
|--------|---------------------|-------------|
| GET    | `/anilist/media/search` | Returns top K AniList matches ranked by BM25 with n-gram tokenization and season awareness. |
| GET    | `/anilist/media`    | Paginated list of AniList media (anime/manga) stored in Postgres. |
| GET    | `/anilist/media/{id}` | Detailed record for a specific AniList media entry. |

### `/anilist/media` Query Parameters

| Name         | Type   | Default | Notes |
|--------------|--------|---------|-------|
| `page`       | int    | `1`     | 1-based page index. |
| `page_size`  | int    | `20`    | Max `500`. |
| `search`     | string | –       | Fuzzy match (trigram prefilter → BM25 re-rank) against title fields. |
| `title_romaji` | string | –     | Case-insensitive partial match on the romaji title column. |
| `title_english` | string | –    | Case-insensitive partial match on the English title column. |
| `title_native` | string | –     | Case-insensitive partial match on the native title column. |
| `type`       | string | –       | Exact match on `type` column (e.g., `ANIME`, `MANGA`). |
| `season`     | string | –       | Exact match on `season` (accepted values upper-cased internally). |
| `season_year`| int    | –       | Filters by release year. |

**Response:**
```jsonc
{
  "data": [ /* array of AniList media records */ ],
  "pagination": {
    "page": 1,
    "page_size": 20,
    "total": 1234,
    "has_more": true
  }
}
```

### `/anilist/media/{id}` Path Parameter

| Name | Type | Description |
|------|------|-------------|
| `id` | int  | AniList media identifier. |

### `/anilist/media/search` Query Parameters

| Name     | Type   | Default | Notes |
|----------|--------|---------|-------|
| `search` | string | –       | **Required.** Search query. Returns up to K results with a `score` field. |
| `limit`  | int    | `10`    | Max results to return. Capped at 50. |

**Response:**
```jsonc
[
  {
    "id": 1535,
    "title": "Tensei Shitara Slime Datta Ken",
    "romaji": "Tensei Shitara Slime Datta Ken",
    "english": "That Time I Got Reincarnated as a Slime",
    "native": "転生したらスライムだった件",
    "score": 0.84
  }
]
```

**Examples:**
```bash
# Search AniList (top 5 results)
curl 'http://localhost:8081/anilist/media/search?search=slime&limit=5'

# Paginated list with filters
curl 'http://localhost:8081/anilist/media?page=1&page_size=50&type=ANIME&season=WINTER&season_year=2024'

# Get specific media
curl 'http://localhost:8081/anilist/media/1535'
```

## MyAnimeList Catalog

| Method | Path                   | Description |
|--------|------------------------|-------------|
| GET    | `/myanimelist/anime/search` | Returns top K MyAnimeList matches ranked by BM25. |
| GET    | `/myanimelist/anime`   | Paginated list of anime fetched via the Jikan API and stored locally. |
| GET    | `/myanimelist/anime/{id}` | Detailed record for a specific MyAnimeList entry. |

### `/myanimelist/anime` Query Parameters

| Name        | Type   | Default | Notes |
|-------------|--------|---------|-------|
| `page`      | int    | `1`     | 1-based page index. |
| `page_size` | int    | `20`    | Max `500`. |
| `search`    | string | –       | Fuzzy match (trigram prefilter → BM25 re-rank) across title fields. |
| `type`      | string | –       | Exact match on `type` column (e.g., `TV`, `Movie`). |
| `season`    | string | –       | Filters by season string (stored lowercase). |
| `year`      | int    | –       | Filters by release year. |

**Response:**
```jsonc
{
  "data": [ /* array of MyAnimeList anime records */ ],
  "pagination": {
    "page": 1,
    "page_size": 20,
    "total": 5678,
    "has_more": true
  }
}
```

### `/myanimelist/anime/{id}` Path Parameter

| Name | Type | Description |
|------|------|-------------|
| `id` | int  | MyAnimeList `mal_id`. |

### `/myanimelist/anime/search` Query Parameters

| Name     | Type   | Default | Notes |
|----------|--------|---------|-------|
| `search` | string | –       | **Required.** Search query. Returns up to K results with a `score` field. |
| `limit`  | int    | `10`    | Max results to return. Capped at 50. |

**Response:**
```jsonc
[
  {
    "id": 39535,
    "title": "Tensei shitara Slime Datta Ken",
    "score": 0.82
  }
]
```

**Examples:**
```bash
# Search MyAnimeList
curl 'http://localhost:8081/myanimelist/anime/search?search=attack%20titan&limit=10'

# Paginated list
curl 'http://localhost:8081/myanimelist/anime?page=2&page_size=50&type=TV&year=2023'

# Get specific anime
curl 'http://localhost:8081/myanimelist/anime/39535'
```
