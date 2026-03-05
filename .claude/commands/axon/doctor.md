---
description: Run connectivity diagnostics for Axon dependencies
allowed-tools: mcp__axon__axon, Bash
---

Use `mcp__axon__axon` directly:

```json
{ "action": "doctor" }
```

Checks: Postgres, Redis, RabbitMQ, Qdrant, TEI, LLM, Chrome. Report failures first with probable cause and remediation.
