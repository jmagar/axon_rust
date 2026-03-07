---
description: Ingest external sources with lifecycle job controls
argument-hint: <github|reddit|youtube|sessions> <target> | status <job-id> | list | cleanup | clear | recover
allowed-tools: mcp__axon__axon, Bash
---

Use `mcp__axon__axon` directly.

**Start:**
```json
{ "action": "ingest", "source_type": "github", "target": "owner/repo" }
{ "action": "ingest", "source_type": "reddit", "target": "rust" }
{ "action": "ingest", "source_type": "youtube", "target": "https://youtube.com/watch?v=..." }
{ "action": "ingest", "source_type": "sessions", "target": "./exports" }
```

**Lifecycle:**
```json
{ "action": "ingest", "subaction": "status", "job_id": "<uuid>" }
{ "action": "ingest", "subaction": "cancel", "job_id": "<uuid>" }
{ "action": "ingest", "subaction": "list", "limit": 10 }
{ "action": "ingest", "subaction": "cleanup" }
{ "action": "ingest", "subaction": "clear" }
{ "action": "ingest", "subaction": "recover" }
```

Parse `$ARGUMENTS`: first arg is `source_type`, second is `target`. Lifecycle keywords route to management subactions.
