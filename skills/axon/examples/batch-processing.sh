#!/bin/bash
# Example: Batch Processing with Axon
# Purpose: Efficiently scrape multiple URLs in parallel using batch operations
# Use Case: Data collection, competitive analysis, content aggregation

set -euo pipefail

# ============================================================================
# CONFIGURATION
# ============================================================================

# Load credentials from .env file
if [[ -f ~/.claude-homelab/.env ]]; then
    set -a
    # shellcheck source=/dev/null
    source ~/.claude-homelab/.env
    set +a
else
    echo "ERROR: .env file not found at ~/.claude-homelab/.env" >&2
    exit 1
fi

# FIRECRAWL_API_KEY is optional for self-hosted setups; only passed if non-empty

# Configuration
OUTPUT_DIR="/tmp/axon-batch-processing"
mkdir -p "$OUTPUT_DIR"

echo "=== Axon Batch Processing Example ==="
echo "Output Directory: $OUTPUT_DIR"
echo

# ============================================================================
# EXAMPLE 1: SIMPLE BATCH SCRAPE (ASYNC)
# ============================================================================

echo "=== Example 1: Async Batch Scraping ==="
echo "Scraping multiple URLs in parallel without waiting..."
echo

# Define target URLs
URLS=(
    "https://docs.firecrawl.dev/introduction"
    "https://docs.firecrawl.dev/features/scrape"
    "https://docs.firecrawl.dev/features/crawl"
    "https://docs.firecrawl.dev/features/map"
    "https://docs.firecrawl.dev/api-reference/introduction"
)

# Start batch job (returns job ID immediately)
echo "Starting batch job for ${#URLS[@]} URLs..."

BATCH_OUTPUT=$(axon batch "${URLS[@]}" --json)
JOB_ID=$(echo "$BATCH_OUTPUT" | jq -r '.id')

echo "✅ Batch job started: $JOB_ID"
echo "Full response:"
echo "$BATCH_OUTPUT" | jq '.'
echo

# Save job ID for later status checks
echo "$JOB_ID" > "$OUTPUT_DIR/job-id.txt"
echo "Job ID saved to: $OUTPUT_DIR/job-id.txt"
echo

# ============================================================================
# EXAMPLE 2: CHECK BATCH STATUS
# ============================================================================

echo "=== Example 2: Checking Batch Status ==="
echo "Polling job status until completion..."
echo

# Poll the batch job status
MAX_ATTEMPTS=30
ATTEMPT=0
SLEEP_INTERVAL=5

while [[ $ATTEMPT -lt $MAX_ATTEMPTS ]]; do
    # Check status
    STATUS_OUTPUT=$(axon batch status "$JOB_ID" --json)
    STATUS=$(echo "$STATUS_OUTPUT" | jq -r '.status')
    COMPLETED=$(echo "$STATUS_OUTPUT" | jq -r '.completed // 0')
    TOTAL=$(echo "$STATUS_OUTPUT" | jq -r '.total // 0')
    FAILED=$(echo "$STATUS_OUTPUT" | jq -r '.failed // 0')

    echo "[$((ATTEMPT + 1))/$MAX_ATTEMPTS] Status: $STATUS | Progress: $COMPLETED/$TOTAL | Failed: $FAILED"

    # Check if completed
    if [[ "$STATUS" == "completed" ]]; then
        echo "✅ Batch job completed!"

        # Save full results
        echo "$STATUS_OUTPUT" | jq '.' > "$OUTPUT_DIR/batch-results.json"
        echo "Results saved to: $OUTPUT_DIR/batch-results.json"
        break
    fi

    # Check if failed
    if [[ "$STATUS" == "failed" ]]; then
        echo "❌ Batch job failed!"
        ERROR_MSG=$(echo "$STATUS_OUTPUT" | jq -r '.error // "Unknown error"')
        echo "Error: $ERROR_MSG"
        echo "$STATUS_OUTPUT" | jq '.' > "$OUTPUT_DIR/batch-results.json"
        exit 1
    fi

    # Wait before next check
    sleep $SLEEP_INTERVAL
    ATTEMPT=$((ATTEMPT + 1))
done

if [[ $ATTEMPT -ge $MAX_ATTEMPTS ]]; then
    echo "⚠️ Batch job did not complete within expected time"
    echo "Check status later with: axon batch status $JOB_ID"
fi
echo

# ============================================================================
# EXAMPLE 3: SYNCHRONOUS BATCH (WAIT FOR COMPLETION)
# ============================================================================

echo "=== Example 3: Synchronous Batch Scraping ==="
echo "Scraping multiple URLs and waiting for completion..."
echo

# Define another set of URLs
MORE_URLS=(
    "https://docs.firecrawl.dev/sdks/python"
    "https://docs.firecrawl.dev/sdks/node"
    "https://docs.firecrawl.dev/sdks/cli"
)

# Start batch with --wait flag (blocks until complete)
echo "Starting synchronous batch for ${#MORE_URLS[@]} URLs..."

axon batch "${MORE_URLS[@]}" \
    --wait \
    --poll-interval 3 \
    --json \
    --pretty \
    -o "$OUTPUT_DIR/sync-batch-results.json"

echo "✅ Synchronous batch complete"
echo "Results saved to: $OUTPUT_DIR/sync-batch-results.json"
echo

# Show summary
TOTAL_RESULTS=$(jq '.data | length' "$OUTPUT_DIR/sync-batch-results.json")
echo "Total pages scraped: $TOTAL_RESULTS"
echo

# ============================================================================
# EXAMPLE 4: BATCH WITH CUSTOM OPTIONS
# ============================================================================

echo "=== Example 4: Batch with Custom Scraping Options ==="
echo "Batch scraping with specific format and content filtering..."
echo

# Note: Batch scraping supports the same options as single scrape
# Common options: --format, --only-main-content, --wait-for, etc.

CUSTOM_URLS=(
    "https://docs.firecrawl.dev/changelog"
    "https://docs.firecrawl.dev/pricing"
)

axon batch "${CUSTOM_URLS[@]}" \
    --format markdown,html \
    --only-main-content \
    --wait \
    --pretty \
    -o "$OUTPUT_DIR/custom-batch-results.json"

echo "✅ Custom batch complete"
echo "Results with multiple formats saved"
echo

# ============================================================================
# EXAMPLE 5: PROCESSING BATCH RESULTS
# ============================================================================

echo "=== Example 5: Processing Batch Results ==="
echo "Extracting and analyzing scraped content..."
echo

# Read the batch results
RESULTS_FILE="$OUTPUT_DIR/batch-results.json"

if [[ -f "$RESULTS_FILE" ]]; then
    # Count successful vs failed
    SUCCESS_COUNT=$(jq '[.data[] | select(.status == "completed")] | length' "$RESULTS_FILE")
    FAILED_COUNT=$(jq '[.data[] | select(.status == "failed")] | length' "$RESULTS_FILE")

    echo "Success: $SUCCESS_COUNT"
    echo "Failed: $FAILED_COUNT"
    echo

    # Extract all URLs that were scraped
    echo "Successfully scraped URLs:"
    jq -r '.data[] | select(.status == "completed") | .url' "$RESULTS_FILE"
    echo

    # Extract markdown content from each result
    echo "Extracting markdown content..."
    jq -r '.data[] | select(.status == "completed") | .markdown' "$RESULTS_FILE" \
        > "$OUTPUT_DIR/all-content.md"

    echo "✅ All content combined into: $OUTPUT_DIR/all-content.md"

    # Show content statistics
    TOTAL_LINES=$(wc -l < "$OUTPUT_DIR/all-content.md")
    TOTAL_WORDS=$(wc -w < "$OUTPUT_DIR/all-content.md")
    echo "Total lines: $TOTAL_LINES"
    echo "Total words: $TOTAL_WORDS"
else
    echo "⚠️ Batch results file not found, skipping processing"
fi
echo

# ============================================================================
# EXAMPLE 6: HANDLING BATCH ERRORS
# ============================================================================

echo "=== Example 6: Handling Batch Errors ==="
echo "Checking for and handling failed URLs..."
echo

# Get errors from batch job
if [[ -n "${JOB_ID:-}" ]]; then
    echo "Checking errors for job: $JOB_ID"

    ERROR_OUTPUT=$(axon batch errors "$JOB_ID" --json 2>/dev/null || echo '{"errors":[]}')
    ERROR_COUNT=$(echo "$ERROR_OUTPUT" | jq '.errors | length')

    if [[ $ERROR_COUNT -gt 0 ]]; then
        echo "⚠️ Found $ERROR_COUNT errors"
        echo "$ERROR_OUTPUT" | jq '.errors' > "$OUTPUT_DIR/batch-errors.json"
        echo "Errors saved to: $OUTPUT_DIR/batch-errors.json"

        # Show first error
        echo "Sample error:"
        echo "$ERROR_OUTPUT" | jq -r '.errors[0] | "URL: \(.url)\nError: \(.error)\nStatus: \(.statusCode)"'
    else
        echo "✅ No errors found"
    fi
else
    echo "⚠️ No job ID available for error checking"
fi
echo

# ============================================================================
# EXAMPLE 7: BATCH WITH AUTO-EMBEDDING (RAG)
# ============================================================================

echo "=== Example 7: Batch Scraping with Auto-Embedding ==="
echo "Scraping multiple pages directly into RAG system..."
echo

# Check if Qdrant is configured
if [[ -n "${QDRANT_URL:-}" ]] && [[ -n "${TEI_URL:-}" ]]; then
    COLLECTION="${QDRANT_COLLECTION:-axon}"
    echo "Qdrant configured, embedding to collection: $COLLECTION"

    # Batch scrape with automatic embedding
    RAG_URLS=(
        "https://docs.firecrawl.dev/features/extract"
        "https://docs.firecrawl.dev/features/batch"
    )

    echo "Batch scraping and embedding ${#RAG_URLS[@]} URLs..."

    axon batch "${RAG_URLS[@]}" \
        --wait \
        --collection "$COLLECTION"

    echo "✅ Batch scraped and embedded into Qdrant"
    echo "Query the content with: axon query \"your question\""
else
    echo "⚠️ Qdrant not configured - skipping RAG example"
    echo "To enable: Set QDRANT_URL and TEI_URL in .env"
fi
echo

# ============================================================================
# EXAMPLE 8: BATCH FROM URL LIST FILE
# ============================================================================

echo "=== Example 8: Batch Processing from URL List ==="
echo "Reading URLs from file and batch processing..."
echo

# Create a URL list file
URL_LIST_FILE="$OUTPUT_DIR/url-list.txt"
cat > "$URL_LIST_FILE" <<EOF
https://docs.firecrawl.dev/introduction
https://docs.firecrawl.dev/changelog
https://docs.firecrawl.dev/pricing
EOF

echo "URL list created: $URL_LIST_FILE"
echo "Contents:"
cat "$URL_LIST_FILE"
echo

# Read URLs into array and batch process
mapfile -t URL_ARRAY < "$URL_LIST_FILE"

echo "Processing ${#URL_ARRAY[@]} URLs from file..."

axon batch "${URL_ARRAY[@]}" \
    --wait \
    --pretty \
    -o "$OUTPUT_DIR/file-batch-results.json"

echo "✅ File-based batch complete"
echo

# ============================================================================
# EXAMPLE 9: CANCEL RUNNING BATCH
# ============================================================================

echo "=== Example 9: Cancelling Batch Jobs ==="
echo "Demonstrating how to cancel long-running batch jobs..."
echo

# Start a batch that we'll cancel
CANCEL_URLS=(
    "https://docs.firecrawl.dev/introduction"
    "https://docs.firecrawl.dev/features/scrape"
)

echo "Starting batch job to demonstrate cancellation..."
CANCEL_OUTPUT=$(axon batch "${CANCEL_URLS[@]}" --json)
CANCEL_JOB_ID=$(echo "$CANCEL_OUTPUT" | jq -r '.id')

echo "Job started: $CANCEL_JOB_ID"
echo "Waiting 2 seconds..."
sleep 2

# Cancel the job
echo "Cancelling job..."
axon batch cancel "$CANCEL_JOB_ID"

echo "✅ Job cancelled"
echo "Note: Already completed items will remain available"
echo

# ============================================================================
# RESULTS SUMMARY
# ============================================================================

echo "=== Batch Processing Examples Complete ==="
echo
echo "Summary:"
echo "  1. ✅ Async batch scraping (start and check later)"
echo "  2. ✅ Status polling and monitoring"
echo "  3. ✅ Synchronous batch (wait for completion)"
echo "  4. ✅ Custom options (formats, filtering)"
echo "  5. ✅ Results processing and extraction"
echo "  6. ✅ Error handling and troubleshooting"
echo "  7. ✅ Batch with auto-embedding (RAG)"
echo "  8. ✅ Batch from URL list file"
echo "  9. ✅ Cancelling running jobs"
echo
echo "Generated files in $OUTPUT_DIR:"
ls -lh "$OUTPUT_DIR"
echo
echo "Key learnings:"
echo "  - Use async batch for large jobs (check status later)"
echo "  - Use sync batch (--wait) for smaller jobs (blocks until done)"
echo "  - Always handle errors with 'axon batch errors <job-id>'"
echo "  - Poll status every 3-5 seconds for responsive monitoring"
echo "  - Cancel long-running jobs with 'axon batch cancel <job-id>'"
echo
echo "Next steps:"
echo "  1. Process your own URL lists"
echo "  2. Integrate batch results into your workflow"
echo "  3. Set up webhooks for completion notifications"
echo
echo "For more batch options, see: skills/axon/references/job-management.md"
