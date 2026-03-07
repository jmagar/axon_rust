---
description: List indexed source URLs with chunk counts
argument-hint: [--limit N]
allowed-tools: mcp__axon__axon, Bash
---

Use `mcp__axon__axon` directly:

```json
{ "action": "sources", "limit": 20 }
```

Optional: `limit` (int), `offset` (int), `response_mode`.

Present source URLs with chunk counts.
