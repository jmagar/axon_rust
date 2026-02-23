# axon reddit

Ingest a subreddit or Reddit thread into Qdrant. Authenticates via Reddit OAuth2, fetches posts and top-level comments, chunks the content, and embeds it into the configured Qdrant collection.

## Synopsis

```bash
axon reddit <target> [FLAGS]
axon reddit <SUBCOMMAND> [ARGS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<target>` | Subreddit name (e.g. `rust`) or thread URL (e.g. `https://reddit.com/r/rust/comments/abc123/...`) |

## Required Environment Variables

| Variable | Description |
|----------|-------------|
| `REDDIT_CLIENT_ID` | OAuth2 client ID from `https://www.reddit.com/prefs/apps`. |
| `REDDIT_CLIENT_SECRET` | OAuth2 client secret from the same Reddit app. |

Set in `.env`:

```bash
REDDIT_CLIENT_ID=your_client_id
REDDIT_CLIENT_SECRET=your_client_secret
```

### Creating a Reddit App

1. Go to `https://www.reddit.com/prefs/apps`
2. Click "create another app..."
3. Choose "script" type
4. Set redirect URI to `http://localhost:8080` (not used, but required)
5. Copy the client ID (shown under the app name) and secret

## Flags

All global flags apply. Key flags for this command:

| Flag | Default | Description |
|------|---------|-------------|
| `--wait <bool>` | `false` | Block until the ingest job completes. |
| `--collection <name>` | `cortex` | Qdrant collection to embed into. |
| `--json` | `false` | Machine-readable JSON output. |

## Job Subcommands

```bash
axon reddit status <job_id>   # show job status
axon reddit cancel <job_id>   # cancel a pending/running job
axon reddit list              # list recent ingest jobs (last 50)
axon reddit cleanup           # remove failed/canceled jobs older than 30 days
axon reddit clear             # delete all ingest jobs and purge the queue
axon reddit recover           # reclaim stale/interrupted jobs
axon reddit worker            # run an ingest worker inline (blocking)
```

Note: These subcommands operate on all ingest jobs (github, reddit, youtube). The `list` output shows `source_type/target` to distinguish them.

## Examples

```bash
# Async (default) — returns immediately with a job ID
axon reddit rust

# Synchronous — blocks until complete
axon reddit rust --wait true

# Specific thread
axon reddit "https://reddit.com/r/rust/comments/abc123/announcing_tokio_1_0"

# Specific collection
axon reddit MachineLearning --collection ml-discussions --wait true

# List all ingest jobs
axon reddit list
```

## Ingest Scope

The ingest pipeline fetches:

- Post titles and selftext (for text posts) or link content (for link posts)
- Top-level comments and their top replies
- Post scores, comment counts, and metadata (flair, author, timestamp)

All content is chunked and embedded via TEI before upsert into Qdrant.

## Notes

- Reddit API uses OAuth2 client credentials flow (no user login required).
- Rate limit: 60 requests/minute for OAuth2 apps.
- The ingest job records are stored in the `axon_ingest_jobs` Postgres table with `source_type='reddit'` and `target` set to the subreddit name or URL.
- Very active subreddits (e.g. `r/programming`) may have many posts; the ingest pipeline fetches the top/hot/new posts within a configurable window.
