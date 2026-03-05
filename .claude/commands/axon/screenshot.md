---
description: Capture webpage screenshot via headless Chrome
argument-hint: <url>
allowed-tools: mcp__axon__axon, Bash
---

Use `mcp__axon__axon` directly:

```json
{ "action": "screenshot", "url": "<url from $ARGUMENTS>" }
```

Optional: `viewport` (string, e.g. "1280x720"), `response_mode`.

Present artifact path and image metadata. On failure, suggest checking Chrome via `action: "doctor"`.
