---
description: List indexed domains and document stats
argument-hint: [--limit N]
allowed-tools: mcp__axon__axon, Bash
---

Use `mcp__axon__axon` directly:

```json
{ "action": "domains", "limit": 20 }
```

Optional: `limit` (int), `offset` (int), `response_mode`.

Present domains sorted by indexed URL count, highest first.
