#!/bin/bash
# Script Name: crawl-site.sh
# Purpose: Crawl entire website with Axon
# Usage: ./crawl-site.sh <url> [limit] [max-depth]

set -euo pipefail

# === Functions ===

usage() {
    cat <<EOF
Usage: $0 <url> [limit] [max-depth]

Crawl entire website with progress tracking.

Arguments:
    url             Website URL to crawl
    limit           Optional: Maximum pages to crawl (omit for unlimited)
    max-depth       Optional: Maximum crawl depth (omit for unlimited)

Options:
    --help          Show this help message

Examples:
    $0 https://example.com              # No limits
    $0 https://example.com 50           # Max 50 pages, unlimited depth
    $0 https://example.com 100 5        # Max 100 pages, depth 5

Environment Variables:
    FIRECRAWL_API_KEY    API key for Firecrawl cloud API
    FIRECRAWL_API_URL    Custom API endpoint (optional)

Output:
    Crawl results with progress indicator (synchronous mode)

EOF
}

# Check for --help/-h before loading .env
for arg in "$@"; do
    if [[ "$arg" == "--help" || "$arg" == "-h" ]]; then
        usage
        exit 0
    fi
done

# Load environment variables from .env
ENV_FILE="${ENV_FILE:-$HOME/.claude-homelab/.env}"
if [[ -f "$ENV_FILE" ]]; then
    # Source .env file and export variables
    set -a
    source "$ENV_FILE"
    set +a
else
    echo "ERROR: .env file not found at $ENV_FILE" >&2
    exit 1
fi

# === Main Script ===

main() {
    # Validate arguments
    if [[ $# -lt 1 ]]; then
        echo "ERROR: URL required" >&2
        usage
        exit 1
    fi

    local url="$1"
    local limit="${2:-}"
    local max_depth="${3:-}"

    # Validate URL format
    if [[ ! "$url" =~ ^https?:// ]]; then
        echo "ERROR: URL must start with http:// or https://" >&2
        exit 1
    fi

    # Validate limit is a positive number if provided
    if [[ -n "$limit" ]] && ! [[ "$limit" =~ ^[1-9][0-9]*$ ]]; then
        echo "ERROR: Limit must be a positive number (>0)" >&2
        exit 1
    fi

    # Validate max_depth is a positive number if provided
    if [[ -n "$max_depth" ]] && ! [[ "$max_depth" =~ ^[1-9][0-9]*$ ]]; then
        echo "ERROR: Max depth must be a positive number (>0)" >&2
        exit 1
    fi

    # Build firecrawl command
    local -a cmd=(
        axon crawl "$url"
        --wait
        --progress
    )

    # Add limit only if provided
    if [[ -n "$limit" ]]; then
        cmd+=(--limit "$limit")
    fi

    # Add max-depth only if provided
    if [[ -n "$max_depth" ]]; then
        cmd+=(--max-depth "$max_depth")
    fi

    # Add custom API URL if set (self-hosted)
    if [[ -n "${FIRECRAWL_API_URL:-}" ]]; then
        cmd+=(--api-url "$FIRECRAWL_API_URL")
    fi

    # Execute command
    echo "Crawling: $url" >&2
    if [[ -n "$limit" ]] || [[ -n "$max_depth" ]]; then
        echo "Limit: ${limit:-unlimited} pages, Max depth: ${max_depth:-unlimited}" >&2
    else
        echo "No limits imposed" >&2
    fi
    echo "" >&2
    # Pass API key via environment variable to avoid exposure in process list
    FIRECRAWL_API_KEY="${FIRECRAWL_API_KEY:-}" "${cmd[@]}"
}

main "$@"
