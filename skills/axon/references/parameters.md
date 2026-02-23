# Axon Parameters Reference

Complete reference for all Axon CLI parameters organized by category.

---

## Common Parameters

These parameters work across most commands:

- `-o, --output <path>`: Save output to file instead of stdout
- `--json`: Output as JSON format (compact)
- `--pretty`: Pretty-print JSON output (human-readable)
- `--timing`: Show request timing and performance information

---

## Format Parameters

Control output format for scraping operations:

### Format Types

- `--format <formats>`: Comma-separated list of output formats
  - `markdown`: Clean markdown text (LLM-ready)
  - `html`: Cleaned HTML
  - `rawHtml`: Original HTML source
  - `links`: Array of all URLs found on page
  - `images`: Array of all image URLs
  - `screenshot`: Base64-encoded page screenshot
  - `summary`: AI-generated summary of content
  - `changeTracking`: Track changes over time
  - `attributes`: Extract HTML element attributes
  - `branding`: Branding information

**Output behavior:**
- Single format → Returns raw content (pipe-friendly)
- Multiple formats → Returns JSON object with all formats

---

## Content Filtering Parameters

Control which content is extracted:

### Main Content Filtering

- `--only-main-content`: Strip navigation, footers, ads (default: true)
- `--no-only-main-content`: Include full page content

### HTML Tag Filtering

- `--include-tags <tags>`: Comma-separated list of HTML tags to include
  - Example: `--include-tags "article,main,p"`
- `--exclude-tags <tags>`: Comma-separated list of HTML tags to exclude
  - Default: `"nav,footer"`
  - Example: `--exclude-tags "nav,footer,aside,script"`

---

## JavaScript & Rendering Parameters

Control how JavaScript-heavy sites are handled:

- `--wait-for <ms>`: Wait time in milliseconds before scraping
  - Use for JavaScript-rendered content
  - Example: `--wait-for 3000` (wait 3 seconds)
- `--timeout <seconds>`: Request timeout in seconds
  - Default: 15 seconds
  - Example: `--timeout 30`

---

## Screenshot Parameters

Capture visual content:

- `--screenshot`: Take screenshot of page
  - Returns base64-encoded image in JSON
  - Requires `--format screenshot` or multiple formats

---

## Search Parameters

### Result Limiting

- `--limit <n>`: Maximum number of results to return
  - Example: `--limit 10`

### Source Filtering

- `--sources <types>`: Comma-separated source types
  - `web`: Web pages (default)
  - `news`: News articles
  - `images`: Image results
  - Example: `--sources web,news`

### Category Filtering

- `--categories <types>`: Comma-separated category types
  - `github`: GitHub repositories
  - `research`: Research papers
  - `pdf`: PDF documents
  - Example: `--categories github,research`

### Time-Based Filtering

- `--tbs <filter>`: Time-based search filter
  - `qdr:h`: Last hour
  - `qdr:d`: Last day (24 hours)
  - `qdr:w`: Last week
  - `qdr:m`: Last month
  - `qdr:y`: Last year
  - Example: `--tbs qdr:d`

### Geographic Filtering

- `--location <city>`: Geographic location for search
  - Example: `--location "San Francisco"`
- `--country <code>`: Country code (2-letter)
  - Example: `--country US`

### Search Actions

- `--scrape`: Extract content from search results
  - Scrapes each result URL
  - Increases execution time

---

## Mapping Parameters

Control URL discovery:

### URL Discovery

- `--limit <n>`: Maximum URLs to discover
- `--search <pattern>`: Filter URLs by pattern
  - Example: `--search "blog"`
  - Example: `--search "/docs/"`

### Subdomain Handling

- `--include-subdomains`: Include URLs from subdomains
  - Example: Includes `blog.example.com`, `docs.example.com`

### Query Parameter Handling

- `--ignore-query-parameters`: Remove query strings for deduplication
  - `https://example.com/page?id=1` becomes `https://example.com/page`

### Sitemap Handling

- `--sitemap <mode>`: Sitemap handling mode
  - `include`: Use sitemap + crawl (default)
  - `only`: Only use sitemap, no additional crawling
  - `skip`: Ignore sitemap completely
  - Example: `--sitemap only`

---

## Crawl Parameters

Control website traversal:

### Crawl Limits

- `--limit <n>`: Maximum number of pages to crawl
  - Example: `--limit 100`
- `--max-depth <n>`: Maximum crawl depth from start URL
  - Depth 0: Only start URL
  - Depth 1: Start URL + direct links
  - Depth 2: Start URL + direct links + their links
  - Example: `--max-depth 3`

### Path Filtering

- `--include-paths <patterns>`: Comma-separated path patterns to include
  - Supports wildcards: `*` (any characters), `?` (single character)
  - Example: `--include-paths "/blog/*"`
  - Example: `--include-paths "/docs/*,/api/*"`
- `--exclude-paths <patterns>`: Comma-separated path patterns to exclude
  - Example: `--exclude-paths "/admin/*,/private/*"`

### Rate Limiting & Concurrency

- `--delay <ms>`: Delay between requests in milliseconds
  - Recommended: 1000ms (1 second) for polite crawling
  - Example: `--delay 1000`
- `--max-concurrency <n>`: Maximum concurrent requests
  - Default varies by plan
  - Example: `--max-concurrency 5`

### Crawl Modes

- `--wait`: Wait for crawl completion (synchronous mode)
  - Blocks until finished
  - Required for `--progress`
- `--progress`: Show progress indicator
  - Only works with `--wait`
- `--poll-interval <ms>`: Status check interval for async mode
  - Default: 2000ms (2 seconds)
  - Example: `--poll-interval 5000`
- `--crawl-entire-domain`: Remove scope restrictions
  - Crawls all pages on domain
  - Ignores path patterns

---

## Extract Parameters

Control structured data extraction:

### Extraction Methods

- `--prompt <prompt>`: Natural language extraction prompt
  - Example: `--prompt "Extract product names and prices"`
- `--schema <json>`: JSON schema for structured extraction
  - Example: `--schema '{"type":"object","properties":{"name":{"type":"string"}}}'`
- `--system-prompt <prompt>`: System prompt for extraction context
  - Example: `--system-prompt "You are a product data extractor"`

### Extraction Scope

- `--allow-external-links`: Follow external links during extraction
- `--enable-web-search`: Enable web search for additional context
- `--include-subdomains`: Include subdomains when extracting

### Extraction Output

- `--show-sources`: Include source URLs in extraction result

---

## Batch Parameters

Control batch scraping operations:

### Batch Input

- `[urls...]`: Space-separated list of URLs to scrape
  - Example: `axon batch url1 url2 url3`

### Batch Execution

- `--wait`: Wait for batch completion
- `--poll-interval <seconds>`: Polling interval in seconds
  - Default: 5 seconds
- `--timeout <seconds>`: Timeout for wait operation

### Batch Configuration

- `--format <formats>`: Scrape format(s) for batch results
- `--only-main-content`: Only return main content
- `--wait-for <ms>`: Wait time before scraping in milliseconds
- `--scrape-timeout <seconds>`: Per-page scrape timeout
- `--screenshot`: Include screenshot format
- `--include-tags <tags>`: Tags to include
- `--exclude-tags <tags>`: Tags to exclude
- `--max-concurrency <number>`: Max concurrency for batch

### Batch Options

- `--ignore-invalid-urls`: Skip invalid URLs instead of failing
- `--webhook <url>`: Webhook URL for batch completion notification
- `--zero-data-retention`: Enable zero data retention
- `--idempotency-key <key>`: Idempotency key for batch job
  - Prevents duplicate jobs
- `--append-to-id <id>`: Append results to existing batch ID
- `--integration <name>`: Integration name for analytics

---

## RAG & Vector Database Parameters

Control embedding and vector database operations:

### Embedding Control

- `--no-embed`: Disable auto-embedding
  - Scrapes content without adding to vector database
  - Use for testing or non-RAG use cases
- `--remove`: Remove domain from vector DB before scraping
  - Clears existing content for domain
  - Useful for refreshing indexed content

### Collection Management

- `--collection <name>`: Qdrant collection name
  - Default: From `QDRANT_COLLECTION` environment variable
  - Example: `--collection documentation`

### Chunking Control

- `--no-chunk`: Disable chunking, embed as single vector
  - Default: Auto-chunking enabled
  - Use for short documents

### Query Parameters

- `--limit <number>`: Maximum number of query results
  - Default: 5
  - Example: `--limit 10`
- `--domain <domain>`: Filter results by domain
  - Example: `--domain docs.example.com`
- `--full`: Show full chunk text (not truncated)
  - Default: 100 character preview
- `--group`: Group results by URL
  - Consolidates chunks from same document

---

## Authentication Parameters

Control API authentication:

- `--api-key <key>`: Firecrawl API key
  - Overrides `FIRECRAWL_API_KEY` environment variable
  - Example: `--api-key fc-YOUR-KEY`
- `--api-url <url>`: Custom API endpoint
  - Overrides `FIRECRAWL_API_URL` environment variable
  - Use for self-hosted instances
  - Example: `--api-url http://localhost:3002`

---

## Output Parameters

Control output destination and format:

- `-o, --output <path>`: Save output to file
  - For directories: `results/` (search results)
  - For files: `output.md`, `output.json`
- `--json`: Output as JSON format (compact)
- `--pretty`: Pretty-print JSON (human-readable)
  - Includes indentation and line breaks
- `--no-pretty`: Disable pretty JSON output (list command)

---

## Performance Parameters

Control request behavior:

- `--timing`: Show request timing and performance info
  - Includes latency, processing time, etc.
- `--timeout <seconds>`: Request timeout
  - Default: 15 seconds
  - Example: `--timeout 30`

---

## Environment Variable Overrides

All parameters can be set via environment variables:

```bash
# Authentication
export FIRECRAWL_API_KEY="fc-your-key"
export FIRECRAWL_API_URL="https://api.firecrawl.dev"

# Qdrant Vector Database
export QDRANT_URL="http://localhost:6333"
export QDRANT_API_KEY=""
export QDRANT_COLLECTION="axon"

# Text Embeddings Inference
export TEI_URL="http://localhost:8080"
export TEI_MODEL="BAAI/bge-small-en-v1.5"
export TEI_DIMENSIONS=384

# Telemetry
export FIRECRAWL_NO_TELEMETRY=1  # Disable telemetry
```

---

## Parameter Constraints

**CRITICAL:** Do NOT add `--limit`, `--max-depth`, or other constraint parameters unless the user explicitly requests them.

- ❌ **Don't:** Automatically add `--limit 10` or `--max-depth 3`
- ✅ **Do:** Only add constraints when user says "max 10 results", "depth 3", etc.
- ✅ **Do:** Let operations run unlimited by default - user will stop them if needed
- ✅ **Do:** Trust the user to set appropriate limits for their use case

---

For command examples, see [commands.md](./commands.md)
For job management, see [job-management.md](./job-management.md)
For RAG features, see [vector-database.md](./vector-database.md)
