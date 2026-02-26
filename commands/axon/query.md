---
description: Semantic search over embedded content in Qdrant
argument-hint: <search-query> [--limit N] [--domain example.com]
allowed-tools: mcp__axon__axon
---

# Semantic Search Query

Call the Axon MCP tool (`axon`) with:
- `action: "query"`
- map `$ARGUMENTS` to schema fields

## Instructions

1. Execute using the Axon MCP tool (`axon`) with action/subaction routing and mapped arguments.
2. Parse ranked results, scores, snippets, and URLs.
3. Present top results with relevance ordering.
4. Summarize dominant themes and next query refinements.

## Expected Output

The command returns:
- ranked semantic matches
- similarity scores
- source URLs and content snippets
