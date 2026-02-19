---
description: Retrieve full document from vector database by URL
argument-hint: <url>
allowed-tools: Bash(axon *)
---

# Retrieve Full Document

Execute the Axon retrieve command with the provided arguments:

```bash
axon retrieve $ARGUMENTS
```

The command normalizes URL input and also tries trailing-slash variants so
`example.com/docs`, `https://example.com/docs`, and `https://example.com/docs/`
resolve to the same stored document when possible.

## Expected Output

Plaintext mode:
- Header: `Retrieve Result for <url>`
- Chunk count
- Reconstructed content (chunks ordered by `chunk_index`)

JSON mode:
- `url`
- `chunks`
- `content`
