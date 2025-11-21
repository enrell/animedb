# API Benchmark Guide

The repository includes a comprehensive benchmark script for testing API performance.

## Running Benchmarks

### Basic Usage

```bash
./scripts/benchmark.sh
```

Runs all benchmarks with 10 requests per endpoint (default).

### Custom Request Count

```bash
NUM_REQUESTS=20 ./scripts/benchmark.sh
```

Runs all benchmarks with 20 requests per endpoint.

### Verbose Output

```bash
VERBOSE=true ./scripts/benchmark.sh
```

Shows individual request timings instead of progress dots.

### Custom Base URL

```bash
BASE_URL=http://localhost:9000 ./scripts/benchmark.sh
```

Benchmarks against a different API endpoint.

### Combined Options

```bash
BASE_URL=http://api.example.com NUM_REQUESTS=50 VERBOSE=true ./scripts/benchmark.sh
```

## What Gets Benchmarked

The script tests:

1. **Health Check** - `/healthz`
2. **AniList Search** - Basic (limit=10) and max limit (limit=50)
3. **AniList Pagination** - Small (page_size=20) and large (page_size=500)
4. **MyAnimeList Search** - Basic and max limits
5. **MyAnimeList Pagination** - Small and large page sizes
6. **Realtime Search** - Both sources, AniList only, MAL only
7. **Search Variants** - 5 different queries × 4 limit values

## Output Format

Each endpoint benchmark shows:

```
[INFO] Benchmarking: AniList Search (basic)
[INFO] Endpoint: /anilist/media/search?search=slime&limit=10
[INFO] Requests: 10
..........
  Min                  : 0.0234s
  Max                  : 0.0456s
  Average              : 0.0345s
  Throughput           : 29.01 req/s
[OK] AniList Search (basic) completed
```

## Metrics Explained

- **Min** - Fastest response time observed
- **Max** - Slowest response time observed
- **Average** - Mean response time across all requests
- **Throughput** - Requests per second (higher is better)

## Performance Targets

Based on the 2-stage search implementation:

- **Stage 1 (Prefilter)** - Trigram lookup: <20ms
- **Stage 2 (Re-rank)** - BM25 on 100 candidates: <30ms
- **Total search response** - Target <50ms for typical queries

## Example Session

```bash
$ NUM_REQUESTS=5 ./scripts/benchmark.sh

╔════════════════════════════════════════╗
║     AnimeDB API Benchmark Suite        ║
╚════════════════════════════════════════╝

[INFO] Checking if API is running at http://localhost:8081...
[OK] API is healthy

[INFO] Starting benchmarks with 5 requests per endpoint
...
```
