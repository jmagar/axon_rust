# axon refresh
Last Modified: 2026-03-03

Revalidate already-known URLs and keep indexed content fresh without full rediscovery crawls.

## Synopsis

```bash
axon refresh <url...> [FLAGS]
axon refresh <SUBCOMMAND> [ARGS]
```

## One-Off Refresh

```bash
# Refresh one URL synchronously
axon refresh https://docs.rs/spider --wait true

# Refresh multiple URLs asynchronously (default)
axon refresh https://docs.rs/spider https://qdrant.tech/documentation

# Refresh using manifest fallback from a seed domain
axon refresh https://docs.rs --wait true
```

URL resolution behavior:

- Uses positional URLs when provided
- If no URLs are provided and `--start-url` is set, reads seed manifest URLs
- If a single positional URL looks like a bare domain seed, tries manifest fallback for that seed

## Job Subcommands

```bash
axon refresh status <job_id>
axon refresh cancel <job_id>
axon refresh errors <job_id>
axon refresh list
axon refresh cleanup
axon refresh clear
axon refresh recover
axon refresh worker
```

## Schedule Subcommands

```bash
axon refresh schedule add <name> [seed_url] [--every-seconds N | --tier high|medium|low] [--urls "u1,u2"]
axon refresh schedule list
axon refresh schedule enable <name>
axon refresh schedule disable <name>
axon refresh schedule delete <name>
axon refresh schedule worker
axon refresh schedule run-due [--batch N]
```

`refresh schedule` is a compatibility surface backed by top-level watch definitions (`task_type=refresh`). You can continue using the same commands/scripts; schedule rows are bridged to watch scheduler state.

`refresh schedule add` requires at least one of:

- `[seed_url]`
- `--urls <csv>`

## Tier Presets

| Tier | Seconds |
|------|---------|
| `high` | `1800` |
| `medium` | `21600` |
| `low` | `86400` |

## Flags

All global flags apply. Key flags for this command:

| Flag | Default | Description |
|------|---------|-------------|
| `--wait <bool>` | `false` | Run refresh inline and block until complete. |
| `--json` | `false` | Machine-readable output. |
| `--yes` | `false` | Skip destructive confirmation (for `clear`). |

## Examples

```bash
# Async enqueue
axon refresh https://docs.rs/spider

# Sync refresh from manifest seed
axon refresh https://docs.rs --wait true

# Create schedule using tier preset
axon refresh schedule add docs-high https://docs.rs --tier high

# Create schedule using explicit URL list
axon refresh schedule add docs-explicit --urls "https://docs.rs/spider,https://qdrant.tech/documentation"

# Run one due-schedule sweep now
axon refresh schedule run-due --batch 50 --json
```

## Notes

- `refresh worker` is the refresh job consumer lane.
- `refresh schedule worker` is the scheduler loop that periodically runs due-schedule sweeps.
- `schedule run-due` dispatches due schedules immediately and reports claimed/dispatched/skipped/failed counts.
- For new automation, prefer `axon watch ...` for scheduler-first workflows and treat `refresh schedule` as a compatibility alias.
