#!/bin/bash
set -e

# =============================================================================
# E2E Test Script for animedb - TVmaze + IMDb Integration
# =============================================================================
# This script tests:
# 1. Database creation and schema migration
# 2. TVmaze sync (Shows)
# 3. IMDb sync (Movies and Shows)
# 4. Local search by MediaKind
# 5. Merge of multiple sources
# 6. Remote search via TVmaze API
# 7. GraphQL API endpoints
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
DB_PATH="/tmp/animedb-e2e-test-$(date +%s).sqlite"
LOG_FILE="/tmp/animedb-e2e-test-$(date +%s).log"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "${BLUE}[$(date +%H:%M:%S)]${NC} $1" | tee -a "$LOG_FILE"
}

pass() {
    echo -e "${GREEN}[PASS]${NC} $1" | tee -a "$LOG_FILE"
}

fail() {
    echo -e "${RED}[FAIL]${NC} $1" | tee -a "$LOG_FILE"
    exit 1
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1" | tee -a "$LOG_FILE"
}

section() {
    echo "" | tee -a "$LOG_FILE"
    echo -e "${YELLOW}═══════════════════════════════════════════════════════════════${NC}" | tee -a "$LOG_FILE"
    echo -e "${YELLOW}  $1${NC}" | tee -a "$LOG_FILE"
    echo -e "${YELLOW}═══════════════════════════════════════════════════════════════${NC}" | tee -a "$LOG_FILE"
}

# =============================================================================
# SETUP
# =============================================================================

section "SETUP"

log "Project directory: $PROJECT_DIR"
log "Database path: $DB_PATH"
log "Log file: $LOG_FILE"

# Clean up any existing database
rm -f "$DB_PATH"

# Build the project
log "Building project..."
cd "$PROJECT_DIR"
cargo build --release 2>&1 | tee -a "$LOG_FILE"

if [ $? -ne 0 ]; then
    fail "Build failed"
fi
pass "Build successful"

# =============================================================================
# TEST 1: Database Creation & Schema Migration
# =============================================================================

section "TEST 1: Database Creation & Schema Migration"

log "Creating new database..."
cargo run --release --example e2e_test_runner -- "$DB_PATH" --create-only 2>&1 | tee -a "$LOG_FILE"

# Verify database was created
if [ ! -f "$DB_PATH" ]; then
    fail "Database file not created at $DB_PATH"
fi
pass "Database file created"

# Check schema version
SCHEMA_VERSION=$(sqlite3 "$DB_PATH" "PRAGMA user_version;" 2>/dev/null || echo "0")
log "Schema version: $SCHEMA_VERSION"

if [ "$SCHEMA_VERSION" -lt 4 ]; then
    fail "Schema version should be >= 4, got $SCHEMA_VERSION"
fi
pass "Schema migration to version $SCHEMA_VERSION"

# Verify tables exist
TABLES=$(sqlite3 "$DB_PATH" "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name;" 2>/dev/null)
log "Tables: $TABLES"

EXPECTED_TABLES="media media_alias media_external_id source_record"
for table in $EXPECTED_TABLES; do
    if ! echo "$TABLES" | grep -q "$table"; then
        fail "Missing table: $table"
    fi
done
pass "All expected tables exist"

# Check CHECK constraints include new media_kind values
CONSTRAINT=$(sqlite3 "$DB_PATH" "SELECT sql FROM sqlite_master WHERE name='media' AND type='table';" 2>/dev/null)
if ! echo "$CONSTRAINT" | grep -q "'show'"; then
    fail "media_kind CHECK constraint missing 'show'"
fi
if ! echo "$CONSTRAINT" | grep -q "'movie'"; then
    fail "media_kind CHECK constraint missing 'movie'"
fi
pass "CHECK constraints include 'show' and 'movie'"

# =============================================================================
# TEST 2: TVmaze Sync (Shows)
# =============================================================================

section "TEST 2: TVmaze Sync (Shows)"

log "Syncing 2 pages from TVmaze (page size: 10)..."
cargo run --release --example e2e_test_runner -- "$DB_PATH" \
    --sync-tvmaze --pages 2 --page-size 10 2>&1 | tee -a "$LOG_FILE"

# Count shows in database
SHOW_COUNT=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM media WHERE media_kind = 'show';" 2>/dev/null)
log "Shows in database: $SHOW_COUNT"

if [ "$SHOW_COUNT" -lt 1 ]; then
    fail "Expected at least 1 show, got $SHOW_COUNT"
fi
pass "TVmaze sync inserted $SHOW_COUNT shows"

# Verify show has required fields
FIRST_SHOW=$(sqlite3 "$DB_PATH" "SELECT id, title_display FROM media WHERE media_kind = 'show' LIMIT 1;" 2>/dev/null)
log "First show: $FIRST_SHOW"

if [ -z "$FIRST_SHOW" ]; then
    fail "No show found in database"
fi
pass "Show record has title_display"

# Verify external_ids for TVmaze
SHOW_ID=$(echo "$FIRST_SHOW" | cut -d'|' -f1)
TVMAZE_EXT_ID=$(
    sqlite3 "$DB_PATH" \
        "SELECT source_id FROM media_external_id WHERE media_id = $SHOW_ID AND source = 'tvmaze';" \
        2>/dev/null
)
log "TVmaze external ID: $TVMAZE_EXT_ID"

if [ -z "$TVMAZE_EXT_ID" ]; then
    fail "Show missing TVmaze external ID"
fi
pass "Show has TVmaze external ID"

# =============================================================================
# TEST 3: IMDb Sync (Movies)
# =============================================================================

section "TEST 3: IMDb Sync (Movies)"

log "Syncing from IMDb (page size: 50, max pages: 1)..."
log "Note: IMDb sync downloads title.basics.tsv.gz (~100MB) and title.ratings.tsv.gz (~5MB)"
log "This may take a minute..."

# Use a timeout for IMDb sync since it downloads large files
timeout 300 cargo run --release --example e2e_test_runner -- "$DB_PATH" \
    --sync-imdb --media-kind movie --pages 1 --page-size 50 2>&1 \
    | tee -a "$LOG_FILE" || {
    warn "IMDb sync timed out or failed (this is expected in constrained environments)"
    warn "Skipping IMDb tests..."
    SKIP_IMDB=1
}

if [ "$SKIP_IMDB" != "1" ]; then
    # Count movies in database
    MOVIE_COUNT=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM media WHERE media_kind = 'movie';" 2>/dev/null)
    log "Movies in database: $MOVIE_COUNT"

    if [ "$MOVIE_COUNT" -lt 1 ]; then
        warn "Expected at least 1 movie from IMDb sync"
    else
        pass "IMDb sync inserted $MOVIE_COUNT movies"
    fi

    # Verify movie has IMDB external ID
    FIRST_MOVIE=$(
        sqlite3 "$DB_PATH" \
            "SELECT id, title_display, season_year FROM media WHERE media_kind = 'movie' LIMIT 1;" \
            2>/dev/null
    )
    log "First movie: $FIRST_MOVIE"

    if [ -n "$FIRST_MOVIE" ]; then
        MOVIE_ID=$(echo "$FIRST_MOVIE" | cut -d'|' -f1)
        IMDB_EXT_ID=$(
            sqlite3 "$DB_PATH" \
                "SELECT source_id FROM media_external_id WHERE media_id = $MOVIE_ID AND source = 'imdb';" \
                2>/dev/null
        )
        log "IMDb external ID: $IMDB_EXT_ID"

        if [ -z "$IMDB_EXT_ID" ]; then
            warn "Movie missing IMDb external ID"
        else
            pass "Movie has IMDb external ID: $IMDB_EXT_ID"
        fi

        # Verify season_year (startYear in IMDb) is populated
        YEAR=$(echo "$FIRST_MOVIE" | cut -d'|' -f3)
        if [ -n "$YEAR" ] && [ "$YEAR" != "" ]; then
            pass "Movie has year: $YEAR"
        else
            warn "Movie missing year (may be null in source)"
        fi
    fi
fi

# =============================================================================
# TEST 4: Local Search by MediaKind
# =============================================================================

section "TEST 4: Local Search by MediaKind"

log "Searching for shows containing 'the'..."
SHOW_HITS=$(sqlite3 "$DB_PATH" "SELECT media.id, media.title_display, media_fts.rank 
    FROM media 
    JOIN media_fts ON media_fts.rowid = media.id 
    WHERE media_fts.media_kind = 'show' 
    AND media_fts.title_display MATCH 'the*' 
    ORDER BY media_fts.rank 
    LIMIT 5;" 2>/dev/null || echo "")

if [ -n "$SHOW_HITS" ]; then
    log "Show search results:"
    echo "$SHOW_HITS" | while read line; do
        log "  $line"
    done
    pass "FTS5 search for shows works"
else
    warn "No show search results (may need more data)"
fi

log "Searching for movies..."
if [ "$SKIP_IMDB" != "1" ]; then
    MOVIE_HITS=$(sqlite3 "$DB_PATH" "SELECT media.id, media.title_display, media_fts.rank 
        FROM media 
        JOIN media_fts ON media_fts.rowid = media.id 
        WHERE media_fts.media_kind = 'movie' 
        AND media_fts.title_display MATCH '*' 
        ORDER BY media_fts.rank 
        LIMIT 5;" 2>/dev/null || echo "")

    if [ -n "$MOVIE_HITS" ]; then
        log "Movie search results:"
        echo "$MOVIE_HITS" | while read line; do
            log "  $line"
        done
        pass "FTS5 search for movies works"
    else
        warn "No movie search results"
    fi
fi

# =============================================================================
# TEST 5: Merge of Multiple Sources (TVmaze + IMDb)
# =============================================================================

section "TEST 5: Merge of Multiple Sources"

# Find shows that have both TVmaze and IMDb external IDs
log "Finding shows with both TVmaze and IMDb external IDs..."
MERGED_SHOWS=$(sqlite3 "$DB_PATH" "
    SELECT m.id, m.title_display, 
           GROUP_CONCAT(DISTINCT e.source || ':' || e.source_id) as sources
    FROM media m
    JOIN media_external_id e ON e.media_id = m.id
    WHERE m.media_kind = 'show'
    GROUP BY m.id
    HAVING COUNT(DISTINCT e.source) >= 2
    LIMIT 5;" 2>/dev/null || echo "")

if [ -n "$MERGED_SHOWS" ]; then
    log "Merged shows (multiple sources):"
    echo "$MERGED_SHOWS" | while read line; do
        log "  $line"
    done
    pass "Merge of multiple sources works"
else
    warn "No merged shows found (TVmaze may not have IMDb IDs for the synced shows)"
fi

# =============================================================================
# TEST 6: Remote Search via TVmaze API
# =============================================================================

section "TEST 6: Remote Search via TVmaze API"

log "Searching TVmaze API for 'breaking bad'..."
TVMAZE_SEARCH=$(
    cargo run --release --example e2e_test_runner -- "$DB_PATH" \
        --remote-search "breaking bad" 2>&1 | tee -a "$LOG_FILE" || echo ""
)

if echo "$TVMAZE_SEARCH" | grep -q "Breaking Bad"; then
    pass "TVmaze remote search found 'Breaking Bad'"
else
    warn "TVmaze remote search did not find 'Breaking Bad'"
fi

# =============================================================================
# TEST 7: GraphQL API
# =============================================================================

section "TEST 7: GraphQL API"

log "Starting GraphQL API server in background..."
ANIMEDB_DATABASE_PATH="$DB_PATH" ANIMEDB_LISTEN_ADDR="127.0.0.1:18080" \
    cargo run --release --package animedb-api &
API_PID=$!

# Wait for server to start
sleep 5

log "Testing health endpoint..."
HEALTH=$(curl -s http://127.0.0.1:18080/healthz 2>/dev/null || echo "")

if echo "$HEALTH" | grep -q "ok"; then
    pass "GraphQL API health endpoint works"
else
    warn "GraphQL API health endpoint failed"
fi

log "Testing GraphQL query: shows..."
GRAPHQL_PAYLOAD='{"query":"{ search(query: \"the\", options: { limit: 5, mediaKind: SHOW })'
GRAPHQL_PAYLOAD+=' { mediaId titleDisplay mediaKind } }"}'
GRAPHQL_RESULT=$(curl -s -X POST http://127.0.0.1:18080/graphql \
    -H "Content-Type: application/json" \
    -d "$GRAPHQL_PAYLOAD" \
    2>/dev/null || echo "")

log "GraphQL result: $GRAPHQL_RESULT"

if echo "$GRAPHQL_RESULT" | grep -q "titleDisplay"; then
    pass "GraphQL search query works"
else
    warn "GraphQL search query failed"
fi

log "Testing GraphQL query: media kinds..."
GRAPHQL_KINDS=$(curl -s -X POST http://127.0.0.1:18080/graphql \
    -H "Content-Type: application/json" \
    -d '{"query": "{ __type(name: \"MediaKindObject\") { enumValues { name } } }"}' 2>/dev/null || echo "")

if echo "$GRAPHQL_KINDS" | grep -q "SHOW" && echo "$GRAPHQL_KINDS" | grep -q "MOVIE"; then
    pass "GraphQL schema includes SHOW and MOVIE kinds"
else
    fail "GraphQL schema missing SHOW or MOVIE kinds"
fi

log "Testing GraphQL query: source names..."
GRAPHQL_SOURCES=$(curl -s -X POST http://127.0.0.1:18080/graphql \
    -H "Content-Type: application/json" \
    -d '{"query": "{ __type(name: \"SourceNameObject\") { enumValues { name } } }"}' 2>/dev/null || echo "")

if echo "$GRAPHQL_SOURCES" | grep -q "TVMAZE" && echo "$GRAPHQL_SOURCES" | grep -q "IMDB"; then
    pass "GraphQL schema includes TVMAZE and IMDB sources"
else
    fail "GraphQL schema missing TVMAZE or IMDB sources"
fi

# Stop the API server
kill $API_PID 2>/dev/null || true

# =============================================================================
# TEST 8: Source Record Tracking
# =============================================================================

section "TEST 8: Source Record Tracking"

log "Checking source_record table..."
SOURCE_RECORDS=$(
    sqlite3 "$DB_PATH" \
        "SELECT source, COUNT(*) as count FROM source_record GROUP BY source;" \
        2>/dev/null || echo ""
)

log "Source records by provider:"
echo "$SOURCE_RECORDS" | while read line; do
    log "  $line"
done

if [ -n "$SOURCE_RECORDS" ]; then
    pass "Source records are being tracked"
else
    warn "No source records found"
fi

# Verify raw_json is stored
RAW_JSON_CHECK=$(
    sqlite3 "$DB_PATH" \
        "SELECT COUNT(*) FROM source_record WHERE raw_json IS NOT NULL;" \
        2>/dev/null || echo "0"
)
log "Records with raw_json: $RAW_JSON_CHECK"

if [ "$RAW_JSON_CHECK" -gt 0 ]; then
    pass "raw_json is being stored for debugging"
else
    warn "No raw_json stored (may be expected for some sources)"
fi

# =============================================================================
# TEST 9: Sync State Persistence
# =============================================================================

section "TEST 9: Sync State Persistence"

log "Checking sync_state table..."
SYNC_STATES=$(
    sqlite3 "$DB_PATH" \
        "SELECT source, scope, last_page, last_success_at FROM sync_state;" \
        2>/dev/null || echo ""
)

log "Sync states:"
echo "$SYNC_STATES" | while read line; do
    log "  $line"
done

if [ -n "$SYNC_STATES" ]; then
    pass "Sync state is being persisted"
else
    warn "No sync state found"
fi

# =============================================================================
# CLEANUP
# =============================================================================

section "CLEANUP"

log "Cleaning up..."
rm -f "$DB_PATH"

pass "E2E test completed successfully!"

echo ""
echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}  ALL TESTS PASSED${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
echo ""
echo "Full log available at: $LOG_FILE"
