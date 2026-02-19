#!/bin/bash
# Script Name: map-site.sh
# Purpose: Map all URLs on a website with Axon
# Usage: ./map-site.sh <url> [output-file] [-- <extra-axon-flags...>]

set -euo pipefail

# Load environment variables from .env
ENV_FILE="$HOME/claude-homelab/.env"
if [[ -f "$ENV_FILE" ]]; then
    # Source .env file and export variables
    set -a
    # shellcheck source=/dev/null
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

Map all URLs on a website without scraping content.

Arguments:
    url             Website URL to map
    output-file     Optional: Save output to file (JSON format)

Options:
    --help          Show this help message

Examples:
    $0 https://example.com
    $0 https://example.com sitemap.json
    $0 https://example.com --limit 1000
    $0 https://example.com sitemap.json -- --limit 1000

Environment Variables:
    FIRECRAWL_API_KEY    API key for Firecrawl cloud API
    FIRECRAWL_API_URL    Custom API endpoint (optional)

Output:
    JSON array of discovered URLs

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
    local -a passthrough_args=("$@")

    # Validate URL format
    if [[ ! "$url" =~ ^https?:// ]]; then
        echo "ERROR: URL must start with http:// or https://" >&2
        exit 1
    fi

    # Build firecrawl command
    local -a cmd=(axon map "$url" --json)

    # Add API key if set (cloud API)
    if [[ -n "${FIRECRAWL_API_KEY:-}" ]]; then
        cmd+=(--api-key "$FIRECRAWL_API_KEY")
    fi

    # Add custom API URL if set (self-hosted)
    if [[ -n "${FIRECRAWL_API_URL:-}" ]]; then
        cmd+=(--api-url "$FIRECRAWL_API_URL")
    fi

    # Add output file if provided
    if [[ -n "$output_file" ]]; then
        cmd+=(-o "$output_file")
        echo "Mapping URLs from: $url" >&2
        echo "Output: $output_file" >&2
    else
        echo "Mapping URLs from: $url" >&2
    fi

    # Forward any additional Axon map flags.
    if [[ ${#passthrough_args[@]} -gt 0 ]]; then
        cmd+=("${passthrough_args[@]}")
    fi

    # Execute command
    "${cmd[@]}"

    # Show success message if output file specified
    if [[ -n "$output_file" ]]; then
        echo "Mapping complete: $output_file" >&2
    fi
}

main "$@"
