---
description: Run Tavily AI research with synthesized summary
argument-hint: <query>
allowed-tools: mcp__axon__axon, Bash
---

Use `mcp__axon__axon` directly:

```json
{ "action": "research", "query": "<query from $ARGUMENTS>" }
```

Optional: `limit` (int), `offset` (int), `search_time_range` ("day"|"week"|"month"|"year"), `response_mode`.

Present synthesized findings, source quality assessment, and knowledge gaps. Results are auto-indexed.
