---
description: Embed content with lifecycle job controls
argument-hint: <input> | status <job-id> | cancel <job-id> | list | cleanup | clear | recover
allowed-tools: mcp__axon__axon, Bash
---

Use `mcp__axon__axon` directly.

**Start:**
```json
{ "action": "embed", "input": "<url-or-path from $ARGUMENTS>" }
```

**Lifecycle:**
```json
{ "action": "embed", "subaction": "status", "job_id": "<uuid>" }
{ "action": "embed", "subaction": "cancel", "job_id": "<uuid>" }
{ "action": "embed", "subaction": "list", "limit": 10 }
{ "action": "embed", "subaction": "cleanup" }
{ "action": "embed", "subaction": "clear" }
{ "action": "embed", "subaction": "recover" }
```

Parse `$ARGUMENTS`: content/URL/path → start, lifecycle keyword + UUID → management.
