---
description: Show Qdrant collection and indexing statistics
argument-hint: [--collection name]
allowed-tools: mcp__axon__axon
---

# Collection Stats

Call the Axon MCP tool (`axon`) with:
- `action: "stats"`
- map `$ARGUMENTS` to schema fields

## Instructions

1. Execute using the Axon MCP tool (`axon`) with action/subaction routing and mapped arguments.
2. Parse collection-level metrics.
3. Present size/capacity signals and notable changes.

## Expected Output

The command returns stats including:
- collection name
- vector count
- storage/index details
