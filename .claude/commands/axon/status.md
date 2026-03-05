---
description: Check Axon async queue and worker status
allowed-tools: mcp__axon__axon, Bash
---

Use `mcp__axon__axon` directly:

```json
{ "action": "status" }
```

Present queue depths, worker activity, and pending/running/failed job counts. Highlight blocked or failing lanes first.
