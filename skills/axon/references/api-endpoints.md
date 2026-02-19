# Axon API Endpoints Reference

Complete reference for Axon CLI commands and API endpoints.

## Authentication

Axon supports multiple authentication methods:

### Cloud API Authentication

**Environment Variable (Recommended):**
```bash
export FIRECRAWL_API_KEY="fc-your-api-key"
```

**CLI Flag:**
```bash
axon <command> --api-key fc-your-api-key
```

**Interactive Login:**
```bash
# Browser OAuth (recommended for interactive use)
axon login --browser

# Direct API key entry
axon login --api-key fc-your-api-key
```

### Self-Hosted Authentication

Self-hosted instances automatically skip authentication:

```bash
export FIRECRAWL_API_URL="http://localhost:3002"
export FIRECRAWL_API_KEY=""  # Empty or omitted
```

## Core Commands

### Scrape

Extract content from a single URL.

**Endpoint:** `POST /scrape`

**CLI Usage:**
```bash
axon <url> [options]
```

**Parameters:**

| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `url` | string | URL to scrape (required) | - |
| `--format` | string | Output format(s): `markdown`, `html`, `rawHtml`, `links`, `screenshot` | `markdown` |
| `--only-main-content` | boolean | Extract only main content (removes navigation, footers) | `true` |
| `--wait-for` | number | Wait time for JavaScript rendering (milliseconds) | `0` |
| `--screenshot` | boolean | Capture page screenshot | `false` |
| `--include-tags` | string | Comma-separated HTML tags to include | all |
| `--exclude-tags` | string | Comma-separated HTML tags to exclude | none |
| `-o, --output` | string | Save output to file | stdout |
| `--pretty` | boolean | Pretty-print JSON output | `false` |

**Examples:**

```bash
# Basic scrape
axon https://example.com

# Multiple formats (returns JSON)
axon https://example.com --format markdown,html,links

# Wait for JavaScript
axon https://example.com --wait-for 5000

# Only main content with screenshot
axon https://example.com --only-main-content --screenshot

# Save to file
axon https://example.com --format markdown,html -o output.json --pretty
```

**Response:**

Single format:
```
# Raw content output (markdown, html, etc.)
```

Multiple formats:
```json
{
  "markdown": "# Page Title\n\nContent...",
  "html": "<html>...</html>",
  "links": ["https://example.com/page1", "https://example.com/page2"],
  "screenshot": "base64-encoded-image-data"
}
```

---

### Search

Search the web with optional content scraping.

**Endpoint:** `POST /search`

**CLI Usage:**
```bash
axon search <query> [options]
```

**Parameters:**

| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `query` | string | Search query (required) | - |
| `--limit` | number | Maximum results to return | `10` |
| `--scrape` | boolean | Scrape content from search results | `false` |
| `--sources` | string | Filter sources: `web`, `images`, `news` | `web` |
| `--categories` | string | Filter categories: `github`, `research`, `pdf` | all |
| `--tbs` | string | Time-based filter: `qdr:h`, `qdr:d`, `qdr:w`, `qdr:m`, `qdr:y` | none |
| `--location` | string | Geographic location for search | none |
| `--country` | string | Country code (US, UK, etc.) | none |
| `-o, --output` | string | Save results to directory | stdout |

**Examples:**

```bash
# Basic search
axon search "AI agent benchmarks"

# Search and scrape results
axon search "web scraping tutorials" --scrape --limit 5

# Filter by source and time
axon search "AI news" --sources news --tbs qdr:d

# Category filtering
axon search "machine learning" --categories github,research

# Geographic search
axon search "local events" --location "San Francisco" --country US

# Save results
axon search "AI agents" --scrape --limit 5 -o results/
```

**Response:**

```json
{
  "results": [
    {
      "title": "Page Title",
      "url": "https://example.com/page",
      "description": "Page description...",
      "content": "Scraped content (if --scrape used)..."
    }
  ],
  "total": 42
}
```

---

### Map

Discover all URLs on a website without scraping content.

**Endpoint:** `POST /map`

**CLI Usage:**
```bash
axon map <url> [options]
```

**Parameters:**

| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `url` | string | Website URL to map (required) | - |
| `--limit` | number | Maximum URLs to discover | `5000` |
| `--search` | string | Filter URLs by pattern | none |
| `--include-subdomains` | boolean | Include subdomain URLs | `false` |
| `--ignore-query-parameters` | boolean | Remove query strings for deduplication | `false` |
| `--sitemap` | string | Sitemap handling: `include`, `skip`, `only` | `include` |
| `--json` | boolean | Output as JSON | `false` |
| `-o, --output` | string | Save to file | stdout |

**Examples:**

```bash
# Basic mapping
axon map https://example.com

# Limit results
axon map https://example.com --limit 1000

# Search for specific paths
axon map https://example.com --search "blog"

# Include subdomains
axon map https://example.com --include-subdomains

# Only use sitemap
axon map https://example.com --sitemap only

# Save as JSON
axon map https://example.com --json -o sitemap.json
```

**Response:**

Text format:
```
https://example.com/
https://example.com/about
https://example.com/blog
https://example.com/contact
```

JSON format:
```json
{
  "urls": [
    "https://example.com/",
    "https://example.com/about",
    "https://example.com/blog",
    "https://example.com/contact"
  ],
  "total": 4
}
```

---

### Crawl

Systematically crawl entire websites with depth and path controls.

**Endpoint:** `POST /crawl`

**CLI Usage:**
```bash
axon crawl <url> [options]
```

**Parameters:**

| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `url` | string | Website URL to crawl (required) | - |
| `--limit` | number | Maximum pages to crawl | `100` |
| `--max-depth` | number | Maximum crawl depth from start URL | `2` |
| `--include-paths` | string | Comma-separated path patterns to include | all |
| `--exclude-paths` | string | Comma-separated path patterns to exclude | none |
| `--delay` | number | Delay between requests (milliseconds) | `0` |
| `--max-concurrency` | number | Maximum concurrent requests | `5` |
| `--wait` | boolean | Wait for crawl completion (synchronous) | `false` |
| `--progress` | boolean | Show progress indicator | `false` |
| `--poll-interval` | number | Status check interval for async mode (ms) | `2000` |
| `--crawl-entire-domain` | boolean | Remove scope restrictions | `false` |

**Examples:**

```bash
# Basic crawl with waiting
axon crawl https://example.com --wait --progress

# Limit pages and depth
axon crawl https://example.com --limit 50 --max-depth 3 --wait

# Path filtering
axon crawl https://example.com --include-paths "/blog/*" --wait
axon crawl https://example.com --exclude-paths "/admin/*,/api/*" --wait

# Rate limiting
axon crawl https://example.com --delay 1000 --max-concurrency 3 --wait

# Async crawl (returns job ID)
axon crawl https://example.com --limit 100
```

**Response:**

Synchronous mode (--wait):
```json
{
  "success": true,
  "pages": [
    {
      "url": "https://example.com/",
      "content": "Page content...",
      "status": 200
    }
  ],
  "total": 42
}
```

Asynchronous mode (default):
```json
{
  "success": true,
  "jobId": "abc123def456",
  "message": "Crawl started. Check status with job ID."
}
```

---

## Utility Commands

### Status

Check authentication status, concurrency, and remaining credits.

**CLI Usage:**
```bash
axon --status
```

**Response:**
```json
{
  "authenticated": true,
  "concurrency": "5/100",
  "credits": 9850,
  "version": "1.0.0"
}
```

---

### Credit Usage

Display remaining API credits (cloud API only).

**CLI Usage:**
```bash
axon credit-usage
```

**Response:**
```json
{
  "credits": 9850,
  "plan": "pro",
  "resetDate": "2026-03-01T00:00:00Z"
}
```

---

### Configuration

View or manage CLI configuration.

**CLI Usage:**
```bash
# View configuration
axon config

# Login
axon login --browser
axon login --api-key fc-YOUR-KEY

# Logout
axon logout
```

---

## Global Options

Available for all commands:

| Option | Description | Default |
|--------|-------------|---------|
| `--api-key <key>` | Override API key | `$FIRECRAWL_API_KEY` |
| `--api-url <url>` | Custom API endpoint | `https://api.firecrawl.dev` |
| `--help` | Show command help | - |
| `--version` | Show CLI version | - |

---

## Output Formats

### Single Format Output

When using a single format (e.g., `--format markdown`), returns raw content:

```bash
axon https://example.com --format markdown
```

Output:
```markdown
# Page Title

Content goes here...
```

### Multiple Format Output

When using multiple formats (e.g., `--format markdown,html,links`), returns JSON:

```bash
axon https://example.com --format markdown,html,links --pretty
```

Output:
```json
{
  "markdown": "# Page Title\n\nContent...",
  "html": "<html>...</html>",
  "links": ["https://example.com/link1", "https://example.com/link2"]
}
```

---

## Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `FIRECRAWL_API_KEY` | API key for authentication | Yes (cloud), No (self-hosted) |
| `FIRECRAWL_API_URL` | Custom API endpoint | No (defaults to cloud) |
| `FIRECRAWL_NO_TELEMETRY` | Disable usage analytics | No |

**Example:**

```bash
export FIRECRAWL_API_KEY="fc-your-api-key"
export FIRECRAWL_API_URL="https://api.firecrawl.dev"
export FIRECRAWL_NO_TELEMETRY=1
```

---

## Error Responses

### Authentication Errors

**401 Unauthorized:**
```json
{
  "error": "Unauthorized",
  "message": "Invalid or missing API key"
}
```

**Solution:** Check `FIRECRAWL_API_KEY` in `.env` or use `--api-key` flag.

### Rate Limiting

**429 Too Many Requests:**
```json
{
  "error": "Rate limit exceeded",
  "message": "Too many requests. Please slow down.",
  "retryAfter": 60
}
```

**Solution:** Add `--delay` to requests or reduce `--max-concurrency`.

### Validation Errors

**400 Bad Request:**
```json
{
  "error": "Bad Request",
  "message": "Invalid URL format"
}
```

**Solution:** Check URL format (must start with `http://` or `https://`).

### Server Errors

**500 Internal Server Error:**
```json
{
  "error": "Internal Server Error",
  "message": "An unexpected error occurred"
}
```

**Solution:** Retry request or contact support if persistent.

---

## Rate Limiting

### Cloud API Limits

- **Concurrent jobs:** Up to 100 (plan-dependent)
- **Requests per minute:** Varies by plan
- **Pages per crawl:** Configurable with `--limit`

**Check limits:**
```bash
axon --status
```

### Self-Hosted Limits

No default rate limiting. Configure as needed in self-hosted instance settings.

---

## Best Practices

1. **Use `--only-main-content`** to reduce data size by 50-80%
2. **Set appropriate `--delay`** (1000ms recommended) to avoid rate limiting
3. **Limit concurrency** with `--max-concurrency` for polite scraping
4. **Filter paths early** with `--include-paths` to reduce crawl scope
5. **Use `--max-depth`** to control crawl depth and execution time
6. **Map first** to understand site structure before full crawl
7. **Use async mode** for long-running crawls (omit `--wait`)
8. **Monitor credits** regularly with `credit-usage` command

---

## Resources

- [Official Documentation](https://docs.firecrawl.dev/)
- [CLI Documentation](https://docs.firecrawl.dev/sdks/cli)
- [GitHub Repository](https://github.com/firecrawl/firecrawl)
- [CLI GitHub](https://github.com/firecrawl/cli)
