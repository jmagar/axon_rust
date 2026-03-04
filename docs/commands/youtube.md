# axon youtube
Last Modified: 2026-03-03

Ingest a YouTube transcript into Qdrant using `yt-dlp` subtitle extraction.

## Synopsis

```bash
axon youtube <url-or-id> [FLAGS]
axon youtube <SUBCOMMAND> [ARGS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<url-or-id>` | YouTube video URL or bare 11-character video ID. |

Accepted URL shapes include `watch?v=...`, `youtu.be/...`, `/embed/...`, `/shorts/...`, `/v/...`.

## Required External Dependency

`yt-dlp` must be installed and available on `$PATH`.

## Flags

All global flags apply. Key flags for this command:

| Flag | Default | Description |
|------|---------|-------------|
| `--wait <bool>` | `false` | Block until ingestion completes; otherwise enqueue async job. |
| `--collection <name>` | `cortex` | Target Qdrant collection. |
| `--json` | `false` | Machine-readable output. |

## Job Subcommands

```bash
axon youtube status <job_id>
axon youtube cancel <job_id>
axon youtube errors <job_id>
axon youtube list
axon youtube cleanup
axon youtube clear
axon youtube recover
axon youtube worker
```

These subcommands operate on the shared ingest queue across source types.

## Examples

```bash
# Async enqueue (default)
axon youtube "https://www.youtube.com/watch?v=dQw4w9WgXcQ"

# Sync run
axon youtube "https://youtu.be/dQw4w9WgXcQ" --wait true

# Bare video ID
axon youtube dQw4w9WgXcQ --wait true
```

## Notes

- Ingest currently resolves a single video ID and normalizes to `https://www.youtube.com/watch?v=<id>`.
- If subtitles/captions are unavailable, ingestion fails with a clear error.
- Job records are stored in `axon_ingest_jobs` with `source_type='youtube'`.
