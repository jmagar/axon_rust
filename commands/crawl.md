---
description: Crawl entire website with depth and path controls
argument-hint: <url> [--max-pages N] [--max-depth N]
allowed-tools: Bash(axon *)
---

# Crawl Entire Website

Execute the Axon crawl command with the provided arguments:

```bash
axon crawl $ARGUMENTS
```

## Instructions

1. **Execute the command** using the Bash tool with the arguments provided
2. **Monitor the crawl progress** - crawling is asynchronous and may take time
3. **Parse the response** to extract:
   - List of discovered URLs
   - Scraped content for each page
   - Crawl statistics (pages found, depth reached)
   - Embedding confirmation
4. **Present the results** including:
   - Total pages discovered
   - Summary of content scraped
   - Any errors or warnings

## Expected Output

By default, crawl is **asynchronous** (`--wait false`): the command enqueues the job and returns a job ID immediately. Use `--wait true` to block until the crawl completes.

**Async (default):** Returns a job ID to track progress:
```
Crawl job enqueued: <job-id>
```

**Sync (`--wait true`) with `--json`:** Returns JSON containing:
- `job_id`: Crawl job identifier for status tracking
- `status`: Final crawl status (`completed` / `failed`)
- `pages`: Array of discovered and scraped pages
- `stats`: Crawl statistics

JSON output is only emitted when the `--json` flag is set. Without `--json`, output is human-readable text.

Present a summary of discovered pages and confirm successful embedding to Qdrant.
