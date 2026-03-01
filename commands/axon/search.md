---
description: Search the web and enqueue crawl jobs from results
argument-hint: <query> [--limit N]
allowed-tools: mcp__axon__axon
---

# Web Search

Call the Axon MCP tool (`axon`) with:
- `action: "search"`
- map `$ARGUMENTS` to schema fields

## Instructions

1. Execute using the Axon MCP tool (`axon`) with action/subaction routing and mapped arguments.
2. Parse ranked results, snippets, and URLs.
3. Confirm crawl job enqueue behavior from returned results.
4. Present top hits and next actions.

## Expected Output

The command returns:
- ranked search results
- result URLs/snippets
- crawl enqueue information
