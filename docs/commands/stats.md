# axon stats
Last Modified: 2026-03-03

Version: 1.0.0
Last Updated: 20:30:18 | 03/03/2026 EST

Show vector and pipeline statistics for the active collection. Combines Qdrant collection snapshots with Postgres-derived job/command metrics.

## Synopsis

```bash
axon stats [FLAGS]
```

## Arguments

None.

## Required Environment Variables

| Variable | Description |
|----------|-------------|
| `AXON_PG_URL` | Required by global config parsing and used for pipeline/command metrics. |
| `AXON_REDIS_URL` | Required by global config parsing (all commands). |
| `AXON_AMQP_URL` | Required by global config parsing (all commands). |
| `QDRANT_URL` | Qdrant base URL. |

## Flags

All global flags apply. Key flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--collection <name>` | `cortex` | Qdrant collection to inspect. |
| `--json` | `false` | Full stats payload as JSON. |

## Examples

```bash
# Human-readable stats panels
axon stats

# JSON payload
axon stats --json

# Different collection
axon stats --collection docs-local
```

## Output Sections

Human output prints three sections:
- `Vector Stats` (collection status, vector counts, dimension/distance, payload schema)
- `Pipeline Stats` (crawl/embed duration metrics, totals, longest/most-chunks jobs)
- `Command Counts` (counts from `axon_command_runs`)

## Notes

- Qdrant stats are required; if Qdrant endpoints fail, the command fails.
- Postgres metrics are best-effort: if metric queries fail, affected fields become `null`/`n/a` while Qdrant stats still print.
