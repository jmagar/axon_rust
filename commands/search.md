---
description: Search the web and return a list of result URLs
argument-hint: "<search query>" [--limit N]
allowed-tools: Bash(axon *)
---

# Web Search

Execute the Axon search command with the provided arguments:

```bash
axon search $ARGUMENTS
```

## Instructions

1. **Execute the command** using the Bash tool with the arguments provided
2. **Parse the response** to extract:
   - Result URLs from the search
3. **Present the results** including:
   - List of result URLs

## Behavior

`search` queries DuckDuckGo and returns a plain list of result URLs. It does **not** auto-scrape or index results — use `/axon:scrape` or `/axon:batch` to fetch and embed content from the returned URLs.

Use `--limit N` (default: 10) to control the number of URLs returned.

## Expected Output

The command prints result URLs to stdout, one per line:

```
  • https://example.com/result-1
  • https://example.com/result-2
  ...
```

There is no JSON output mode for this command.

## Key Differences from /axon:query

- **`/axon:search`**: Searches the **web** via DuckDuckGo and returns a URL list
- **`/axon:query`**: Searches your **existing knowledge base** in Qdrant (semantic search)

Use `/axon:search` to discover URLs, then `/axon:scrape` or `/axon:batch` to fetch and index the content.
Use `/axon:query` to search what you've already indexed.
