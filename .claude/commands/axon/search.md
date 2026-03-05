---
description: Search the web and enqueue crawl jobs from results
argument-hint: <query> [--limit N]
allowed-tools: mcp__axon__axon, Bash
---

Use `mcp__axon__axon` directly:

```json
{ "action": "search", "query": "<query from $ARGUMENTS>", "limit": 10 }
```

Optional: `limit` (int), `offset` (int), `search_time_range` ("day"|"week"|"month"|"year"), `response_mode`.

Results are auto-indexed. Present top hits with URLs/snippets and confirm crawl enqueue status.
