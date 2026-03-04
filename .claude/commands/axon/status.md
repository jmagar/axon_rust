---
description: Check Axon async queue and worker status
argument-hint: [--json]
allowed-tools: mcp__axon__axon
---

# Check Axon Status

Call the Axon MCP tool (`axon`) with:
- `action: "status"`
- map `$ARGUMENTS` to schema fields

## Instructions

1. Execute using the Axon MCP tool (`axon`) with action/subaction routing and mapped arguments.
2. Parse queue and worker health information.
3. Present blocked or failing lanes first.

## Expected Output

The command returns status information including:
- queue depth
- worker activity
- pending/running/failed jobs
