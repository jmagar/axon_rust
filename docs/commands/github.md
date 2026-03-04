# axon github
Last Modified: 2026-03-03

Ingest a GitHub repository into Qdrant. Fetches repository metadata, docs/files, issues, pull requests, and wiki pages, then chunks and embeds content into the configured collection.

## Synopsis

```bash
axon github <owner/repo> [FLAGS]
axon github <SUBCOMMAND> [ARGS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<owner/repo>` | Repository identifier (for example `rust-lang/rust`). GitHub URL forms are also accepted. |

## Required Environment Variables

None are strictly required for public repositories, but:

| Variable | Description |
|----------|-------------|
| `GITHUB_TOKEN` | Optional for public repos (higher rate limits), required for private repos. |

## Flags

All global flags apply. GitHub-specific and key flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--include-source <bool>` | `false` | Also index source-code files (not only docs/issues/PRs/wiki). |
| `--wait <bool>` | `false` | Block until ingestion completes; otherwise enqueue async job. |
| `--collection <name>` | `cortex` | Target Qdrant collection. |
| `--json` | `false` | Machine-readable output. |

## Job Subcommands

```bash
axon github status <job_id>
axon github cancel <job_id>
axon github errors <job_id>
axon github list
axon github cleanup
axon github clear
axon github recover
axon github worker
```

These job subcommands operate on the shared ingest queue across source types (`github`, `reddit`, `youtube`, `sessions`).

## Examples

```bash
# Async enqueue (default)
axon github rust-lang/rust

# Sync run
axon github rust-lang/rust --wait true

# Include source code files
axon github tokio-rs/tokio --include-source true --wait true

# URL form is accepted
axon github https://github.com/rust-lang/rust.git --wait true

# Inspect job
axon github status 550e8400-e29b-41d4-a716-446655440000
```

## Notes

- Private repos need a token with repo access.
- Large repos can produce many chunks and take significant time.
- Job records are stored in `axon_ingest_jobs` with `source_type='github'`.
