#!/bin/bash
# Script Name: scrape.sh
# Purpose: Scrape a single URL with Axon
# Usage: ./scrape.sh <url> [output-file] [-- <extra-axon-flags...>]

set -euo pipefail

# Load environment variables from .env
ENV_FILE="${ENV_FILE:-$HOME/.claude-homelab/.env}"
if [[ -f "$ENV_FILE" ]]; then
    set -a
    source "$ENV_FILE"
    set +a
else
    echo "ERROR: .env file not found at $ENV_FILE" >&2
    exit 1
fi

# === Functions ===

usage() {
    cat <<EOF
Usage: $0 <url> [output-file] [-- <extra-axon-flags...>]

Scrape a single URL and extract main content in markdown format.

Arguments:
    url             URL to scrape
    output-file     Optional: Save output to file instead of stdout

Options:
    --help          Show this help message

Examples:
    $0 https://example.com
    $0 https://example.com output.md
    $0 https://example.com --format markdown,html
    $0 https://example.com output.md -- --timeout 30

Environment Variables:
    FIRECRAWL_API_KEY    API key for Firecrawl cloud API
    FIRECRAWL_API_URL    Custom API endpoint (optional)

EOF
}

# === Main Script ===

main() {
    # Check for help flag
    if [[ "${1:-}" == "--help" ]]; then
        usage
        exit 0
    fi

    # Validate arguments
    if [[ $# -lt 1 ]]; then
        echo "ERROR: URL required" >&2
        usage
        exit 1
    fi

    local url="$1"
    shift
    local output_file=""
    if [[ $# -gt 0 ]] && [[ "${1:-}" != -* ]]; then
        output_file="$1"
        shift
    fi
    # Strip leading "--" separator so it is not forwarded as an argument
    if [[ "${1:-}" == "--" ]]; then
        shift
    fi
    local -a passthrough_args=("$@")

    # Validate URL format
    if [[ ! "$url" =~ ^https?:// ]]; then
        echo "ERROR: URL must start with http:// or https://" >&2
        exit 1
    fi

    # Build firecrawl command
    local -a cmd=(axon scrape "$url" --only-main-content)

    # Add custom API URL if set (self-hosted)
    if [[ -n "${FIRECRAWL_API_URL:-}" ]]; then
        cmd+=(--api-url "$FIRECRAWL_API_URL")
    fi

    # Add output file if provided
    if [[ -n "$output_file" ]]; then
        cmd+=(-o "$output_file")
    fi

    # Forward any additional Axon scrape flags.
    if [[ ${#passthrough_args[@]} -gt 0 ]]; then
        cmd+=("${passthrough_args[@]}")
    fi

    # Execute command (API key passed via env to avoid process list exposure)
    FIRECRAWL_API_KEY="${FIRECRAWL_API_KEY:-}" "${cmd[@]}"
}

main "$@"
