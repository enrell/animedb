# API Reference

Base URL defaults to `http://localhost:8081`. Run the server with:

```bash
go run ./cmd/api :8081
```

(Optional flags `--admin-dsn`, `--anilist-dsn`, `--myanimelist-dsn` still work when you need custom Postgres connections.)

## Health

| Method | Path      | Description      |
|--------|-----------|------------------|
| GET    | `/healthz` | Returns `{ "status": "ok" }` when the service is ready. |

## Realtime Search

| Method | Path                | Description |
|--------|---------------------|-------------|
| GET    | `/search/realtime` | Global unified search across both AniList and MyAnimeList with configurable source filtering. Returns top K results ranked by BM25. |

### `/search/realtime` Query Parameters

| Name     | Type   | Default | Notes |
|----------|--------|---------|-------|
| `q` | string | – | Required. Search query. |
| `limit` | int | `10` | Max results to return. Capped at 50. |
| `source` | string | `both` | Filter by source: `anilist`, `myanimelist`, or `both`. |

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

### `/anilist/media/{id}` Path Parameter

| Name | Type | Description |
|------|------|-------------|
| `id` | int  | AniList media identifier. |

### `/anilist/media/search` Query Parameters

| Name     | Type   | Default | Notes |
|----------|--------|---------|-------|
| `search` | string | –       | Required. Search query. Returns up to K results with a `score` field. |
| `limit` | int | `10` | Max results to return. Capped at 50. |

## MyAnimeList Catalog (Jikan)

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

### `/myanimelist/anime/{id}` Path Parameter

| Name | Type | Description |
|------|------|-------------|
| `id` | int  | MyAnimeList `mal_id`. |

### `/myanimelist/anime/search` Query Parameters

| Name     | Type   | Default | Notes |
|----------|--------|---------|-------|
| `search` | string | –       | Required. Search query. Returns up to K results with a `score` field. |
| `limit` | int | `10` | Max results to return. Capped at 50. |

## Response Shape

List endpoints return:

```jsonc
{
  "data": [ /* array of records */ ],
  "pagination": {
    "page": 1,
    "page_size": 20,
    "total": 1234
  }
}
```

Search endpoints return an array of objects with the matched identifier, title variants, and a `score` field:

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

Realtime search returns an array with `source` field indicating which database:

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

Detail endpoints return a single record with fields mirroring the Postgres schema (see `cmd/anilist/main.go` and `cmd/myanimelist/main.go` for full column-to-field mappings).

## Example Requests

**AniList search (top 5 results):**
```bash
curl 'http://localhost:8081/anilist/media/search?search=slime&limit=5'
```

**Realtime unified search:**
```bash
curl 'http://localhost:8081/search/realtime?q=attack%20titan&source=both&limit=10'
```

**MyAnimeList paginated list (page size 50):**
```bash
curl 'http://localhost:8081/myanimelist/anime?page=2&page_size=50&type=TV'
```

**AniList detail:**
```bash
curl 'http://localhost:8081/anilist/media/1535'
```

## Error Responses

Errors are returned as JSON with an HTTP status code and a message:

```jsonc
{
  "error": "not found"
}
```

400-level errors indicate invalid input; 500-level errors indicate server issues.
