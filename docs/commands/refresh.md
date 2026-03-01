# axon refresh
Last Modified: 2026-02-26

Revalidate already-known URLs and keep indexed content fresh without full-site rediscovery crawls.

## One-off Refresh

```bash
# Refresh one URL (synchronous)
axon refresh https://docs.rs/spider --wait true

# Refresh multiple URLs (async enqueue)
axon refresh https://docs.rs/spider https://qdrant.tech/documentation

# Refresh from a seed/domain manifest
axon refresh https://docs.rs --wait true
```

## Schedule Operations

```bash
# Add schedule with explicit interval
axon refresh schedule add docs-medium https://docs.rs --every-seconds 21600

# Add schedule with tier preset
axon refresh schedule add docs-high https://docs.rs --tier high

# List schedules
axon refresh schedule list

# Enable / disable schedule
axon refresh schedule enable docs-high
axon refresh schedule disable docs-high

# Delete schedule
axon refresh schedule delete docs-high

# Trigger one scheduler sweep immediately
axon refresh schedule run-due --json
```

## Tier Presets

| Tier | Seconds |
|------|---------|
| `high` | `1800` |
| `medium` | `21600` |
| `low` | `86400` |

Use `--every-seconds` for custom cadence. If omitted, `--tier medium` behavior maps to `21600`.

## Workers: Scheduler vs Consumer

These are separate processes and should both run in production:

- `axon refresh schedule worker`: scheduler loop. It checks due schedules and enqueues refresh jobs.
- `axon refresh worker`: refresh consumer. It dequeues and executes refresh jobs.

Example split-runtime:

```bash
# Terminal A: scheduler
axon refresh schedule worker

# Terminal B: refresh job consumer
axon refresh worker
```
