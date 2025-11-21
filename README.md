# AnimeDB Ingestion Tools

This repository contains two Go CLIs that mirror data from public anime APIs into local PostgreSQL databases:

- `cmd/anilist` pulls media data from the AniList GraphQL API into a database named `anilist`.
- `cmd/myanimelist` pulls anime data from the Jikan REST API (MyAnimeList mirror) into a database named `myanimelist`.

Both tools automatically create their target databases (and tables) if they do not already exist.

## Prerequisites

- Go 1.21+ (module currently targets Go `1.25.1` for toolchain compatibility).
- PostgreSQL running locally with a user/database `root`/`root` as described in the task request.
  - Update the connection string via the `--dsn` flag if your environment differs.
- The ingestors enable the `unaccent` and `pg_trgm` extensions automatically; ensure your Postgres installation allows those extensions.

## Docker Quickstart

Spin up everything (Postgres, ingestion pipeline, and API) with Docker:

```bash
docker compose up -d --build
```

Services:

- `postgres` – Persistent Postgres instance seeded with the `root` user/database.
- `seed` – Runs `scripts/docker-seed.sh`, which sequentially executes `cmd/anilist`, `cmd/myanimelist`, and `cmd/videos` to provision schemas/data. It exits after a successful pass.
- `api` – Starts the HTTP API once Postgres is healthy and the seed pipeline completes. The API listens on `http://localhost:8081`.

Useful commands:

- `docker compose logs -f seed` – Follow the ingestion progress (the service stops after success).
- `docker compose logs -f api` – Tail the API server logs.
- `docker compose down -v` – Tear everything down, including the Postgres volume.

All DSNs inside the containers point at the internal `postgres` hostname (`postgres://root:root@postgres:5432/...`). Adjust the compose file if you need different credentials.

## Building

```bash
go build ./...
```

This validates that both commands compile.

## AniList Ingestion

```bash
go run ./cmd/anilist --dsn postgres://root:root@localhost:5432/root?sslmode=disable
```

Key flags:

- `--dsn`: Admin-level Postgres connection string used to create/access the `anilist` database. Default matches the provided credentials.
- `--database`: Override the default `anilist` database name (useful for testing).
- `--per-page`: Page size per AniList GraphQL request (max 50).
- `--start-page`: Starting page number (default 1).
- `--max-pages`: Limit total pages processed (0 means fetch everything).
- `--media-type`: AniList `MediaType` filter (default `ANIME`, accepts `MANGA`).

The command respects AniList’s 90 requests/minute rate limit by dynamically pacing requests based on the response headers. On 429 responses it waits for the advertised reset window before retrying.

## MyAnimeList (Jikan) Ingestion

```bash
go run ./cmd/myanimelist --dsn postgres://root:root@localhost:5432/root?sslmode=disable
```

Key flags:

- `--dsn`: Admin-level Postgres connection string used to create/access the `myanimelist` database.
- `--database`: Override the default `myanimelist` database name (useful for testing).
- `--per-page`: Page size per request (Jikan caps at 25; default 25).
- `--start-page`: Starting page number (default 1).
- `--max-pages`: Limit total pages processed (0 means fetch everything).

This command honours Jikan’s published rate limits by observing the `X-RateLimit-*` headers and `Retry-After` responses, backing off whenever the API signals throttling.

## Output Schema Overview

Each command creates a single `media`/`anime` table with indexed fields reflecting the public API payloads. Nested/array fields are stored as `JSONB` so that additional attributes can be accessed through PostgreSQL's JSON operators without schema changes.

Both tables use functional trigram GIN indexes on title expressions for fast fuzzy matching. Searches are accent-insensitive and normalize titles to lower case and ASCII.

See the definitions in:

- `cmd/anilist/main.go` (`CREATE TABLE ... media`)
- `cmd/myanimelist/main.go` (`CREATE TABLE ... anime`)

## Operational Notes

- Both ingestors stream sequentially through pages until the API reports no additional data or you hit the `--max-pages` limit.
- Tables are populated using UPSERTs so re-running a command keeps the databases in sync.
- Long-running ingestions can be interrupted with `Ctrl+C`; the commands handle context cancellation cleanly and leave committed data intact.

## Smoke Test Script

Run a quick end-to-end verification that exercises both ingestors against temporary databases and tears them down afterward:

```bash
go run ./cmd/test-ingest --dsn postgres://root:root@localhost:5432/root?sslmode=disable
```

- The script provisions two random database names, limits each ingestor to a single API page, validates that at least one row is stored, and finally drops both databases (using `DROP DATABASE ... WITH (FORCE)`).
- Override `--anilist-pages`, `--anilist-per-page`, `--mal-pages`, or `--mal-per-page` to tweak throughput while keeping the test lightweight.

## REST API Server

Serve the ingested data over HTTP for web apps:

```bash
go run ./cmd/api :8081
```

The positional argument sets the listen address (defaults to `:8081`). You can still override DSNs with flags:

```bash
go run ./cmd/api :8081 \
  --admin-dsn postgres://root:root@localhost:5432/root?sslmode=disable \
  --anilist-dsn postgres://root:root@localhost:5432/anilist?sslmode=disable \
  --myanimelist-dsn postgres://root:root@localhost:5432/myanimelist?sslmode=disable
```

Key endpoints:

- `GET /healthz` – health check.
- `GET /search/realtime` – unified real-time search (supports both AniList and MyAnimeList with source filtering).
- `GET /anilist/media/search` – top K AniList matches ranked by BM25 with season awareness (`search`, `limit` ≤50, default 10).
- `GET /anilist/media` – paginated AniList catalogue (`page`, `page_size` ≤500, `search`, `type`, `season`, `season_year`).
- `GET /anilist/media/{id}` – single AniList media record.
- `GET /myanimelist/anime/search` – top K MyAnimeList matches ranked by BM25 (`search`, `limit` ≤50, default 10).
- `GET /myanimelist/anime` – paginated MyAnimeList catalogue (`page`, `page_size` ≤500, `search`, `type`, `season`, `year`).
- `GET /myanimelist/anime/{id}` – single MyAnimeList anime record.

Search endpoints use two-stage ranking:
- **Stage 1 (Prefilter)**: Trigram similarity returns top 100 candidates from PostgreSQL.
- **Stage 2 (Re-rank)**: BM25 algorithm with n-grams (up to trigrams) and season matching.

Example quick search (returns up to 10 results, configurable via `limit`):

```bash
curl 'http://localhost:8081/anilist/media/search?search=tensei%20slime&limit=5'
```

Example realtime unified search (defaults to source=both):

```bash
curl 'http://localhost:8081/search/realtime?q=attack%20titan&source=both&limit=10'
```

Paginated responses include metadata, e.g.:

```jsonc
{
  "data": [...],
  "pagination": {
    "page": 1,
    "page_size": 20,
    "total": 1234
  }
}
```

The server checks that the backing databases exist (creating them through the admin DSN when missing) and applies query timeouts to keep requests responsive.

## Operational Notes

- Both ingestors stream sequentially through pages until the API reports no additional data or you hit the `--max-pages` limit.
- Tables are populated using UPSERTs so re-running a command keeps the databases in sync.
- Long-running ingestions can be interrupted with `Ctrl+C`; the commands handle context cancellation cleanly and leave committed data intact.
