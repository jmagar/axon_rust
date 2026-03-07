---
description: Crawl websites with lifecycle job controls
argument-hint: <url> | status <job-id> | cancel <job-id> | list | cleanup | clear | recover
allowed-tools: mcp__axon__axon, Bash
---

Use `mcp__axon__axon` directly.

**Start:**
```json
{ "action": "crawl", "urls": ["<url from $ARGUMENTS>"] }
```
Optional start fields: `max_pages`, `max_depth`, `include_subdomains`, `respect_robots`, `discover_sitemaps`, `render_mode` ("http"|"chrome"|"auto_switch"), `delay_ms`, `response_mode`.

**Lifecycle:**
```json
{ "action": "crawl", "subaction": "status", "job_id": "<uuid>" }
{ "action": "crawl", "subaction": "cancel", "job_id": "<uuid>" }
{ "action": "crawl", "subaction": "list", "limit": 10 }
{ "action": "crawl", "subaction": "cleanup" }
{ "action": "crawl", "subaction": "clear" }
{ "action": "crawl", "subaction": "recover" }
```

Parse `$ARGUMENTS`: URL → start, `status|cancel` + UUID → lifecycle, `list|cleanup|clear|recover` → management.
