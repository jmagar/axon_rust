# axon watch
Last Modified: 2026-03-05

Top-level recurring scheduler definitions and run history.

## Synopsis

```bash
axon watch <SUBCOMMAND> [ARGS]
```

## Subcommands

```bash
axon watch create <name> --task-type <type> --every-seconds <n> [--task-payload <json>]
axon watch list
axon watch run-now <watch_id>
axon watch history <watch_id> [--limit <n>]
```

## Task Payloads

Current worker dispatch support is `task_type=refresh`.

Refresh payload shape:

```json
{"urls":["https://example.com/docs","https://example.com/api"]}
```

If `urls` is empty, `run-now` records a run but does not dispatch a downstream refresh job.

## Examples

```bash
# Create a 5-minute refresh watch
axon watch create docs-refresh \
  --task-type refresh \
  --every-seconds 300 \
  --task-payload '{"urls":["https://docs.rs/spider"]}'

# List watch definitions
axon watch list --json

# Force one immediate run
axon watch run-now <watch_id> --json

# Inspect recent run history
axon watch history <watch_id> --limit 20
```

## Relationship to refresh schedule

`axon refresh schedule ...` remains available as a compatibility interface and is backed by watch definitions with `task_type=refresh`.
