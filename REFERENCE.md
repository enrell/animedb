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

## AniList Catalog

| Method | Path                | Description |
|--------|---------------------|-------------|
| GET    | `/anilist/media/search` | Returns up to five fuzzy-matched AniList records with similarity scores. |
| GET    | `/anilist/media`    | Paginated list of AniList media (anime/manga) stored in Postgres. |
| GET    | `/anilist/media/{id}` | Detailed record for a specific AniList media entry. |

### `/anilist/media` Query Parameters

| Name         | Type   | Default | Notes |
|--------------|--------|---------|-------|
| `page`       | int    | `1`     | 1-based page index. |
| `page_size`  | int    | `20`    | Max `100`. |
| `search`     | string | –       | Fuzzy match (trigram) against normalized romaji/English/native titles, tolerant of noisy input. |
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
| `search` | string | –       | Required. Fuzzy match term; returns up to five results with a `score` field. |

## MyAnimeList Catalog (Jikan)

| Method | Path                   | Description |
|--------|------------------------|-------------|
| GET    | `/myanimelist/anime/search` | Returns up to five fuzzy-matched MyAnimeList records with similarity scores. |
| GET    | `/myanimelist/anime`   | Paginated list of anime fetched via the Jikan API and stored locally. |
| GET    | `/myanimelist/anime/{id}` | Detailed record for a specific MyAnimeList entry. |

### `/myanimelist/anime` Query Parameters

| Name        | Type   | Default | Notes |
|-------------|--------|---------|-------|
| `page`      | int    | `1`     | 1-based page index. |
| `page_size` | int    | `20`    | Max `100`. |
| `search`    | string | –       | Fuzzy match (trigram) across normalized primary/English/Japanese titles, tolerant of noisy input. |
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
| `search` | string | –       | Required. Fuzzy match term; returns up to five results with a `score` field. |

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

Detail endpoints return a single record with fields mirroring the Postgres schema (see `cmd/anilist/main.go` and `cmd/myanimelist/main.go` for full column-to-field mappings).

## Example Requests

**AniList search:**
```bash
curl 'http://localhost:8081/anilist/media/search?search=slime'
```

**MyAnimeList paginated list:**
```bash
curl 'http://localhost:8081/myanimelist/anime?page=2&page_size=10&type=TV'
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
