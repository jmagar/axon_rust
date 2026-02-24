# axon youtube

Ingest a YouTube video into Qdrant. Uses `yt-dlp` to download auto-generated or manual transcripts (VTT format), chunks the transcript text, and embeds it into the configured Qdrant collection.

## Synopsis

```bash
axon youtube <url> [FLAGS]
axon youtube <SUBCOMMAND> [ARGS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<url>` | YouTube video URL (`watch?v=<ID>`, `youtu.be/<ID>`, or bare ID) |

**Note:** Only single-video URLs are supported. The video ID is extracted from the `v=` query parameter; `list=` (playlist) parameters are stripped. Pure playlist/channel URLs (no `v=`) fail immediately.

## Required External Dependency

`yt-dlp` must be on `$PATH`. In Docker (axon-workers) it is installed automatically. For local dev install instructions see [`docs/ingest/youtube.md`](../ingest/youtube.md).

If `yt-dlp` is not found, the command fails immediately with a clear error.

## Flags

All global flags apply. Key flags for this command:

| Flag | Default | Description |
|------|---------|-------------|
| `--wait <bool>` | `false` | Block until the ingest job completes. |
| `--collection <name>` | `cortex` | Qdrant collection to embed into. |
| `--json` | `false` | Machine-readable JSON output. |

## Job Subcommands

```bash
axon youtube status <job_id>   # show job status
axon youtube cancel <job_id>   # cancel a pending/running job
axon youtube list              # list recent ingest jobs (last 50)
axon youtube cleanup           # remove failed/canceled jobs older than 30 days
axon youtube clear             # delete all ingest jobs and purge the queue
axon youtube recover           # reclaim stale/interrupted jobs
axon youtube worker            # run an ingest worker inline (blocking)
```

Note: These subcommands operate on all ingest jobs (GitHub, Reddit, YouTube). The `list` output shows `source_type/target` to distinguish them.

## Examples

```bash
# Async (default) — returns immediately with a job ID
axon youtube "https://www.youtube.com/watch?v=dQw4w9WgXcQ"

# Synchronous — blocks until complete
axon youtube "https://www.youtube.com/watch?v=dQw4w9WgXcQ" --wait true

# Short URL form
axon youtube "https://youtu.be/dQw4w9WgXcQ" --wait true

# Video URL with playlist context (playlist param is stripped, only the video is ingested)
axon youtube "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLxxxxxx" --wait true

# Specific collection
axon youtube "https://www.youtube.com/watch?v=rust_talk" --collection rust-talks

# Check status
axon youtube status 550e8400-e29b-41d4-a716-446655440000
```

## Notes

- Videos without transcripts or subtitles (no auto-generated captions) will fail with a clear error.
- Age-restricted or private videos may not be accessible without authentication. See `yt-dlp` documentation for cookie-based auth.
- The ingest job record is stored in `axon_ingest_jobs` with `source_type='youtube'` and `target` set to the canonical `watch?v=<ID>` URL.
- `yt-dlp` writes a temporary `.vtt` file; the pipeline cleans it up after processing.
