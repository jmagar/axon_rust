# axon sessions
Last Modified: 2026-03-03

Ingest local AI session history (Claude, Codex, Gemini) into Qdrant.

## Synopsis

```bash
axon sessions [FLAGS]
axon sessions <SUBCOMMAND> [ARGS]
```

## Provider Sources

| Provider | Path |
|----------|------|
| Claude | `~/.claude/projects/` |
| Codex | `~/.codex/sessions/` |
| Gemini | `~/.gemini/history/`, `~/.gemini/tmp/` |

## Flags

All global flags apply. Sessions-specific and key flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--claude` | `false` | Include Claude sessions. |
| `--codex` | `false` | Include Codex sessions. |
| `--gemini` | `false` | Include Gemini sessions. |
| `--project <name>` | — | Case-insensitive substring filter on project name. |
| `--wait <bool>` | `false` | Block until ingestion completes; otherwise enqueue async ingest job. |
| `--collection <name>` | `cortex` | Target Qdrant collection. |
| `--json` | `false` | Machine-readable output. |

Provider selection rule:
- If none of `--claude/--codex/--gemini` is set, all providers are ingested.

## Job Subcommands

```bash
axon sessions status <job_id>
axon sessions cancel <job_id>
axon sessions errors <job_id>
axon sessions list
axon sessions cleanup
axon sessions clear
axon sessions recover
axon sessions worker
```

These subcommands operate on the shared ingest queue across source types.

## Examples

```bash
# Async enqueue (default) for all providers
axon sessions

# Sync run for Codex only
axon sessions --codex --wait true

# Claude + Gemini filtered to project name match
axon sessions --claude --gemini --project axon --wait true

# Check job status
axon sessions status 550e8400-e29b-41d4-a716-446655440000
```

## Notes

- Session ingest uses an incremental state tracker table: `axon_session_ingest_state`.
- Job records for queued runs are stored in `axon_ingest_jobs` with `source_type='sessions'`.
