---
description: Run connectivity diagnostics for Axon dependencies
argument-hint: [--json]
allowed-tools: mcp__axon__axon
---

# Run Axon Doctor

Call the Axon MCP tool (`axon`) with:
- `action: "doctor"`
- map `$ARGUMENTS` to schema fields

## Instructions

1. Execute using the Axon MCP tool (`axon`) with action/subaction routing and mapped arguments.
2. Parse each service check result.
3. Report failures first with probable cause.

## Expected Output

The command returns diagnostics for:
- Postgres
- Redis
- RabbitMQ
- Qdrant
- TEI
- LLM endpoint
