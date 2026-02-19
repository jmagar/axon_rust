#!/bin/bash
# Example: Basic Web Scraping Workflow
# Purpose: Demonstrates simple web page scraping with Axon
# Use Case: Extracting documentation or blog content for analysis

set -euo pipefail

# ============================================================================
# CONFIGURATION
# ============================================================================

# Load credentials from .env file
if [[ -f ~/claude-homelab/.env ]]; then
    set -a
    source ~/claude-homelab/.env
    set +a
else
    echo "ERROR: .env file not found at ~/claude-homelab/.env" >&2
    echo "Please create .env with FIRECRAWL_API_KEY" >&2
    exit 1
fi

# Validate credentials for cloud mode.
# Self-hosted mode may legitimately run without an API key.
API_URL="${FIRECRAWL_API_URL:-https://api.firecrawl.dev}"
if [[ "$API_URL" == "https://api.firecrawl.dev" || "$API_URL" == "https://api.firecrawl.dev/" ]]; then
    if [[ -z "${FIRECRAWL_API_KEY:-}" ]]; then
        echo "ERROR: FIRECRAWL_API_KEY must be set when using Firecrawl cloud API" >&2
        exit 1
    fi
fi

# ============================================================================
# EXAMPLE 1: SCRAPE SINGLE PAGE (MARKDOWN FORMAT)
# ============================================================================

echo "=== Example 1: Basic Page Scraping ==="
echo "Scraping a single documentation page to markdown format..."
echo

# Target: Scrape a documentation page
# Output: Clean markdown suitable for LLM processing
# Options:
#   --only-main-content: Remove navigation, footers, ads
#   --format markdown: Return as clean markdown text
#   -o: Save to file instead of stdout

axon scrape "https://docs.firecrawl.dev/introduction" \
    --only-main-content \
    --format markdown \
    -o /tmp/axon-intro.md

echo "✅ Scraped content saved to: /tmp/axon-intro.md"
echo "Preview (first 10 lines):"
head -n 10 /tmp/axon-intro.md
echo

# ============================================================================
# EXAMPLE 2: SCRAPE WITH MULTIPLE FORMATS
# ============================================================================

echo "=== Example 2: Multiple Format Scraping ==="
echo "Scraping with both markdown and HTML for comparison..."
echo

# Target: Get both formats for analysis
# Output: JSON object with both markdown and html fields
# Use case: When you need to compare cleaned vs raw content

axon scrape "https://docs.firecrawl.dev/features/scrape" \
    --format markdown,html \
    --only-main-content \
    --pretty \
    -o /tmp/axon-multi-format.json

echo "✅ Multi-format content saved to: /tmp/axon-multi-format.json"
echo "JSON structure:"
jq 'keys' /tmp/axon-multi-format.json
echo

# ============================================================================
# EXAMPLE 3: SCRAPE WITH JAVASCRIPT WAIT
# ============================================================================

echo "=== Example 3: Scraping JavaScript-Heavy Sites ==="
echo "Scraping a page that requires JavaScript rendering..."
echo

# Target: Sites with dynamic content loaded by JavaScript
# Options:
#   --wait-for 3000: Wait 3 seconds for JavaScript to execute
#   --timeout 30: Set 30 second timeout for slow sites

axon scrape "https://docs.firecrawl.dev/features/crawl" \
    --wait-for 3000 \
    --timeout 30 \
    --only-main-content \
    --format markdown \
    -o /tmp/axon-js-page.md

echo "✅ JavaScript-rendered content saved to: /tmp/axon-js-page.md"
echo

# ============================================================================
# EXAMPLE 4: SCRAPE WITH LINK EXTRACTION
# ============================================================================

echo "=== Example 4: Extracting All Links ==="
echo "Scraping page and extracting all hyperlinks..."
echo

# Target: Extract all links for further crawling or analysis
# Output: Array of URLs found on the page
# Use case: Building a sitemap or discovering related content

axon scrape "https://docs.firecrawl.dev" \
    --format links \
    --pretty \
    -o /tmp/axon-links.json

echo "✅ Links extracted to: /tmp/axon-links.json"
echo "Total links found:"
jq '. | length' /tmp/axon-links.json
echo "First 5 links:"
jq '.[0:5]' /tmp/axon-links.json
echo

# ============================================================================
# EXAMPLE 5: SCRAPE WITH AUTO-EMBEDDING (RAG)
# ============================================================================

echo "=== Example 5: Scrape and Auto-Embed for RAG ==="
echo "Scraping content and automatically embedding into Qdrant..."
echo

# Target: Build RAG knowledge base automatically
# Output: Content scraped AND embedded as vectors in Qdrant
# Note: Requires Qdrant and TEI services running (see SKILL.md Setup)
# Default behavior: Auto-embedding is ENABLED unless --no-embed is used

# Check if Qdrant is configured
if [[ -n "${QDRANT_URL:-}" ]] && [[ -n "${TEI_URL:-}" ]]; then
    echo "Qdrant configured at: ${QDRANT_URL}"
    echo "Embedding will be automatic..."

    axon scrape "https://docs.firecrawl.dev/api-reference/introduction" \
        --only-main-content \
        --format markdown

    echo "✅ Content scraped and embedded into Qdrant collection: ${QDRANT_COLLECTION:-axon}"
    echo "You can now query this content with: axon query \"your question\""
else
    echo "⚠️ Qdrant not configured - skipping auto-embedding example"
    echo "To enable: Set QDRANT_URL and TEI_URL in .env (see SKILL.md Setup section)"
fi
echo

# ============================================================================
# EXAMPLE 6: DISABLE AUTO-EMBEDDING
# ============================================================================

echo "=== Example 6: Scrape Without Embedding ==="
echo "Sometimes you just want to scrape without building RAG index..."
echo

# Use case: Testing, data extraction, or when you don't need semantic search
# Option: --no-embed disables automatic embedding to Qdrant

axon scrape "https://docs.firecrawl.dev/changelog" \
    --only-main-content \
    --format markdown \
    --no-embed \
    -o /tmp/axon-changelog.md

echo "✅ Content scraped WITHOUT embedding"
echo "Saved to: /tmp/axon-changelog.md"
echo

# ============================================================================
# CLEANUP & NEXT STEPS
# ============================================================================

echo "=== Basic Scraping Examples Complete ==="
echo
echo "Generated files:"
ls -lh /tmp/axon-*.{md,json} 2>/dev/null || echo "  (some files may not exist if examples were skipped)"
echo
echo "Next steps:"
echo "  1. Try scraping your own URLs"
echo "  2. Experiment with different formats (markdown, html, links, screenshot)"
echo "  3. Check out examples/rag-pipeline.sh for building a complete RAG system"
echo "  4. See examples/batch-processing.sh for scraping multiple URLs at once"
echo
echo "For more options, run: axon scrape --help"
