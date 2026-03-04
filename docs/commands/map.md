# axon map
Last Modified: 2026-03-03

Version: 1.0.0
Last Updated: 20:29:46 | 03/03/2026 EST

Discover URLs from a site without writing markdown artifacts. Runs inline and returns discovered URLs plus crawl/map summary metrics.

## Synopsis

```bash
axon map <url> [FLAGS]
axon map --start-url <url> [FLAGS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<url>` | Start URL to map (optional if `--start-url` is set) |

## Flags

All global flags apply. Key flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--max-depth <n>` | `5` | Maximum depth for URL discovery. |
| `--render-mode <mode>` | `auto-switch` | `auto-switch` starts in HTTP and retries in Chrome only if HTTP sees zero pages. |
| `--discover-sitemaps <bool>` | `true` | Append sitemap/robots-discovered URLs and dedupe final list. |
| `--include-subdomains <bool>` | `false` | Include subdomains under same parent domain. |
| `--json` | `false` | Print structured payload including all discovered URLs. |

## Examples

```bash
# Positional start URL
axon map https://example.com/docs

# Using --start-url
axon map --start-url https://example.com/docs

# JSON output for automation
axon map https://example.com --json

# Disable sitemap discovery
axon map https://example.com --discover-sitemaps false
```

## Output

JSON mode returns:
- `url`
- `mapped_urls`
- `sitemap_urls`
- `pages_seen`
- `thin_pages`
- `elapsed_ms`
- `urls` (full discovered list)

## Behavior Notes

- `map` validates the URL before crawl starts.
- When `--render-mode auto-switch`, Chrome fallback is only attempted if HTTP mapping finds zero pages.
- `map` is synchronous and does not enqueue jobs.
