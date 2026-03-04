# axon status
Last Modified: 2026-03-03

Show local job state across crawl, extract, embed, ingest, and refresh queues.

## Synopsis

```bash
axon status [FLAGS]
```

## Flags

All global flags apply. Key flags for this command:

| Flag | Default | Description |
|------|---------|-------------|
| `--json` | `false` | Print machine-readable JSON status payload. |
| `--reclaimed` | `false` | Show only watchdog-reclaimed stale-running failures. |

## Output

Human output prints grouped sections and status breakdowns for:

- Crawls
- Refresh
- Embeds
- Ingests
- Extracts

JSON output shape:

```json
{
  "local_crawl_jobs": [...],
  "local_extract_jobs": [...],
  "local_embed_jobs": [...],
  "local_ingest_jobs": [...],
  "local_refresh_jobs": [...]
}
```

## Examples

```bash
# Human summary
axon status

# JSON payload
axon status --json

# Only watchdog-reclaimed stale jobs
axon status --reclaimed --json
```

## Notes

- `status` loads up to 20 recent jobs per queue family.
- By default, watchdog-reclaimed failures are hidden. `--reclaimed` flips to reclaimed-only mode.
- This command is read-only and does not enqueue or mutate jobs.
