# Reddit Ingest
Last Modified: 2026-03-03

Version: 1.0.0
Last Updated: 01:26:53 | 02/25/2026 EST

> CLI reference (flags, subcommands, examples): [`docs/commands/reddit.md`](../commands/reddit.md)

Ingests subreddit posts and comment threads into Qdrant via the Reddit OAuth2 API (client credentials flow — no user login required).

## What Gets Indexed

- **Posts**: title + selftext body (for text posts); title + URL (for link posts)
- **Comments**: full thread up to `--depth` levels deep, filtered by `--min-score`
- Deleted (`[deleted]`) and removed (`[removed]`) comments are skipped
- Post metadata: score, flair, author, timestamp

## Prerequisites

A Reddit **script app** with client credentials. Both `REDDIT_CLIENT_ID` and `REDDIT_CLIENT_SECRET` are required — the command fails immediately if either is missing.

1. Go to [reddit.com/prefs/apps](https://www.reddit.com/prefs/apps) → **"create another app"**
2. Select **"script"** type, set redirect URI to `http://localhost:8080`
3. Copy the **client ID** (displayed under the app name) and **client secret**

```bash
# .env
REDDIT_CLIENT_ID=your_client_id
REDDIT_CLIENT_SECRET=your_client_secret
```

## How It Works

1. Validates subreddit name: 3–21 chars, alphanumeric + underscore only (prevents path traversal)
2. Authenticates via Reddit OAuth2 client credentials, obtaining a bearer token
3. Fetches posts from `https://oauth.reddit.com/r/<subreddit>/<sort>?limit=100`; paginates until `--max-posts` reached
4. For each post, fetches the comment tree at `https://oauth.reddit.com<permalink>.json?limit=100&depth=<n>`
5. Recursively traverses comments up to `--depth` levels, skipping entries below `--min-score`
6. Posts and comments embedded via `embed_text_with_metadata()` → TEI → Qdrant

**User-Agent:** `axon-ingest/1.0 by /u/axon_bot` (Reddit requires a descriptive UA string per their API terms)

## Rate Limits

Reddit OAuth2 script apps are allowed 100 requests/minute. On 429 responses, the ingest worker currently retries with fixed exponential backoff (2s, 4s, 8s; max 3 retries). `Retry-After` headers are not currently parsed.

## Known Limitations

| Limitation | Detail |
|-----------|--------|
| **Link posts** | Only title + URL indexed; no external page content. Use `axon crawl` for the linked URL |
| **Comment depth limits** | Reddit's API can truncate very deep threads before `--depth` is reached |
| **Private / quarantined subreddits** | Client credentials flow cannot access these; fails with 403 |
| **Score freshness** | Scores captured at index time; not updated on re-index |
| **Subreddit name validation** | 3–21 chars, alphanumeric + underscore only. Don't include `r/` prefix |

## Troubleshooting

**`invalid subreddit name`**

Name contains invalid characters or wrong length. Remove any `r/` prefix.

**`401 Unauthorized`**

Wrong `REDDIT_CLIENT_ID` or `REDDIT_CLIENT_SECRET`. Verify in `.env` and confirm the app type is **"script"** on reddit.com/prefs/apps.

**`403 Forbidden`**

Subreddit is private or quarantined — not accessible with client credentials.

**Rate limit / 429 errors**

Handled automatically with exponential backoff retries. If errors persist, reduce request rate and verify app health on Reddit.
