#!/bin/bash
# Example: Website Change Monitoring with Axon
# Purpose: Track changes to web pages over time for alerts and analysis
# Use Case: Monitor documentation, pricing pages, terms of service, competitor sites

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
MONITOR_DIR="/tmp/axon-monitoring"
STATE_DIR="$MONITOR_DIR/state"
DIFF_DIR="$MONITOR_DIR/diffs"
ALERT_DIR="$MONITOR_DIR/alerts"

# Create directories
mkdir -p "$STATE_DIR" "$DIFF_DIR" "$ALERT_DIR"

# Target pages to monitor
MONITORED_PAGES=(
    "https://docs.firecrawl.dev/introduction"
    "https://docs.firecrawl.dev/pricing"
    "https://docs.firecrawl.dev/changelog"
)

echo "=== Axon Website Change Monitoring Example ==="
echo "Monitor Directory: $MONITOR_DIR"
echo "Pages to Monitor: ${#MONITORED_PAGES[@]}"
echo

# ============================================================================
# EXAMPLE 1: INITIAL BASELINE CAPTURE
# ============================================================================

echo "=== Example 1: Capturing Initial Baseline ==="
echo "Creating baseline snapshots of monitored pages..."
echo

for url in "${MONITORED_PAGES[@]}"; do
    # Generate filename from URL
    FILENAME=$(echo "$url" | sed 's|https://||' | sed 's|/|_|g')
    BASELINE_FILE="$STATE_DIR/${FILENAME}.baseline.md"

    echo "Capturing: $url"

    # Scrape page without embedding (we just want the content)
    axon scrape "$url" \
        --only-main-content \
        --format markdown \
        --no-embed \
        -o "$BASELINE_FILE"

    echo "  → Saved to: $BASELINE_FILE"
done

echo "✅ Baseline snapshots captured"
echo

# ============================================================================
# EXAMPLE 2: CHECK FOR CHANGES
# ============================================================================

echo "=== Example 2: Checking for Changes ==="
echo "Comparing current content against baseline..."
echo

CHANGES_DETECTED=0

for url in "${MONITORED_PAGES[@]}"; do
    FILENAME=$(echo "$url" | sed 's|https://||' | sed 's|/|_|g')
    BASELINE_FILE="$STATE_DIR/${FILENAME}.baseline.md"
    CURRENT_FILE="$STATE_DIR/${FILENAME}.current.md"
    DIFF_FILE="$DIFF_DIR/${FILENAME}.$(date +%Y%m%d-%H%M%S).diff"

    echo "Checking: $url"

    # Scrape current version
    axon scrape "$url" \
        --only-main-content \
        --format markdown \
        --no-embed \
        -o "$CURRENT_FILE"

    # Compare with baseline
    if [[ -f "$BASELINE_FILE" ]]; then
        if diff -u "$BASELINE_FILE" "$CURRENT_FILE" > "$DIFF_FILE" 2>&1; then
            echo "  → No changes detected"
            rm "$DIFF_FILE"  # Clean up empty diff
        else
            echo "  ⚠️ CHANGES DETECTED!"
            echo "  → Diff saved to: $DIFF_FILE"

            # Count lines changed
            ADDED=$(grep -c '^+[^+]' "$DIFF_FILE" || echo 0)
            REMOVED=$(grep -c '^-[^-]' "$DIFF_FILE" || echo 0)
            echo "  → Lines added: $ADDED, removed: $REMOVED"

            ((CHANGES_DETECTED++))

            # Create alert file
            ALERT_FILE="$ALERT_DIR/${FILENAME}.$(date +%Y%m%d-%H%M%S).alert.txt"
            cat > "$ALERT_FILE" <<EOF
Website Change Alert
====================

URL: $url
Timestamp: $(date)

Changes:
  Lines added: $ADDED
  Lines removed: $REMOVED

Diff file: $DIFF_FILE

Action Required:
  - Review changes in diff file
  - Update baseline if changes are expected
  - Investigate if changes are unexpected
EOF

            echo "  → Alert created: $ALERT_FILE"
        fi
    else
        echo "  ⚠️ No baseline found, treating current as baseline"
        cp "$CURRENT_FILE" "$BASELINE_FILE"
    fi

    echo
done

echo "Summary: $CHANGES_DETECTED pages changed"
echo

# ============================================================================
# EXAMPLE 3: DETAILED CHANGE ANALYSIS
# ============================================================================

echo "=== Example 3: Detailed Change Analysis ==="
echo

if [[ $CHANGES_DETECTED -gt 0 ]]; then
    echo "Analyzing changes in detail..."
    echo

    # Show sample diff output
    SAMPLE_DIFF=$(find "$DIFF_DIR" -name "*.diff" -type f | head -n 1)

    if [[ -n "$SAMPLE_DIFF" ]]; then
        echo "Sample diff output (first 30 lines):"
        echo "================================"
        head -n 30 "$SAMPLE_DIFF"
        echo "================================"
        echo
        echo "Full diff available at: $SAMPLE_DIFF"
    fi
else
    echo "No changes detected - nothing to analyze"
fi
echo

# ============================================================================
# EXAMPLE 4: MONITORING SPECIFIC SECTIONS
# ============================================================================

echo "=== Example 4: Monitoring Specific Page Sections ==="
echo "Tracking changes to specific content patterns..."
echo

TARGET_URL="https://docs.firecrawl.dev/pricing"
SECTION_PATTERN="## Pricing"  # Monitor pricing section

echo "Monitoring section: '$SECTION_PATTERN' on $TARGET_URL"

# Scrape current version
CURRENT_CONTENT=$(axon scrape "$TARGET_URL" \
    --only-main-content \
    --format markdown \
    --no-embed)

# Extract section using awk
SECTION_CONTENT=$(echo "$CURRENT_CONTENT" | awk '/^## Pricing/,/^## [^#]/')

# Save section to file
SECTION_FILE="$STATE_DIR/pricing-section.$(date +%Y%m%d-%H%M%S).md"
echo "$SECTION_CONTENT" > "$SECTION_FILE"

echo "✅ Section captured: $SECTION_FILE"
echo "Section preview (first 10 lines):"
head -n 10 "$SECTION_FILE"
echo

# ============================================================================
# EXAMPLE 5: SCHEDULED MONITORING (CRON PATTERN)
# ============================================================================

echo "=== Example 5: Automated Monitoring Pattern ==="
echo "Demonstrating how to set up scheduled monitoring..."
echo

cat > "$MONITOR_DIR/monitor-cron.sh" <<'EOF'
#!/bin/bash
# Automated Website Monitor - Run via cron
# Schedule: 0 */6 * * * /tmp/axon-monitoring/monitor-cron.sh

set -euo pipefail

# Configuration
source ~/.claude-homelab/.env
MONITOR_DIR="/tmp/axon-monitoring"
STATE_DIR="$MONITOR_DIR/state"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)

# Pages to monitor
PAGES=(
    "https://docs.firecrawl.dev/introduction"
    "https://docs.firecrawl.dev/pricing"
)

# Monitoring function
check_page() {
    local url="$1"
    local filename=$(echo "$url" | sed 's|https://||' | sed 's|/|_|g')
    local previous="$STATE_DIR/${filename}.latest.md"
    local current="$STATE_DIR/${filename}.${TIMESTAMP}.md"

    # Scrape current version
    axon scrape "$url" \
        --only-main-content \
        --format markdown \
        --no-embed \
        -o "$current"

    # Compare if previous exists
    if [[ -f "$previous" ]]; then
        if ! diff -q "$previous" "$current" > /dev/null 2>&1; then
            echo "CHANGE DETECTED: $url"

            # Send notification (requires gotify skill)
            if command -v notify-send &>/dev/null; then
                notify-send "Website Changed" "$url has been modified"
            fi

            # Or use Gotify (if configured)
            # bash ~/claude-homelab/skills/gotify/scripts/send.sh \
            #     -t "Website Monitor" \
            #     -m "Changes detected on $url" \
            #     -p 7
        fi
    fi

    # Update latest
    ln -sf "$(basename "$current")" "$previous"
}

# Monitor all pages
for page in "${PAGES[@]}"; do
    check_page "$page"
done

# Cleanup old snapshots (keep last 30 days)
find "$STATE_DIR" -name "*.md" -mtime +30 -delete

echo "Monitoring run complete: $TIMESTAMP"
EOF

chmod +x "$MONITOR_DIR/monitor-cron.sh"

echo "✅ Automated monitoring script created"
echo "Location: $MONITOR_DIR/monitor-cron.sh"
echo
echo "To schedule with cron:"
echo "  crontab -e"
echo "  # Add line:"
echo "  0 */6 * * * $MONITOR_DIR/monitor-cron.sh"
echo
echo "This will check for changes every 6 hours"
echo

# ============================================================================
# EXAMPLE 6: CHANGE NOTIFICATION INTEGRATION
# ============================================================================

echo "=== Example 6: Change Notification Integration ==="
echo "Sending alerts when changes are detected..."
echo

if [[ $CHANGES_DETECTED -gt 0 ]]; then
    # Check if Gotify is configured
    if [[ -n "${GOTIFY_URL:-}" ]] && [[ -n "${GOTIFY_TOKEN:-}" ]]; then
        echo "Gotify configured, sending notification..."

        # Build notification message
        MESSAGE="Website monitoring detected $CHANGES_DETECTED page(s) with changes:"
        for url in "${MONITORED_PAGES[@]}"; do
            FILENAME=$(echo "$url" | sed 's|https://||' | sed 's|/|_|g')
            DIFF_FILE=$(find "$DIFF_DIR" -name "${FILENAME}.*.diff" -type f | tail -n 1)
            if [[ -n "$DIFF_FILE" ]]; then
                MESSAGE+=$'\n'"- $url"
            fi
        done

        # Send Gotify notification
        bash ~/claude-homelab/skills/gotify/scripts/send.sh \
            -t "Website Changes Detected" \
            -m "$MESSAGE" \
            -p 7

        echo "✅ Notification sent via Gotify"
    else
        echo "⚠️ Gotify not configured - notification skipped"
        echo "To enable: Set GOTIFY_URL and GOTIFY_TOKEN in .env"
    fi
else
    echo "No changes detected - no notifications sent"
fi
echo

# ============================================================================
# EXAMPLE 7: HISTORICAL CHANGE TRACKING
# ============================================================================

echo "=== Example 7: Historical Change Tracking ==="
echo "Maintaining version history of monitored pages..."
echo

# Create timestamped snapshots for history
for url in "${MONITORED_PAGES[@]}"; do
    FILENAME=$(echo "$url" | sed 's|https://||' | sed 's|/|_|g')
    HISTORY_DIR="$MONITOR_DIR/history/$FILENAME"
    mkdir -p "$HISTORY_DIR"

    SNAPSHOT_FILE="$HISTORY_DIR/$(date +%Y%m%d-%H%M%S).md"

    # Copy current state to history
    CURRENT_FILE="$STATE_DIR/${FILENAME}.current.md"
    if [[ -f "$CURRENT_FILE" ]]; then
        cp "$CURRENT_FILE" "$SNAPSHOT_FILE"
        echo "Saved snapshot: $SNAPSHOT_FILE"
    fi
done

echo "✅ Historical snapshots saved"
echo

# Show history summary
echo "Version history:"
for url in "${MONITORED_PAGES[@]}"; do
    FILENAME=$(echo "$url" | sed 's|https://||' | sed 's|/|_|g')
    HISTORY_DIR="$MONITOR_DIR/history/$FILENAME"

    if [[ -d "$HISTORY_DIR" ]]; then
        COUNT=$(find "$HISTORY_DIR" -name "*.md" -type f | wc -l)
        echo "  $url: $COUNT versions"
    fi
done
echo

# ============================================================================
# EXAMPLE 8: UPDATE BASELINE
# ============================================================================

echo "=== Example 8: Updating Baseline After Review ==="
echo "Once changes are reviewed and accepted, update baseline..."
echo

# Simulate updating baseline for pages with expected changes
for url in "${MONITORED_PAGES[@]}"; do
    FILENAME=$(echo "$url" | sed 's|https://||' | sed 's|/|_|g')
    BASELINE_FILE="$STATE_DIR/${FILENAME}.baseline.md"
    CURRENT_FILE="$STATE_DIR/${FILENAME}.current.md"

    if [[ -f "$CURRENT_FILE" ]]; then
        # Backup old baseline
        if [[ -f "$BASELINE_FILE" ]]; then
            BACKUP_FILE="$BASELINE_FILE.$(date +%Y%m%d-%H%M%S).bak"
            cp "$BASELINE_FILE" "$BACKUP_FILE"
            echo "Backed up baseline: $BACKUP_FILE"
        fi

        # Update baseline
        cp "$CURRENT_FILE" "$BASELINE_FILE"
        echo "Updated baseline: $BASELINE_FILE"
    fi
done

echo "✅ Baselines updated"
echo

# ============================================================================
# RESULTS SUMMARY
# ============================================================================

echo "=== Website Monitoring Examples Complete ==="
echo
echo "Summary:"
echo "  1. ✅ Captured initial baselines"
echo "  2. ✅ Detected changes via diff comparison"
echo "  3. ✅ Analyzed change details"
echo "  4. ✅ Monitored specific page sections"
echo "  5. ✅ Created automated monitoring script"
echo "  6. ✅ Integrated change notifications"
echo "  7. ✅ Maintained version history"
echo "  8. ✅ Updated baselines after review"
echo
echo "Generated monitoring structure:"
tree -L 2 "$MONITOR_DIR" 2>/dev/null || find "$MONITOR_DIR" -type f | head -20
echo
echo "Key files:"
echo "  - Baselines: $STATE_DIR/*.baseline.md"
echo "  - Current: $STATE_DIR/*.current.md"
echo "  - Diffs: $DIFF_DIR/*.diff"
echo "  - Alerts: $ALERT_DIR/*.alert.txt"
echo "  - History: $MONITOR_DIR/history/*/*.md"
echo "  - Cron script: $MONITOR_DIR/monitor-cron.sh"
echo
echo "Monitoring workflow:"
echo "  1. Run this script to establish baseline"
echo "  2. Schedule monitor-cron.sh with cron (every 6 hours recommended)"
echo "  3. Review alerts when changes are detected"
echo "  4. Update baselines after reviewing expected changes"
echo "  5. Investigate unexpected changes immediately"
echo
echo "Next steps:"
echo "  1. Customize MONITORED_PAGES array for your URLs"
echo "  2. Set up Gotify for push notifications"
echo "  3. Schedule automated monitoring with cron"
echo "  4. Integrate with your incident management system"
echo
echo "For more monitoring patterns, see: skills/axon/README.md"
