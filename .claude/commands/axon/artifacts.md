---
description: Inspect Axon artifact files (head, grep, wc, read)
argument-hint: <head|grep|wc|read> <path> [pattern]
allowed-tools: mcp__axon__axon, Bash
---

Use `mcp__axon__axon` directly:

```json
{ "action": "artifacts", "subaction": "head", "path": "<artifact-path>", "limit": 20 }
{ "action": "artifacts", "subaction": "read", "path": "<artifact-path>" }
{ "action": "artifacts", "subaction": "wc", "path": "<artifact-path>" }
{ "action": "artifacts", "subaction": "grep", "path": "<artifact-path>", "pattern": "<regex>" }
```

Optional: `limit` (int), `offset` (int).

Parse `$ARGUMENTS`: first arg is subaction, second is path, third (for grep) is pattern. Paths are relative to artifact root (`.cache/axon-mcp/`).
