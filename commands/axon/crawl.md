---
description: Crawl websites with lifecycle job controls
argument-hint: <url> [options] | status <job-id> | cancel <job-id> | list | cleanup | clear | recover
allowed-tools: mcp__axon__axon
---

# Crawl Website Content

Call the Axon MCP tool (`axon`) with:
- `action: "crawl"`
- `subaction: "start|status|cancel|list|cleanup|clear|recover"` from `$ARGUMENTS`
- map remaining `$ARGUMENTS` to schema fields

## Instructions

1. Execute using the Axon MCP tool (`axon`) with action/subaction routing and mapped arguments.
2. Handle both start and lifecycle operations (`status|cancel|list|cleanup|clear|recover`).
3. Parse job ID, status transitions, progress, and errors.
4. Present crawl coverage summary and failures.

## Expected Output

The command returns:
- crawl job identifiers
- lifecycle status/progress
- URL/page results and crawl stats
