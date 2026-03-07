---
description: Semantic search over embedded content in Qdrant
argument-hint: <search-query> [--limit N]
allowed-tools: mcp__axon__axon, Bash
---

Use `mcp__axon__axon` directly:

```json
{ "action": "query", "query": "<search terms from $ARGUMENTS>", "limit": 10 }
```

Optional: `limit` (int, default 10), `offset` (int), `response_mode`.

Present ranked results with scores, source URLs, and content snippets. Summarize themes and suggest refinements if sparse.
