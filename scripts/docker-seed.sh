#!/bin/sh

set -eu

ADMIN_DSN="${ADMIN_DSN:-postgres://root:root@host.docker.internal:5432/root?sslmode=disable}"
ANILIST_DB="${ANILIST_DB:-anilist}"
MYANIMELIST_DB="${MYANIMELIST_DB:-myanimelist}"

ANILIST_SORT="${ANILIST_SORT:-ID}"
ENABLE_ANILIST_SEED="${ENABLE_ANILIST_SEED:-true}"
ENABLE_MYANIMELIST_SEED="${ENABLE_MYANIMELIST_SEED:-true}"
FORCE_SEED="${FORCE_SEED:-false}"

SKIP_FLAG=""
if [ "$FORCE_SEED" = "false" ]; then
  SKIP_FLAG="--skip-if-seeded"
fi

log() {
  printf '[%s] %s\n' "$(date +'%Y-%m-%dT%H:%M:%S%z')" "$*"
}

log "Configuration: ANILIST_SORT=${ANILIST_SORT}, ENABLE_ANILIST_SEED=${ENABLE_ANILIST_SEED}, ENABLE_MYANIMELIST_SEED=${ENABLE_MYANIMELIST_SEED}, FORCE_SEED=${FORCE_SEED}"

log "Starting database seed pipeline"

if [ "$ENABLE_ANILIST_SEED" = "true" ]; then
  log "Provisioning AniList database (${ANILIST_DB}) with sort ${ANILIST_SORT}"
  go run ./cmd/anilist --dsn "${ADMIN_DSN}" --database "${ANILIST_DB}" --sort "${ANILIST_SORT}" $SKIP_FLAG
else
  log "Skipping AniList database provisioning (ENABLE_ANILIST_SEED=false)"
fi

if [ "$ENABLE_MYANIMELIST_SEED" = "true" ]; then
  log "Provisioning MyAnimeList database (${MYANIMELIST_DB})"
  go run ./cmd/myanimelist --dsn "${ADMIN_DSN}" --database "${MYANIMELIST_DB}" $SKIP_FLAG
else
  log "Skipping MyAnimeList database provisioning (ENABLE_MYANIMELIST_SEED=false)"
fi

log "Database seed pipeline finished"
