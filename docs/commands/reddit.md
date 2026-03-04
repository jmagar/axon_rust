# axon reddit
Last Modified: 2026-03-03

Ingest subreddit content or a single thread into Qdrant using Reddit OAuth2 client credentials.

## Synopsis

```bash
axon reddit <target> [FLAGS]
axon reddit <SUBCOMMAND> [ARGS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<target>` | Subreddit name (for example `rust`) or full Reddit thread URL. |

## Required Environment Variables

| Variable | Description |
|----------|-------------|
| `REDDIT_CLIENT_ID` | OAuth2 client ID from Reddit app settings. |
| `REDDIT_CLIENT_SECRET` | OAuth2 client secret from the same app. |

## Flags

All global flags apply. Reddit-specific and key flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--sort` | `hot` | Subreddit sort: `hot`, `top`, `new`, `rising`. |
| `--time` | `day` | Time range for `top`: `hour`, `day`, `week`, `month`, `year`, `all`. |
| `--max-posts` | `25` | Maximum posts to fetch (`0` = unlimited). |
| `--min-score` | `0` | Minimum score threshold for posts/comments. |
| `--depth` | `2` | Comment traversal depth. |
| `--scrape-links <bool>` | `false` | Scrape linked URLs from link posts (where supported). |
| `--wait <bool>` | `false` | Block until ingestion completes; otherwise enqueue async job. |
| `--collection <name>` | `cortex` | Target Qdrant collection. |
| `--json` | `false` | Machine-readable output. |

## Job Subcommands

```bash
axon reddit status <job_id>
axon reddit cancel <job_id>
axon reddit errors <job_id>
axon reddit list
axon reddit cleanup
axon reddit clear
axon reddit recover
axon reddit worker
```

These subcommands operate on the shared ingest queue across source types.

## Examples

```bash
# Async enqueue (default)
axon reddit rust

# Sync run with top posts from last month
axon reddit rust --sort top --time month --max-posts 50 --wait true

# Single thread
axon reddit "https://www.reddit.com/r/rust/comments/abc123/example_thread"

# Include link scraping flag
axon reddit MachineLearning --scrape-links true --wait true
```

## Notes

- Uses OAuth2 app credentials (no user login flow).
- Job records are stored in `axon_ingest_jobs` with `source_type='reddit'`.
