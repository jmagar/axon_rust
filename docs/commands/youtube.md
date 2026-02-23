# axon youtube

Ingest a YouTube video, playlist, or channel into Qdrant. Uses `yt-dlp` to download auto-generated or manual transcripts (VTT format), chunks the transcript text, and embeds it into the configured Qdrant collection.

## Synopsis

```bash
axon youtube <url> [FLAGS]
axon youtube <SUBCOMMAND> [ARGS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<url>` | YouTube video, playlist, or channel URL |

## Required External Dependency

`yt-dlp` must be installed and accessible in `$PATH`:

```bash
# Install
pip install yt-dlp
# or
brew install yt-dlp

# Verify
yt-dlp --version
```

If `yt-dlp` is not found, the command fails immediately with a clear error.

## Optional Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENAI_BASE_URL` | LLM endpoint used for optional transcript post-processing or summarization. Not required for basic transcript ingestion. |
| `OPENAI_MODEL` | Model name for optional LLM step. |

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

Note: These subcommands operate on all ingest jobs (github, reddit, youtube). The `list` output shows `source_type/target` to distinguish them.

## Examples

```bash
# Async (default) — returns immediately with a job ID
axon youtube "https://www.youtube.com/watch?v=dQw4w9WgXcQ"

# Synchronous — blocks until complete
axon youtube "https://www.youtube.com/watch?v=dQw4w9WgXcQ" --wait true

# Playlist
axon youtube "https://www.youtube.com/playlist?list=PLxxxxxx" --wait true

# Specific collection
axon youtube "https://www.youtube.com/watch?v=rust_talk" --collection rust-talks

# Check status
axon youtube status 550e8400-e29b-41d4-a716-446655440000
```

## Ingest Scope

The ingest pipeline:

1. Invokes `yt-dlp` to download the VTT subtitle file (auto-generated or manual)
2. Parses VTT format into plain text, stripping timestamps and cue metadata
3. Chunks the transcript text (2000 chars, 200-char overlap)
4. Embeds each chunk via TEI and upserts into Qdrant

Video title, channel name, and URL are stored as point metadata.

## Notes

- Videos without transcripts or subtitles (no auto-generated captions) will fail. `yt-dlp` must be able to extract a VTT file.
- Age-restricted or private videos may not be accessible without authentication. See `yt-dlp` documentation for cookie-based auth.
- Playlists are processed sequentially. Large playlists (100+ videos) can take a long time.
- The ingest job records are stored in the `axon_ingest_jobs` Postgres table with `source_type='youtube'` and `target` set to the video/playlist/channel URL.
- `yt-dlp` writes a temporary `.vtt` file; the ingest pipeline cleans it up after processing.
