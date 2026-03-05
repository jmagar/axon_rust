---
description: Scrape one or more URLs to markdown output
argument-hint: <url>
allowed-tools: mcp__axon__axon, Bash
---

Use `mcp__axon__axon` directly:

```json
{ "action": "scrape", "url": "<url from $ARGUMENTS>" }
```

Optional: `response_mode`.

Content is auto-embedded into Qdrant. Report markdown output, artifact path, and embed status. On failure, suggest `action: "doctor"`.
