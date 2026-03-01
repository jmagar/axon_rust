# axon search
Last Modified: 2026-02-25

Version: 1.0.0
Last Updated: 01:26:53 | 02/25/2026 EST

Web search via Tavily AI. Displays ranked search results with snippets and automatically enqueues crawl jobs for the result URLs, building the local knowledge base in the background.

## Synopsis

```bash
axon search <query> [FLAGS]
axon search --query "<query>" [FLAGS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<query>` | Search query (positional, or via `--query`) |

## Required Environment Variables

| Variable | Description |
|----------|-------------|
| `TAVILY_API_KEY` | Tavily AI Search API key. Get one at `https://tavily.com`. |

## Flags

All global flags apply. Key flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--query <text>` | — | Search query (alternative to positional argument). |
| `--limit <n>` | `10` | Maximum number of search results to retrieve. |
| `--json` | `false` | Machine-readable JSON output. |

Note: `search` runs synchronously and does not support `--wait`. Crawl jobs for the result URLs are enqueued asynchronously regardless.

## Examples

```bash
# Basic search
axon search "rust async web framework"

# Using --query flag
axon search --query "qdrant vector database tutorial"

# Limit results
axon search "tokio channel" --limit 5

# JSON output
axon search "spider.rs docs" --json
```

## Output

The command prints ranked results with:
- Position and title
- URL
- Snippet (if provided by Tavily)

After printing results, it enqueues crawl jobs for each unique origin domain (or exact URL if `--crawl-from-result` is set). The crawl jobs run in the background via the AMQP worker.

## Auto-crawl Behavior

By default, `search` strips each result URL to its scheme+host origin and enqueues one crawl job per unique origin. This means three results from `docs.example.com` produce one crawl job for `https://docs.example.com`, not three.

URLs that fail SSRF validation (private IPs, localhost, reserved ranges) are silently skipped before enqueue.

## Notes

- `search` does not perform LLM synthesis. For AI-synthesized research answers, use `axon research`.
- To search the local Qdrant knowledge base (not the web), use `axon query`.
- Crawl jobs are enqueued but not awaited. Check `axon status` or `axon crawl list` to monitor them.
- If `axon-workers` is not running, enqueued crawl jobs will pend until workers are started.
