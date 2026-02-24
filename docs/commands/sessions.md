# axon sessions

Ingest AI session exports into Qdrant. Reads local session history files from Claude, Codex, and Gemini in their standard export formats, chunks the conversation content, and embeds it into the configured Qdrant collection.

**Incremental:** Files are tracked by path, mtime, and size. Re-running `sessions` only processes new or changed files — already-indexed sessions are skipped.

## Synopsis

```bash
axon sessions [FLAGS]
```

## Arguments

None. Session file paths are hardcoded per provider:

| Provider | Scan path |
|----------|-----------|
| Claude | `~/.claude/projects/` |
| Codex | `~/.codex/sessions/` |
| Gemini | `~/.gemini/history/`, `~/.gemini/tmp/` |

## Flags

All global flags apply. Key flags for this command:

| Flag | Default | Description |
|------|---------|-------------|
| `--collection <name>` | `cortex` | Qdrant collection to embed into. |
| `--json` | `false` | Machine-readable JSON output. |

Note: `sessions` runs **synchronously** and does not support `--wait`. It does not enqueue a job — it runs inline and returns when complete.

## Examples

```bash
# Ingest sessions (synchronous, incremental)
axon sessions

# Into a specific collection
axon sessions --collection ai-history

# JSON output
axon sessions --json
```

> For supported formats, state tracker details, and troubleshooting see [`docs/ingest/sessions.md`](../ingest/sessions.md).

## Notes

- Unlike `github`, `reddit`, and `youtube`, the `sessions` command does not use the AMQP queue and does not create an `axon_ingest_jobs` record. It runs entirely in-process.
- This is intentional: session files are local, the operation is fast, and async queuing adds no benefit for local I/O.
- The state tracker table (`axon_session_ingest_state`) is auto-created in Postgres on first run.
