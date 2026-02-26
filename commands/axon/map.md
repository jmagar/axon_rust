---
description: Discover URLs on a site without scraping content
argument-hint: <url> [--limit N]
allowed-tools: mcp__axon__axon
---

# Map Website URLs

Call the Axon MCP tool (`axon`) with:
- `action: "map"`
- map `$ARGUMENTS` to schema fields

## Instructions

1. Execute using the Axon MCP tool (`axon`) with action/subaction routing and mapped arguments.
2. Parse discovered URLs and coverage summary.
3. Present domain/path distribution and crawl recommendations.

## Expected Output

The command returns:
- discovered URLs
- counts by scope/path
- mapping summary
