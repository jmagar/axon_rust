# axon sessions

Ingest AI session exports into Qdrant. Reads local session history files from Claude, Codex, and Gemini in their standard export formats, chunks the conversation content, and embeds it into the configured Qdrant collection.

## Synopsis

```bash
axon sessions [FLAGS]
```

## Arguments

None. The command reads session export paths from the system configuration and environment.

## Required Environment Variables

None — session ingestion reads local files and does not call external APIs (beyond TEI for embedding).

## Flags

All global flags apply. Key flags for this command:

| Flag | Default | Description |
|------|---------|-------------|
| `--collection <name>` | `cortex` | Qdrant collection to embed into. |
| `--json` | `false` | Machine-readable JSON output. |

Note: `sessions` runs synchronously and does not support `--wait`. It does not enqueue a job — it runs inline and returns when complete.

## Examples

```bash
# Ingest sessions (synchronous)
axon sessions

# Into a specific collection
axon sessions --collection ai-history

# JSON output
axon sessions --json
```

## Ingest Scope

The pipeline discovers and processes session files from standard export locations for:

- **Claude** — `~/.claude/` conversation exports
- **Codex** — Codex session history files
- **Gemini** — Gemini session exports

Supported formats include JSON exports and markdown-based conversation logs. Each session is chunked and embedded with metadata (session ID, date, AI provider).

## Notes

- Unlike `github`, `reddit`, and `youtube`, `sessions` does not use the AMQP queue and does not create an `axon_ingest_jobs` record. It runs entirely in-process.
- This is intentional: session files are local, the operation is typically fast, and there is no benefit to async queuing for local I/O.
- If you have a very large history (hundreds of sessions), the command may take a few minutes. Run with `--json` to see progress output.
- Sessions that have already been embedded are not deduplicated automatically. Running `sessions` again will re-embed existing content. Use `axon dedupe` afterward if needed.
