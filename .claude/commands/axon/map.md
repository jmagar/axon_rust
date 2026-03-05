---
description: Discover URLs on a site without scraping content
argument-hint: <url> [--limit N]
allowed-tools: mcp__axon__axon, Bash
---

Use `mcp__axon__axon` directly:

```json
{ "action": "map", "url": "<url from $ARGUMENTS>", "limit": 25 }
```

Optional: `limit` (int, default 25), `offset` (int), `response_mode`.

Present discovered URLs, total count, and path distribution. Suggest crawl targets based on URL patterns.
