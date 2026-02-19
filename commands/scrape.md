---
description: Scrape a single URL for LLM-ready content
argument-hint: <url> [options]
allowed-tools: Bash(axon *)
---

# Scrape Single URL

Execute the Axon scrape command with the provided arguments:

```bash
axon scrape $ARGUMENTS
```

## Instructions

1. **Execute the command** using the Bash tool with the arguments provided
2. **Parse the response** to extract:
   - Scraped content (Markdown format)
   - Metadata (title, description, URL)
   - Embedding confirmation
3. **Present the results** in a clear, formatted way
4. **Confirm** that content has been embedded into Qdrant for semantic search

## Expected Output

The command returns JSON containing:
- `content`: LLM-ready Markdown content
- `metadata`: Page metadata (title, description, etc.)
- `embedded`: Confirmation of Qdrant embedding

Present the scraped content and confirm successful embedding.
