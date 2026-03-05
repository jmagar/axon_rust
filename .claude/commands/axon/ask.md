---
description: Ask questions grounded in indexed docs (AI-powered Q&A)
argument-hint: <question>
allowed-tools: mcp__axon__axon, Bash
---

Use `mcp__axon__axon` directly:

```json
{ "action": "ask", "query": "<question from $ARGUMENTS>" }
```

Optional: `limit` (int), `response_mode` ("path"|"inline"|"both").

Present the answer first, then supporting sources with relevance scores. Default `response_mode` is `"path"` — use `action: "artifacts", subaction: "read"` to inspect full output.
