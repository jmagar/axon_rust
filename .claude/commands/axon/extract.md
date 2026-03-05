---
description: Extract structured data with lifecycle job controls
argument-hint: <url1> [url2] | status <job-id> | list | cleanup | clear | recover
allowed-tools: mcp__axon__axon, Bash
---

Use `mcp__axon__axon` directly.

**Start:**
```json
{ "action": "extract", "urls": ["<url1>", "<url2>"] }
```

**Lifecycle:**
```json
{ "action": "extract", "subaction": "status", "job_id": "<uuid>" }
{ "action": "extract", "subaction": "cancel", "job_id": "<uuid>" }
{ "action": "extract", "subaction": "list", "limit": 10 }
{ "action": "extract", "subaction": "cleanup" }
{ "action": "extract", "subaction": "clear" }
{ "action": "extract", "subaction": "recover" }
```

Parse `$ARGUMENTS`: URLs → start, lifecycle keyword + UUID → management.
