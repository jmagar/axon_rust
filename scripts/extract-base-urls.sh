#!/bin/bash
#
# extract-base-urls.sh
# Extract all unique base URLs from Qdrant vector database
#
# Usage:
#   ./scripts/extract-base-urls.sh [OPTIONS]
#
# Options:
#   -o, --output FILE    Output file path (default: .cache/indexed-base-urls.txt)
#   -u, --qdrant-url URL Qdrant URL (default: http://localhost:53333)
#   -c, --collection NAME Collection name (default: axon or $QDRANT_COLLECTION)
#   -b, --batch-size N   Batch size for scrolling (default: 10000)
#   -q, --quiet          Suppress progress messages
#   -h, --help           Show this help message
#
# Examples:
#   ./scripts/extract-base-urls.sh
#   ./scripts/extract-base-urls.sh -o urls.txt -q
#   ./scripts/extract-base-urls.sh --qdrant-url http://remote:6333

set -euo pipefail

# Default configuration
QDRANT_URL="${QDRANT_URL:-http://localhost:53333}"
COLLECTION="${QDRANT_COLLECTION:-axon}"
BATCH_SIZE=10000
OUTPUT_FILE=".cache/indexed-base-urls.txt"
QUIET=false

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Disable ANSI colors when not writing to a terminal.
if [ ! -t 2 ]; then
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    NC=''
fi

# Helper functions
log_info() {
    if [ "$QUIET" = false ]; then
        echo -e "${BLUE}ℹ${NC} $1" >&2
    fi
}

log_success() {
    echo -e "${GREEN}✓${NC} $1" >&2
}

log_error() {
    echo -e "${RED}✗${NC} $1" >&2
}

log_progress() {
    if [ "$QUIET" = false ]; then
        echo -e "${YELLOW}⋯${NC} $1" >&2
    fi
}

show_help() {
    sed -n '2,/^$/p' "$0" | sed 's/^# //' | sed 's/^#//'
    exit 0
}

# Parse command-line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -o|--output)
            OUTPUT_FILE="$2"
            shift 2
            ;;
        -u|--qdrant-url)
            QDRANT_URL="$2"
            shift 2
            ;;
        -c|--collection)
            COLLECTION="$2"
            shift 2
            ;;
        -b|--batch-size)
            BATCH_SIZE="$2"
            shift 2
            ;;
        -q|--quiet)
            QUIET=true
            shift
            ;;
        -h|--help)
            show_help
            ;;
        *)
            log_error "Unknown option: $1"
            echo "Use --help for usage information" >&2
            exit 1
            ;;
    esac
done

# Validate dependencies
if ! command -v curl &> /dev/null; then
    log_error "curl is not installed"
    exit 1
fi

if ! command -v jq &> /dev/null; then
    log_error "jq is not installed"
    exit 1
fi

# Create output directory if needed
output_dir=$(dirname "$OUTPUT_FILE")
if [ ! -d "$output_dir" ]; then
    mkdir -p "$output_dir"
    log_info "Created output directory: $output_dir"
fi

# Check Qdrant connection
log_info "Connecting to Qdrant at $QDRANT_URL..."
if ! collection_info=$(curl -s --max-time 30 "$QDRANT_URL/collections/$COLLECTION" 2>/dev/null); then
    log_error "Failed to connect to Qdrant at $QDRANT_URL"
    exit 1
fi

points_count=$(echo "$collection_info" | jq -r '.result.points_count // 0')
if [ "$points_count" -eq 0 ]; then
    log_error "Collection '$COLLECTION' is empty or does not exist"
    exit 1
fi

log_info "Found collection '$COLLECTION' with $points_count points"

# Create temporary file for all URLs
temp_file=$(mktemp)
trap "rm -f \"$temp_file\"" EXIT

# Scroll through all points
offset=null
batch_count=0
total_processed=0

log_info "Extracting base URLs..."

while true; do
    # Construct request
    if [ "$offset" = "null" ]; then
        request_body="{\"limit\": $BATCH_SIZE, \"with_payload\": true, \"with_vector\": false}"
    else
        request_body="{\"limit\": $BATCH_SIZE, \"offset\": \"$offset\", \"with_payload\": true, \"with_vector\": false}"
    fi

    # Fetch batch
    response=$(curl -s --max-time 30 -X POST "$QDRANT_URL/collections/$COLLECTION/points/scroll" \
        -H "Content-Type: application/json" \
        -d "$request_body")

    # Extract URLs and convert to base URLs
    echo "$response" | jq -r '.result.points[].payload.url' | \
        sed 's|^\([^:]*://[^/]*\).*|\1|' >> "$temp_file"

    batch_count=$((batch_count + 1))
    batch_size=$(echo "$response" | jq -r '.result.points | length')
    total_processed=$((total_processed + batch_size))

    # Get next offset
    offset=$(echo "$response" | jq -r '.result.next_page_offset')

    # Progress update
    if [ "$points_count" -gt 0 ]; then
        percentage=$((total_processed * 100 / points_count))
    else
        percentage=0
    fi
    log_progress "Processed $total_processed/$points_count points ($percentage%) - batch #$batch_count"

    # Break if no more pages
    if [ "$offset" = "null" ]; then
        break
    fi
done

# Count pages per base URL and sort by count
log_info "Counting pages per base URL..."
sort "$temp_file" | uniq -c | sort -rn | awk '{printf "%8d  %s\n", $1, $2}' > "$OUTPUT_FILE"

# Calculate statistics
unique_count=$(wc -l < "$OUTPUT_FILE")
total_pages=$(awk '{sum+=$1} END {print sum}' "$OUTPUT_FILE")
avg_pages=0
if [ "$unique_count" -gt 0 ]; then
    avg_pages=$((total_pages / unique_count))
fi

# Output results
log_success "Extraction complete!"
echo ""
echo "Statistics:"
echo "  Total pages indexed: $total_pages"
echo "  Unique base URLs: $unique_count"
echo "  Average pages per domain: $avg_pages"
echo "  Output file: $OUTPUT_FILE"
echo ""
echo "Top 10 domains by page count:"
head -10 "$OUTPUT_FILE"
if [ $unique_count -gt 10 ]; then
    echo "  ... and $((unique_count - 10)) more"
fi
