# Axon Quick Reference

Quick command examples for common Axon operations.

## Authentication

```bash
# Set API key (cloud)
export FIRECRAWL_API_KEY="fc-your-api-key"

# Set custom API URL (self-hosted)
export FIRECRAWL_API_URL="http://localhost:3002"

# Check status
axon --status
```

## Scrape Single Pages

```bash
# Basic scrape
axon https://example.com

# Only main content (recommended)
axon https://example.com --only-main-content

# Multiple formats
axon https://example.com --format markdown,html,links

# Wait for JavaScript
axon https://example.com --wait-for 5000

# Take screenshot
axon https://example.com --screenshot

# Save to file
axon https://example.com --only-main-content -o output.md

# Pretty JSON output
axon https://example.com --format markdown,html --pretty -o output.json

# Wrapper script
./scripts/scrape.sh https://example.com output.md
```

## Search the Web

```bash
# Basic search
axon search "AI agent benchmarks"

# Search and scrape results
axon search "web scraping tutorials" --scrape --limit 5

# Filter by source
axon search "AI news" --sources news
axon search "AI diagrams" --sources images

# Time-based filtering
axon search "latest AI" --tbs qdr:d   # Last day
axon search "this week" --tbs qdr:w   # Last week
axon search "this month" --tbs qdr:m  # Last month

# Category filtering
axon search "machine learning" --categories github,research

# Geographic search
axon search "local events" --location "San Francisco" --country US

# Save results
axon search "AI agents" --scrape -o results/

# Wrapper script
./scripts/search-scrape.sh "AI benchmarks" 5
```

## Map Website URLs

```bash
# Basic mapping
axon map https://example.com

# Limit results
axon map https://example.com --limit 1000

# Search for paths
axon map https://example.com --search "blog"

# Include subdomains
axon map https://example.com --include-subdomains

# Only use sitemap
axon map https://example.com --sitemap only

# Ignore query parameters
axon map https://example.com --ignore-query-parameters

# Save as JSON
axon map https://example.com --json -o sitemap.json

# Wrapper script
./scripts/map-site.sh https://example.com sitemap.json
```

## Crawl Websites

```bash
# Basic crawl with progress
axon crawl https://example.com --wait --progress

# Limit pages and depth
axon crawl https://example.com --limit 50 --max-depth 3 --wait

# Include specific paths
axon crawl https://example.com --include-paths "/blog/*" --wait
axon crawl https://example.com --include-paths "/docs/*,/api/*" --wait

# Exclude paths
axon crawl https://example.com --exclude-paths "/admin/*" --wait

# Rate limiting (polite crawling)
axon crawl https://example.com --delay 1000 --max-concurrency 3 --wait

# Crawl entire domain
axon crawl https://example.com --crawl-entire-domain --wait

# Async crawl (returns job ID)
axon crawl https://example.com --limit 100

# Wrapper script
./scripts/crawl-site.sh https://example.com 100 3
```

## Utility Commands

```bash
# Check authentication and credits
axon --status

# View credit usage (cloud only)
axon credit-usage

# Login
axon login --browser
axon login --api-key fc-YOUR-KEY

# View configuration
axon config

# Logout
axon logout

# Check version
axon --version

# Command help
axon --help
axon <command> --help
```

## Output Formatting

```bash
# Single format (raw output, pipe-friendly)
axon https://example.com --format markdown

# Multiple formats (JSON output)
axon https://example.com --format markdown,html,links

# Pretty-print JSON
axon https://example.com --format markdown,html --pretty

# Save to file
axon https://example.com --format markdown -o output.md
axon https://example.com --format markdown,html --pretty -o output.json
```

## Filtering Content

```bash
# Only main content (removes navigation, footers)
axon https://example.com --only-main-content

# Include specific HTML tags
axon https://example.com --include-tags "article,main,p"

# Exclude specific HTML tags
axon https://example.com --exclude-tags "nav,footer,aside"

# Wait for JavaScript rendering
axon https://example.com --wait-for 3000
```

## Time-Based Search Filters

```bash
# Last hour
axon search "breaking news" --tbs qdr:h

# Last day
axon search "latest updates" --tbs qdr:d

# Last week
axon search "this week's news" --tbs qdr:w

# Last month
axon search "monthly report" --tbs qdr:m

# Last year
axon search "annual review" --tbs qdr:y
```

## Common Workflows

### Extract Article Content for AI

```bash
axon https://blog.example.com/article --only-main-content --format markdown
```

### Research Competitor Websites

```bash
# Map site structure
axon map https://competitor.com --json -o competitor-urls.json

# Crawl specific sections
axon crawl https://competitor.com --include-paths "/products/*" --wait
```

### Monitor Website Changes

```bash
# Scrape page and save
axon https://example.com/changelog --only-main-content -o changelog-$(date +%Y%m%d).md

# Compare with previous version
diff changelog-20260201.md changelog-20260203.md
```

### Gather Training Data

```bash
# Search and scrape multiple results
axon search "machine learning tutorials" --scrape --limit 20 -o ml-data/

# Crawl documentation sites
axon crawl https://docs.example.com --limit 500 --wait
```

### Build Site Archive

```bash
# Map all URLs first
axon map https://example.com --json -o sitemap.json

# Crawl with full content
axon crawl https://example.com --limit 1000 --format markdown,html --wait
```

## Performance Tips

```bash
# Reduce data size with main content only
axon https://example.com --only-main-content

# Add delay to avoid rate limiting
axon crawl https://example.com --delay 1000 --wait

# Limit concurrency for polite scraping
axon crawl https://example.com --max-concurrency 3 --wait

# Filter paths to reduce scope
axon crawl https://example.com --include-paths "/blog/*" --wait

# Use max depth to control crawl size
axon crawl https://example.com --max-depth 2 --wait

# Map first to understand structure
axon map https://example.com --limit 500
```

## Using with jq (JSON Processing)

```bash
# Extract URLs from search results
axon search "AI" --limit 10 | jq -r '.results[].url'

# Count discovered URLs
axon map https://example.com --json | jq '.urls | length'

# Filter crawl results by status
axon crawl https://example.com --wait | jq '.pages[] | select(.status == 200)'

# Extract markdown content
axon https://example.com --format markdown,html --pretty | jq -r '.markdown'
```

## Piping and Redirecting

```bash
# Pipe to less for viewing
axon https://example.com | less

# Pipe to grep for searching
axon https://example.com | grep "keyword"

# Redirect to file
axon https://example.com > output.md

# Append to file
axon https://example.com >> archive.md

# Error output to file
axon https://example.com 2> errors.log
```

## Environment Variable Overrides

```bash
# Use different API key
FIRECRAWL_API_KEY="fc-other-key" axon https://example.com

# Use self-hosted instance
FIRECRAWL_API_URL="http://localhost:3002" axon https://example.com

# Disable telemetry
FIRECRAWL_NO_TELEMETRY=1 axon https://example.com
```

## Wrapper Script Examples

All wrapper scripts source credentials from `~/.claude-homelab/.env`.

```bash
# Scrape single URL
cd ~/claude-homelab/skills/axon
./scripts/scrape.sh https://example.com
./scripts/scrape.sh https://example.com output.md

# Search and scrape
./scripts/search-scrape.sh "AI benchmarks" 5

# Map website
./scripts/map-site.sh https://example.com
./scripts/map-site.sh https://example.com sitemap.json

# Crawl website
./scripts/crawl-site.sh https://example.com
./scripts/crawl-site.sh https://example.com 100 3
```

## Error Handling

```bash
# Check if command succeeded
if axon https://example.com --only-main-content; then
    echo "Success"
else
    echo "Failed"
fi

# Capture output and errors
output=$(axon https://example.com 2>&1)
if [ $? -eq 0 ]; then
    echo "$output"
else
    echo "Error: $output" >&2
fi
```

## Cron Scheduling

```bash
# Daily site scrape
0 2 * * * cd ~/claude-homelab/skills/axon && ./scripts/scrape.sh https://example.com > /tmp/daily-scrape.md

# Weekly site crawl
0 3 * * 0 cd ~/claude-homelab/skills/axon && ./scripts/crawl-site.sh https://example.com 500 5

# Hourly news search
0 * * * * cd ~/claude-homelab/skills/axon && ./scripts/search-scrape.sh "AI news" 10 > ~/news/$(date +\%Y\%m\%d-\%H).json
```

## Quick Comparison

| Task | Command | Use Case |
|------|---------|----------|
| Extract single page | `axon <url>` | Article extraction, content analysis |
| Search web | `axon search <query>` | Research, data gathering |
| Discover URLs | `axon map <url>` | Site structure analysis |
| Crawl website | `axon crawl <url>` | Full site archiving, documentation |

## Format Selection Guide

| Format | Use Case | Output |
|--------|----------|--------|
| `markdown` | LLM processing, AI training | Clean text |
| `html` | Preserve structure, styling | Cleaned HTML |
| `rawHtml` | Full page preservation | Original HTML |
| `links` | Link analysis, sitemap | URL array |
| `screenshot` | Visual reference, archiving | Base64 image |

---

**Need more detail?** See [API Endpoints Reference](./api-endpoints.md) or [Troubleshooting Guide](./troubleshooting.md).
