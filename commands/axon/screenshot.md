---
description: Capture webpage screenshot through Axon action
argument-hint: <url>
allowed-tools: mcp__axon__axon
---

# Capture Screenshot

Call the Axon MCP tool (`axon`) with:
- `action: "screenshot"`
- map `$ARGUMENTS` to schema fields

## Instructions

1. Execute using the Axon MCP tool (`axon`) with action/subaction routing and mapped arguments.
2. Parse screenshot artifact path and metadata.
3. Report navigation/render failures clearly.

## Expected Output

The command returns:
- screenshot artifact path
- image metadata
- error diagnostics (if any)
