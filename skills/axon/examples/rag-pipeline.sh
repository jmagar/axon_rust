#!/bin/bash
# Example: Complete RAG Pipeline with Axon
# Purpose: Build a production-ready RAG system with semantic search
# Use Case: Create searchable knowledge base from documentation sites

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

# Validate RAG infrastructure is available
if [[ -z "${QDRANT_URL:-}" ]] || [[ -z "${TEI_URL:-}" ]]; then
    echo "ERROR: RAG infrastructure not configured" >&2
    echo "Required .env variables:" >&2
    echo "  - QDRANT_URL (Vector database)" >&2
    echo "  - TEI_URL (Text embeddings inference)" >&2
    echo "  - QDRANT_COLLECTION (optional, defaults to 'axon')" >&2
    echo >&2
    echo "See skills/axon/SKILL.md Setup section for details" >&2
    exit 1
fi

# Configuration
COLLECTION="${QDRANT_COLLECTION:-axon}"
TARGET_SITE="https://docs.firecrawl.dev"
OUTPUT_DIR="/tmp/axon-rag-pipeline"

# Create output directory
mkdir -p "$OUTPUT_DIR"

echo "=== Axon RAG Pipeline Example ==="
echo "Collection: $COLLECTION"
echo "Target Site: $TARGET_SITE"
echo "Output Directory: $OUTPUT_DIR"
echo

# ============================================================================
# PHASE 1: DISCOVER CONTENT (MAP)
# ============================================================================

echo "=== Phase 1: Discovering Site Structure ==="
echo "Mapping all URLs on the documentation site..."
echo

# Map the site to discover all available pages
# This is fast and doesn't scrape content - just finds URLs
# Output: List of all URLs that can be crawled

axon map "$TARGET_SITE" \
    --json \
    --pretty \
    -o "$OUTPUT_DIR/sitemap.json"

# Count discovered pages
TOTAL_PAGES=$(jq '. | length' "$OUTPUT_DIR/sitemap.json")
echo "✅ Discovered $TOTAL_PAGES pages"
echo "Sitemap saved to: $OUTPUT_DIR/sitemap.json"
echo

# Show sample of discovered URLs
echo "Sample URLs (first 5):"
jq -r '.[0:5][]' "$OUTPUT_DIR/sitemap.json"
echo

# ============================================================================
# PHASE 2: SCRAPE AND EMBED CONTENT (CRAWL WITH AUTO-EMBED)
# ============================================================================

echo "=== Phase 2: Scraping and Embedding Content ==="
echo "Crawling entire site and automatically embedding into Qdrant..."
echo "This may take several minutes depending on site size..."
echo

# Crawl the site with automatic embedding
# Auto-embedding is ENABLED by default - content goes directly into Qdrant
# Options:
#   --include-paths: Only crawl documentation paths
#   --wait: Wait for crawl to complete (synchronous)
#   --progress: Show progress bar
#   No --limit: Let it crawl everything (user controls this)

axon crawl "$TARGET_SITE" \
    --include-paths "/introduction,/features/*,/api-reference/*" \
    --wait \
    --progress

echo "✅ Content crawled and embedded into Qdrant collection: $COLLECTION"
echo

# ============================================================================
# PHASE 3: VERIFY EMBEDDINGS
# ============================================================================

echo "=== Phase 3: Verifying Embedded Content ==="
echo "Checking what was indexed into Qdrant..."
echo

# List all sources that were embedded
echo "Indexed sources:"
axon sources --collection "$COLLECTION" | head -n 20

# Get database statistics
echo
echo "Database statistics:"
axon stats --collection "$COLLECTION" --verbose

echo

# ============================================================================
# PHASE 4: SEMANTIC SEARCH QUERIES
# ============================================================================

echo "=== Phase 4: Testing Semantic Search ==="
echo "Running example queries against the RAG system..."
echo

# Example Query 1: Basic semantic search
echo "Query 1: 'How do I scrape a website?'"
axon query "How do I scrape a website?" \
    --collection "$COLLECTION" \
    --limit 3 \
    --pretty \
    -o "$OUTPUT_DIR/query1-results.json"

echo "Results saved to: $OUTPUT_DIR/query1-results.json"
echo "Top result:"
jq -r '.[0] | "Score: \(.score) | URL: \(.url) | \(.content[0:200])..."' "$OUTPUT_DIR/query1-results.json"
echo

# Example Query 2: Filtered search by domain
echo "Query 2: 'API authentication' (docs domain only)"
axon query "API authentication" \
    --collection "$COLLECTION" \
    --domain "docs.firecrawl.dev" \
    --limit 3 \
    --pretty \
    -o "$OUTPUT_DIR/query2-results.json"

echo "Results saved to: $OUTPUT_DIR/query2-results.json"
echo "Top result:"
jq -r '.[0] | "Score: \(.score) | URL: \(.url)"' "$OUTPUT_DIR/query2-results.json"
echo

# Example Query 3: Retrieve full document
echo "Query 3: Retrieving full document by URL"
SAMPLE_URL=$(axon sources --collection "$COLLECTION" --json | jq -r '.[0]')

if [[ -z "$SAMPLE_URL" || "$SAMPLE_URL" == "null" ]]; then
    echo "No sources found in collection, skipping retrieve" >&2
else
    echo "Retrieving: $SAMPLE_URL"

    axon retrieve "$SAMPLE_URL" \
        --collection "$COLLECTION" \
        -o "$OUTPUT_DIR/retrieved-doc.json"

    echo "✅ Full document retrieved"
    echo "Saved to: $OUTPUT_DIR/retrieved-doc.json"
fi
echo

# ============================================================================
# PHASE 5: ADVANCED RAG WORKFLOWS
# ============================================================================

echo "=== Phase 5: Advanced RAG Patterns ==="
echo

# Pattern 1: Grouped results (deduplicate by URL)
echo "Pattern 1: Grouped search results (one chunk per URL)"
axon query "crawling websites" \
    --collection "$COLLECTION" \
    --group \
    --limit 5

echo

# Pattern 2: Full document retrieval with semantic search
echo "Pattern 2: Semantic search + full document retrieval"
echo "Find relevant page, then retrieve full content..."

# First, find the most relevant URL
RELEVANT_URL=$(axon query "how to use the API" \
    --collection "$COLLECTION" \
    --limit 1 \
    --json | jq -r '.[0].url')

if [[ -z "$RELEVANT_URL" || "$RELEVANT_URL" == "null" ]]; then
    echo "No results found, skipping retrieve" >&2
else
    echo "Most relevant page: $RELEVANT_URL"

    # Then retrieve the full document
    axon retrieve "$RELEVANT_URL" \
        --collection "$COLLECTION" \
        -o "$OUTPUT_DIR/full-relevant-doc.json"

    echo "✅ Full document retrieved"
fi
echo

# Pattern 3: Multi-query aggregation
echo "Pattern 3: Aggregating results from multiple queries"
echo "Useful for comprehensive research on related topics..."

# Run multiple queries and combine results
queries=(
    "web scraping"
    "crawling websites"
    "extracting data"
)

for query in "${queries[@]}"; do
    echo "  Querying: $query"
    axon query "$query" \
        --collection "$COLLECTION" \
        --limit 2 \
        --json >> "$OUTPUT_DIR/multi-query-results.jsonl"
done

echo "✅ Multi-query results saved to: $OUTPUT_DIR/multi-query-results.jsonl"
echo

# ============================================================================
# PHASE 6: DATABASE MANAGEMENT
# ============================================================================

echo "=== Phase 6: Database Management ==="
echo

# Show indexing history
echo "Recent indexing activity:"
axon history --days 1 --collection "$COLLECTION"
echo

# Show unique domains in collection
echo "Domains indexed:"
axon domains --collection "$COLLECTION"
echo

# Optional: Clean up old content (commented out for safety)
# echo "Cleaning up old content..."
# axon delete --domain "old-docs.example.com" --collection "$COLLECTION" --yes
# echo "✅ Old content removed"
# echo

# ============================================================================
# PHASE 7: INCREMENTAL UPDATES
# ============================================================================

echo "=== Phase 7: Incremental Updates ==="
echo "Adding new content to existing RAG system..."
echo

# Scrape a new page and add to existing collection
NEW_PAGE="https://docs.firecrawl.dev/changelog"

echo "Scraping new page: $NEW_PAGE"
axon scrape "$NEW_PAGE" \
    --only-main-content \
    --format markdown \
    --collection "$COLLECTION"

echo "✅ New content added to collection"
echo

# Verify it's searchable
echo "Verifying new content is searchable..."
axon query "changelog updates" \
    --collection "$COLLECTION" \
    --limit 1

echo

# ============================================================================
# RESULTS SUMMARY
# ============================================================================

echo "=== RAG Pipeline Complete ==="
echo
echo "Summary:"
echo "  1. ✅ Discovered $TOTAL_PAGES pages via sitemap"
echo "  2. ✅ Crawled and embedded content into Qdrant"
echo "  3. ✅ Verified embeddings with stats and sources"
echo "  4. ✅ Tested semantic search queries"
echo "  5. ✅ Demonstrated advanced RAG patterns"
echo "  6. ✅ Managed database with history and domains"
echo "  7. ✅ Added incremental content updates"
echo
echo "Generated files in $OUTPUT_DIR:"
ls -lh "$OUTPUT_DIR"
echo
echo "Your RAG system is now ready for production use!"
echo
echo "Next steps:"
echo "  1. Query your RAG system: axon query \"your question\""
echo "  2. Add more content: axon crawl https://your-docs.com"
echo "  3. Monitor database: axon stats --verbose"
echo "  4. Check indexing history: axon history --days 7"
echo
echo "For more RAG features, see: skills/axon/references/vector-database.md"
