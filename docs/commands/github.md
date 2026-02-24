# axon github

Ingest a GitHub repository into Qdrant. Fetches repository metadata, code files, issues, pull requests, and wiki pages via the GitHub REST API, chunks the content, and embeds it into the configured Qdrant collection.

## Synopsis

```bash
axon github <owner/repo> [FLAGS]
axon github <SUBCOMMAND> [ARGS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<owner/repo>` | GitHub repository in `owner/repo` format (e.g. `rust-lang/rust`) |

## Required Environment Variables

None are strictly required for public repositories, but:

| Variable | Description |
|----------|-------------|
| `GITHUB_TOKEN` | Personal access token. Optional for public repos. Without it, GitHub API rate limits apply (60 req/hr unauthenticated vs. 5000 req/hr authenticated). Required for private repos. |

Set in `.env`:

```bash
GITHUB_TOKEN=ghp_yourtoken
```

## Flags

All global flags apply. Key flags for this command:

| Flag | Default | Description |
|------|---------|-------------|
| `--wait <bool>` | `false` | Block until the ingest job completes. Without this, the job is enqueued and returns immediately. |
| `--collection <name>` | `cortex` | Qdrant collection to embed into. |
| `--yes` | `false` | Skip confirmation prompts. |
| `--json` | `false` | Machine-readable JSON output. |

## Job Subcommands

```bash
axon github status <job_id>   # show job status
axon github cancel <job_id>   # cancel a pending/running job
axon github list              # list recent ingest jobs (last 50)
axon github cleanup           # remove failed/canceled jobs older than 30 days
axon github clear             # delete all ingest jobs and purge the queue
axon github recover           # reclaim stale/interrupted jobs
axon github worker            # run an ingest worker inline (blocking)
```

Note: These subcommands operate on all ingest jobs (GitHub, Reddit, YouTube), not just GitHub jobs. The `list` output shows `source_type/target` to distinguish them.

## Examples

```bash
# Async (default) — returns immediately with a job ID
axon github rust-lang/rust

# Synchronous — blocks until complete
axon github rust-lang/rust --wait true

# Specific collection
axon github tokio-rs/tokio --wait true --collection rust-libs

# Check job status
axon github status 550e8400-e29b-41d4-a716-446655440000

# JSON output
axon github list --json
```

## Ingest Scope

The ingest pipeline fetches:

- Repository metadata (name, description, topics, language, license)
- README and documentation files (`.md`, `.rst`, `.txt`)
- Source code files (configurable via `github_include_source` flag)
- Issues (open and closed, titles + bodies + labels)
- Pull requests (open and closed, titles + bodies)
- Wiki pages (if the wiki is enabled and public)

All content is chunked (2000 chars, 200-char overlap) and embedded via TEI before upsert into Qdrant.

> For implementation details, rate limit guidance, and troubleshooting see [`docs/ingest/github.md`](../ingest/github.md).

## Notes

- Private repositories require a `GITHUB_TOKEN` with at least `repo` scope.
- Very large repositories (e.g. `linux/linux`) may produce thousands of chunks and take several minutes.
- The ingest job records are stored in the `axon_ingest_jobs` Postgres table with `source_type='github'` and `target='owner/repo'`.
- If `yt-dlp` is not installed, this command is unaffected — it only uses the GitHub API.
