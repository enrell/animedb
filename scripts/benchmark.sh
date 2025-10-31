#!/bin/bash

set -e

BASE_URL="${BASE_URL:-http://localhost:8081}"
NUM_REQUESTS="${NUM_REQUESTS:-10}"
VERBOSE="${VERBOSE:-false}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

check_server() {
    log_info "Checking if API is running at $BASE_URL..."
    if ! curl -s "$BASE_URL/healthz" > /dev/null; then
        log_error "API is not running at $BASE_URL"
        exit 1
    fi
    log_success "API is healthy"
}

benchmark_endpoint() {
    local name=$1
    local endpoint=$2
    local num_requests=$3
    
    echo ""
    log_info "Benchmarking: $name"
    log_info "Endpoint: $endpoint"
    log_info "Requests: $num_requests"
    
    local total_time=0
    local min_time=999999
    local max_time=0
    local times=()
    
    for ((i = 1; i <= num_requests; i++)); do
        response_time=$(curl -s -w "%{time_total}" -o /dev/null "$BASE_URL$endpoint")
        times+=("$response_time")
        total_time=$(echo "$total_time + $response_time" | bc)
        
        if (( $(echo "$response_time < $min_time" | bc -l) )); then
            min_time=$response_time
        fi
        if (( $(echo "$response_time > $max_time" | bc -l) )); then
            max_time=$response_time
        fi
        
        if [ "$VERBOSE" = "true" ]; then
            echo "  Request $i: ${response_time}s"
        else
            printf "."
        fi
    done
    
    echo ""
    
    local avg_time=$(echo "scale=4; $total_time / $num_requests" | bc)
    
    printf "  %-20s: %s\n" "Min" "${min_time}s"
    printf "  %-20s: %s\n" "Max" "${max_time}s"
    printf "  %-20s: %s\n" "Average" "${avg_time}s"
    
    local requests_per_sec=$(echo "scale=2; $num_requests / $total_time" | bc)
    printf "  %-20s: %s req/s\n" "Throughput" "$requests_per_sec"
    
    log_success "$name completed"
}

benchmark_search_variants() {
    echo ""
    log_info "Benchmarking search variants (5 requests each)"
    
    local queries=(
        "attack%20titan"
        "tensei%20slime"
        "demon%20slayer"
        "jujutsu%20kaisen"
        "one%20piece"
    )
    
    local limits=(5 10 20 50)
    
    for query in "${queries[@]}"; do
        for limit in "${limits[@]}"; do
            local endpoint="/anilist/media/search?search=$query&limit=$limit"
            local name="AniList search '$query' (limit=$limit)"
            benchmark_endpoint "$name" "$endpoint" 1
        done
    done
}

main() {
    echo ""
    echo -e "${BLUE}╔════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║     AnimeDB API Benchmark Suite        ║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════╝${NC}"
    echo ""
    
    check_server
    
    echo ""
    log_info "Starting benchmarks with $NUM_REQUESTS requests per endpoint"
    
    benchmark_endpoint "Health Check" "/healthz" "$NUM_REQUESTS"
    
    benchmark_endpoint "AniList Search (basic)" "/anilist/media/search?search=slime&limit=10" "$NUM_REQUESTS"
    
    benchmark_endpoint "AniList Search (max limit)" "/anilist/media/search?search=slime&limit=50" "$NUM_REQUESTS"
    
    benchmark_endpoint "AniList List (page 1, size 20)" "/anilist/media?page=1&page_size=20" "$NUM_REQUESTS"
    
    benchmark_endpoint "AniList List (page 1, size 500)" "/anilist/media?page=1&page_size=500" "$NUM_REQUESTS"
    
    benchmark_endpoint "MyAnimeList Search (basic)" "/myanimelist/anime/search?search=slime&limit=10" "$NUM_REQUESTS"
    
    benchmark_endpoint "MyAnimeList Search (max limit)" "/myanimelist/anime/search?search=slime&limit=50" "$NUM_REQUESTS"
    
    benchmark_endpoint "MyAnimeList List (page 1, size 20)" "/myanimelist/anime?page=1&page_size=20" "$NUM_REQUESTS"
    
    benchmark_endpoint "MyAnimeList List (page 1, size 500)" "/myanimelist/anime?page=1&page_size=500" "$NUM_REQUESTS"
    
    benchmark_endpoint "Realtime Search (both sources)" "/search/realtime?q=slime&source=both&limit=10" "$NUM_REQUESTS"
    
    benchmark_endpoint "Realtime Search (AniList only)" "/search/realtime?q=slime&source=anilist&limit=10" "$NUM_REQUESTS"
    
    benchmark_endpoint "Realtime Search (MAL only)" "/search/realtime?q=slime&source=myanimelist&limit=10" "$NUM_REQUESTS"
    
    echo ""
    log_info "Running search variant tests..."
    benchmark_search_variants
    
    echo ""
    echo -e "${BLUE}╔════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║        Benchmark Complete!            ║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════╝${NC}"
    echo ""
}

main
