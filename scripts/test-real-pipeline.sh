#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE_TAG="${IMAGE_TAG:-animedb:real-pipeline}"
API_PORT="${API_PORT:-18080}"
MAX_PAGES="${ANIMEDB_REAL_MAX_PAGES:-1}"
PAGE_SIZE="${ANIMEDB_REAL_PAGE_SIZE:-5}"
TMP_DIR="$(mktemp -d)"
CONTAINER_NAME="animedb-real-pipeline-$$"
HOST_DB_PATH="$TMP_DIR/host-real.sqlite"
API_DB_DIR="$TMP_DIR/api-data"

mkdir -p "$API_DB_DIR"

cleanup() {
  docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

require_bin() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

wait_for_health() {
  local attempts=30
  local url="http://127.0.0.1:${API_PORT}/healthz"
  for _ in $(seq 1 "$attempts"); do
    if curl -fsS "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  echo "api did not become healthy at $url" >&2
  docker logs "$CONTAINER_NAME" >&2 || true
  return 1
}

graphql() {
  local payload="$1"
  curl -fsS "http://127.0.0.1:${API_PORT}/graphql" \
    -H 'content-type: application/json' \
    --data "$payload"
}

assert_contains() {
  local haystack="$1"
  local needle="$2"
  if [[ "$haystack" != *"$needle"* ]]; then
    echo "expected response to contain: $needle" >&2
    echo "$haystack" >&2
    exit 1
  fi
}

require_bin cargo
require_bin docker
require_bin curl

cd "$ROOT_DIR"

echo "==> running crate real pipeline example"
ANIMEDB_REAL_MAX_PAGES="$MAX_PAGES" \
ANIMEDB_REAL_PAGE_SIZE="$PAGE_SIZE" \
cargo run -p animedb --example real_pipeline -- "$HOST_DB_PATH"

echo "==> building docker image"
docker build -t "$IMAGE_TAG" .

echo "==> starting api container"
docker run -d --rm \
  --name "$CONTAINER_NAME" \
  -p "${API_PORT}:8080" \
  -e ANIMEDB_DATABASE_PATH=/data/animedb.sqlite \
  -v "$API_DB_DIR:/data" \
  "$IMAGE_TAG" >/dev/null

wait_for_health

echo "==> syncing real data through graphql api"
anilist_sync="$(graphql "{\"query\":\"mutation { syncDatabase(input: { source: ANILIST, mediaKind: ANIME, maxPages: ${MAX_PAGES}, pageSize: ${PAGE_SIZE} }) { totalUpsertedRecords outcomes { source upsertedRecords fetchedPages } } }\"}")"
assert_contains "$anilist_sync" "\"ANILIST\""
assert_contains "$anilist_sync" "\"totalUpsertedRecords\""

jikan_sync="$(graphql "{\"query\":\"mutation { syncDatabase(input: { source: JIKAN, mediaKind: ANIME, maxPages: ${MAX_PAGES}, pageSize: ${PAGE_SIZE} }) { totalUpsertedRecords outcomes { source upsertedRecords fetchedPages } } }\"}")"
assert_contains "$jikan_sync" "\"JIKAN\""
assert_contains "$jikan_sync" "\"totalUpsertedRecords\""

kitsu_sync="$(graphql "{\"query\":\"mutation { syncDatabase(input: { source: KITSU, mediaKind: ANIME, maxPages: ${MAX_PAGES}, pageSize: ${PAGE_SIZE} }) { totalUpsertedRecords outcomes { source upsertedRecords fetchedPages } } }\"}")"
assert_contains "$kitsu_sync" "\"KITSU\""
assert_contains "$kitsu_sync" "\"totalUpsertedRecords\""

echo "==> querying local graphql data"
media_lookup="$(graphql '{"query":"query { mediaByExternalId(source: ANILIST, sourceId: \"1\") { id name titleDisplay externalIds { source sourceId } } }"}')"
assert_contains "$media_lookup" "\"titleDisplay\""
assert_contains "$media_lookup" "\"ANILIST\""

echo "==> querying remote graphql data"
remote_lookup="$(graphql '{"query":"query { remoteSearch(source: ANILIST, query: \"monster\", options: { mediaKind: ANIME, limit: 1 }) { titleDisplay } }"}')"
assert_contains "$remote_lookup" "\"titleDisplay\""

remote_kitsu_lookup="$(graphql '{"query":"query { remoteSearch(source: KITSU, query: \"monster\", options: { mediaKind: ANIME, limit: 1 }) { titleDisplay nsfw } }"}')"
assert_contains "$remote_kitsu_lookup" "\"titleDisplay\""

echo "==> real pipeline ok"
