# axon ingest
Last Modified: 2026-03-03

Manage shared ingest jobs and the ingest worker lane.

`ingest` is a control-plane alias only. It does not accept source targets directly.

## Synopsis

```bash
axon ingest <SUBCOMMAND> [ARGS]
```

## Subcommands

```bash
axon ingest status <job_id>   # show one ingest job
axon ingest cancel <job_id>   # cancel pending/running job
axon ingest errors <job_id>   # show job error text
axon ingest list              # list recent ingest jobs (last 50)
axon ingest cleanup           # remove failed/canceled + old completed jobs
axon ingest clear             # delete all ingest jobs and purge ingest queue
axon ingest recover           # reclaim stale/interrupted jobs
axon ingest worker            # run ingest worker inline (blocking)
```

## Flags

All global flags apply. Key flags for this command family:

| Flag | Default | Description |
|------|---------|-------------|
| `--yes` | `false` | Skip confirmation prompt for destructive `clear`. |
| `--json` | `false` | Machine-readable output for status/list/control responses. |

## Examples

```bash
# List recent ingest jobs
axon ingest list

# Inspect one job
axon ingest status 550e8400-e29b-41d4-a716-446655440000

# Reclaim stale jobs
axon ingest recover

# Clear queue + table without prompt
axon ingest clear --yes
```

## Notes

- Source ingestion is handled by `axon github`, `axon reddit`, `axon youtube`, and `axon sessions`.
- `list` includes all ingest source types via `source_type/target` fields.
- If no subcommand is provided, command fails with guidance to use a subcommand.
