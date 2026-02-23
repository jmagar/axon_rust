# Changelog

## [Unreleased] — perf/command-performance-fixes

This section documents the changes introduced on the `perf/command-performance-fixes` branch relative to `main`. It serves as an operator migration guide for those upgrading an existing deployment.

---

### New Commands

Five commands have been added. See `docs/commands/` for per-command documentation.

| Command | Purpose | Requires |
|---------|---------|---------|
| `axon github <owner/repo>` | Ingest GitHub repo (code, issues, PRs, wiki) into Qdrant | `GITHUB_TOKEN` (optional) |
| `axon reddit <subreddit>` | Ingest subreddit posts/comments into Qdrant | `REDDIT_CLIENT_ID`, `REDDIT_CLIENT_SECRET` |
| `axon youtube <url>` | Ingest YouTube video transcript via yt-dlp into Qdrant | `yt-dlp` installed, `OPENAI_BASE_URL`/`OPENAI_MODEL` |
| `axon sessions` | Ingest AI session exports (Claude/Codex/Gemini) into Qdrant | None (reads local history paths) |
| `axon research <query>` | Web research via Tavily AI search with LLM synthesis | `TAVILY_API_KEY`, `OPENAI_BASE_URL`, `OPENAI_MODEL` |

The existing `search` command now also requires `TAVILY_API_KEY` (it previously accepted any configured search provider).

---

### New Environment Variables

Add these to your `.env` file if you use the corresponding commands. All are optional unless noted.

| Variable | Required for | Description |
|----------|-------------|-------------|
| `TAVILY_API_KEY` | `search`, `research` | Tavily AI Search API key. Get one at `https://tavily.com`. |
| `REDDIT_CLIENT_ID` | `reddit` | OAuth2 client ID from `https://www.reddit.com/prefs/apps`. |
| `REDDIT_CLIENT_SECRET` | `reddit` | OAuth2 client secret from the same Reddit app. |
| `GITHUB_TOKEN` | `github` (optional) | Personal access token. Without it, GitHub API rate limits apply (60 req/hr unauthenticated vs. 5000 req/hr authenticated). |
| `AXON_INGEST_QUEUE` | `github`, `reddit`, `youtube` | AMQP queue name for ingest jobs. Defaults to `axon.ingest.jobs`. |
| `AXON_INGEST_LANES` | `github`, `reddit`, `youtube` | Number of parallel worker lanes per ingest worker process. Defaults to `2`. |

No existing env vars have been removed or renamed.

---

### New AMQP Queue

A new queue is used by the ingest commands:

```
axon.ingest.jobs  (default; override via AXON_INGEST_QUEUE)
```

The queue is declared durable and will be created automatically when the first ingest job is enqueued or when an ingest worker starts. No manual RabbitMQ setup is required.

If you are monitoring queue depths, add `axon.ingest.jobs` to your monitoring configuration.

---

### New Database Table

A new Postgres table is created automatically by `ensure_schema()` in `crates/jobs/ingest_jobs.rs` on first use. No manual migration is needed.

```sql
CREATE TABLE IF NOT EXISTS axon_ingest_jobs (
    id          UUID PRIMARY KEY,
    status      TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed', 'canceled')),
    source_type TEXT NOT NULL,   -- 'github' | 'reddit' | 'youtube'
    target      TEXT NOT NULL,   -- owner/repo, subreddit, or YouTube URL
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at  TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    error_text  TEXT,
    result_json JSONB,
    config_json JSONB NOT NULL
);
```

This table differs from the other four job tables: it uses `source_type` + `target` instead of `url` or `urls_json`. See `docs/schema.md` for the full column reference.

---

### External Dependency: yt-dlp

The `youtube` command shells out to `yt-dlp` to download transcripts. You must install `yt-dlp` on the host or inside the worker container before using this command.

```bash
# Install on the host
pip install yt-dlp
# or
brew install yt-dlp

# Verify
yt-dlp --version
```

If `yt-dlp` is not found, `axon youtube` will fail with a clear error message. The other commands are not affected.

---

### Worker Container Changes

The ingest worker is not yet wired into the s6 supervisor bundle (`docker/s6-rc.d/`). Until the `ingest-worker` s6 service is added, run the worker inline:

```bash
axon github worker   # or: axon reddit worker / axon youtube worker
```

Or run it directly in a container:

```bash
docker compose exec axon-workers axon github worker
```

This is a known gap; the s6 service definition will be added in a follow-up.

---

### Performance Profile Changes

No changes to the four named profiles (`high-stable`, `balanced`, `extreme`, `max`). The ingest worker uses a fixed lane count (`AXON_INGEST_LANES`, default `2`) rather than CPU-scaled concurrency because ingest throughput is API-bound, not CPU-bound.

---

### ask RAG Tuning (new env vars)

The `ask` command gains eight tuning env vars for controlling the retrieval pipeline. These are all optional with sensible defaults:

| Variable | Default | Effect |
|----------|---------|--------|
| `AXON_ASK_MIN_RELEVANCE_SCORE` | `0.45` | Minimum Qdrant score for context inclusion |
| `AXON_ASK_CANDIDATE_LIMIT` | `64` | Chunks retrieved before reranking |
| `AXON_ASK_CHUNK_LIMIT` | `10` | Chunks included in LLM prompt after reranking |
| `AXON_ASK_FULL_DOCS` | `4` | Top documents for which full-doc backfill is attempted |
| `AXON_ASK_BACKFILL_CHUNKS` | `3` | Extra chunks per full-doc backfill pass |
| `AXON_ASK_DOC_FETCH_CONCURRENCY` | `4` | Concurrent Qdrant fetches during backfill |
| `AXON_ASK_DOC_CHUNK_LIMIT` | `192` | Max chunks fetched per document during backfill |
| `AXON_ASK_MAX_CONTEXT_CHARS` | `120000` | Total characters passed to the LLM |

These replace previously hard-coded constants. Existing deployments are unaffected — the defaults match the previous behavior.

---

### Upgrade Checklist

For operators upgrading from `main`:

- [ ] Add `TAVILY_API_KEY` to `.env` if you want to use `search` or `research`
- [ ] Add `REDDIT_CLIENT_ID` and `REDDIT_CLIENT_SECRET` to `.env` if you want to use `reddit`
- [ ] Add `GITHUB_TOKEN` to `.env` if you want authenticated GitHub API access
- [ ] Install `yt-dlp` on the host or in the worker image if you want to use `youtube`
- [ ] Run `./scripts/install-git-hooks.sh` if you are developing on this branch
- [ ] Allow `axon.ingest.jobs` through any RabbitMQ firewall or monitoring rules
- [ ] The `axon_ingest_jobs` table is auto-created — no manual SQL needed
- [ ] Optionally tune `AXON_ASK_*` env vars if the `ask` command is returning poor results
