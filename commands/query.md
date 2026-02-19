---
description: Semantic search over embedded content in Qdrant
argument-hint: "<search query>" [--limit N]
allowed-tools: Bash(axon *)
---

# Semantic Search Query

Execute the Axon query command with the provided arguments:

```bash
axon query $ARGUMENTS
```

`--limit` is honored via the global CLI flag and controls how many hits are requested from Qdrant.

## Expected Output

Plaintext mode:
- Header: `Query Results for "..."`
- Count line: `Showing N`
- Ranked bullet list with `score`, `url`, and a snippet preview

JSON mode:
- One JSON object per hit:
  - `rank`
  - `score`
  - `url`
  - `snippet`
