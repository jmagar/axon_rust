# axon retrieve
Last Modified: 2026-03-03

Version: 1.0.0
Last Updated: 20:30:18 | 03/03/2026 EST

Retrieve stored document content from Qdrant by URL. The command resolves URL variants, fetches matching chunks, orders by `chunk_index`, and prints reconstructed text.

## Synopsis

```bash
axon retrieve <url> [FLAGS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<url>` | URL (or URL-like target) used for payload `url` lookup. |

## Required Environment Variables

| Variable | Description |
|----------|-------------|
| `AXON_PG_URL` | Required by global config parsing (all commands). |
| `AXON_REDIS_URL` | Required by global config parsing (all commands). |
| `AXON_AMQP_URL` | Required by global config parsing (all commands). |
| `QDRANT_URL` | Qdrant base URL. |

## Flags

All global flags apply. Key flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--collection <name>` | `cortex` | Qdrant collection to read from. |
| `--json` | `false` | Outputs `{url, chunks, content}` JSON. |

Note: `retrieve` runs synchronously and does not enqueue jobs.

## Examples

```bash
# Retrieve indexed content by source URL
axon retrieve https://docs.rs/spider

# Specific collection
axon retrieve https://qdrant.tech/documentation --collection docs-local

# JSON output
axon retrieve https://docs.rs/spider --json
```

## Notes

- Lookup tries normalized URL variants (`target`, normalized, no-trailing-slash, trailing-slash).
- Retrieved points are capped at 500 chunks per request (hard ceiling).
- If no matching payload URL is found, output is `No content found for URL: ...`.
