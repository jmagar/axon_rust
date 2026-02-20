# ⚡ **Axon**

Self-hosted web crawling and RAG pipeline powered by Spider.rs. Single binary (`axon`) backed by a local Docker stack.

## Overview

Axon is a single CLI for crawl/scrape/extract plus local vector retrieval and Q&A. It runs on a local Docker stack (Postgres, Redis, RabbitMQ, Qdrant) and external model endpoints (TEI and OpenAI-compatible API).

## Features

- Commands: `scrape`, `crawl`, `map`, `search`, `batch`, `extract`, `embed`, `query`, `retrieve`, `ask`, `evaluate`, `suggest`, `sources`, `domains`, `stats`, `status`, `doctor`, `dedupe`, `debug`
- Async queue-backed jobs for `crawl`/`batch`/`extract`/`embed`
- TEI embeddings + Qdrant vector storage
- OpenAI-compatible extraction and answer generation
- Browser fallback via Selenium WebDriver for dynamic sites
- Automation-friendly JSON mode via `--json`

## Architecture

### Crate Layout (`crates/*`)

- `crates/cli` — command routing and UX
- `crates/core` — config, HTTP, health checks, logging, content transforms
- `crates/crawl` — crawling engine and sitemap backfill
- `crates/extract` — placeholder module (extraction logic lives in `vector/ops_v2`)
- `crates/jobs` — queue workers for crawl/batch/extract/embed (v2 crawl pipeline)
- `crates/vector` — embeddings + Qdrant operations (`query/retrieve/ask/evaluate/suggest/sources/domains/stats`)

```
axon_rust/
├── mod.rs                  # Library root — run() dispatch
├── main.rs                 # Binary entry point (single binary: axon)
├── crates/
│   ├── mod.rs
│   ├── cli/
│   │   ├── mod.rs
│   │   └── commands/       # One file (or subdir) per command
│   │       ├── common.rs   # URL parsing utilities (parse_urls, expand_url_glob_seed, etc.)
│   │       ├── probe.rs    # HTTP probe helpers used by doctor
│   │       ├── crawl.rs    # Crawl command entry point
│   │       ├── crawl/      # Crawl subcommand modules
│   │       │   ├── audit.rs
│   │       │   └── audit/audit_diff.rs
│   │       ├── doctor/     # Doctor command subdir
│   │       └── scrape.rs, map.rs, batch.rs, embed.rs, extract.rs,
│   │           search.rs, status.rs, debug.rs
│   ├── core/
│   │   ├── config/         # CLI parsing (clap), Config struct, performance profiles
│   │   │   ├── cli.rs      # clap arg definitions (GlobalArgs, subcommand args)
│   │   │   ├── types.rs    # Config struct and enum types
│   │   │   ├── parse.rs    # Post-parse normalization and profile application
│   │   │   └── help.rs     # Long-form help strings
│   │   ├── content/        # HTML→markdown, URL→filename, transform pipeline
│   │   │   ├── deterministic.rs  # DeterministicExtractionEngine, parsers
│   │   │   └── tests.rs
│   │   ├── health.rs       # redis_healthy() connectivity check
│   │   ├── http.rs         # build_client(), fetch_html(), validate_url() (SSRF guard)
│   │   ├── logging.rs      # log_info(), log_warn(), log_done() structured output
│   │   └── ui.rs           # ANSI color helpers (primary, accent, muted, status_text)
│   ├── crawl/
│   │   ├── mod.rs
│   │   ├── engine.rs       # crawl_and_collect_map(), run_crawl_once(),
│   │   │                   # try_auto_switch(), should_fallback_to_chrome()
│   │   └── engine/
│   │       ├── sitemap.rs  # crawl_sitemap_urls(), append_sitemap_backfill()
│   │       └── tests.rs
│   ├── extract/
│   │   └── mod.rs          # (placeholder; LLM extraction is in vector/ops_v2)
│   ├── jobs/               # AMQP-backed async job workers
│   │   ├── mod.rs
│   │   ├── common.rs       # Shared infrastructure: make_pool, open_amqp_channel,
│   │   │   + common/       # claim_next_pending, mark_job_failed, enqueue_job
│   │   │       └── tests.rs
│   │   ├── batch_jobs/     # Batch worker
│   │   │   ├── worker.rs, maintenance.rs, tests.rs
│   │   ├── embed_jobs/     # Embed worker
│   │   │   └── tests.rs
│   │   ├── extract_jobs/   # Extract worker
│   │   │   ├── worker.rs, tests.rs
│   │   └── crawl_jobs_v2/  # V2 crawl pipeline (modular)
│   │       ├── mod.rs, manifest.rs, processor.rs, repo.rs,
│   │       │   sitemap.rs, watchdog.rs, worker.rs
│   │       └── runtime/
│   │           ├── mod.rs, robots.rs, tests.rs, worker.rs
│   │           └── worker/
│   │               ├── worker_loops.rs
│   │               └── worker_process/
│   └── vector/
│       ├── mod.rs
│       ├── ops_dispatch.rs  # Dispatcher: routes to v2 ops; chunk_text(), embed_path_native()
│       └── ops_v2/          # V2 vector ops (modular)
│           ├── input.rs, ranking.rs, tei.rs
│           ├── commands/    # Per-command handlers
│           │   ├── ask/, evaluate.rs, query.rs, streaming.rs, suggest.rs
│           ├── qdrant/      # Qdrant client and operations
│           │   ├── client.rs, commands.rs, types.rs, utils.rs
│           └── stats/
├── docker/
│   ├── Dockerfile          # Multi-stage build; s6-overlay for service supervision
│   ├── rabbitmq/
│   │   └── 20-axon.conf    # RabbitMQ tuning config
│   ├── scripts/
│   │   └── healthcheck-workers.sh
│   └── s6/
│       ├── cont-init.d/
│       │   └── 10-load-axon-env  # Loads .env on container startup
│       └── s6-rc.d/        # s6-rc service definitions
│           ├── crawl-worker/  (run, type)
│           ├── batch-worker/  (run, type)
│           ├── extract-worker/  (run, type)
│           ├── embed-worker/  (run, type)
│           └── user/contents.d/
├── docker-compose.yaml
├── .env                    # Secrets (gitignored)
└── .env.example            # Template — copy to .env and fill in
```

### Docker Services (`docker-compose.yaml`)

- `axon-postgres` -> `localhost:53432`
- `axon-redis` -> `localhost:53379`
- `axon-rabbitmq` -> `localhost:45535`
- `axon-qdrant` -> `localhost:53333` (HTTP), `53334` (gRPC)
- `axon-webdriver` -> `localhost:4444` (WebDriver), `localhost:7900` (VNC)
- `axon-workers` (s6-supervised worker container; depends on all infra being healthy)

Services run on the `axon` bridge network with persistent volumes under `/home/jmagar/appdata/axon-*`.

## Quick Start

```bash
# 1) from repo root
cp .env.example .env
# edit .env — set POSTGRES_PASSWORD, REDIS_PASSWORD, RABBITMQ_PASS, TEI_URL, OPENAI_*

# 2) start stack
docker compose up -d
docker compose ps
```

```bash
# 3) run CLI (wrapper loads .env automatically)
./scripts/axon doctor
./scripts/axon scrape https://example.com --wait true
./scripts/axon crawl https://docs.rs/spider --wait false
./scripts/axon status
```

```bash
# Optional local alias
alias axon='./scripts/axon'

axon doctor
axon query "spider crawler"
axon ask "what does spider.rs support?"
```

## Environment

Copy `.env.example` to `.env`. At minimum set the `[REQUIRED]` vars:

### Required

| Variable | Description |
|----------|-------------|
| `POSTGRES_USER` / `POSTGRES_PASSWORD` / `POSTGRES_DB` | Docker Compose Postgres credentials |
| `REDIS_PASSWORD` | Docker Compose Redis password |
| `RABBITMQ_USER` / `RABBITMQ_PASS` | Docker Compose RabbitMQ credentials |
| `AXON_PG_URL` | PostgreSQL DSN for CLI/workers |
| `AXON_REDIS_URL` | Redis DSN for health checks and cancel flags |
| `AXON_AMQP_URL` | AMQP DSN for queue-backed jobs |
| `QDRANT_URL` | Qdrant base URL |
| `TEI_URL` | TEI embeddings base URL (external — not in compose) |
| `OPENAI_BASE_URL` | OpenAI-compatible base URL for extract/ask/suggest/debug (e.g. `http://host/v1`) |
| `OPENAI_API_KEY` | API key for OPENAI_BASE_URL |
| `OPENAI_MODEL` | Model name for completions |

### Optional Queue and Collection

| Variable | Default | Description |
|----------|---------|-------------|
| `AXON_CRAWL_QUEUE` | `axon.crawl.jobs` | Crawl job queue name |
| `AXON_BATCH_QUEUE` | `axon.batch.jobs` | Batch job queue name |
| `AXON_EXTRACT_QUEUE` | `axon.extract.jobs` | Extract job queue name |
| `AXON_EMBED_QUEUE` | `axon.embed.jobs` | Embed job queue name |
| `AXON_COLLECTION` | `cortex` | Qdrant collection name |
| `AXON_QUEUE_INJECTION_RULES_JSON` | — | JSON rules for queue routing overrides |

### Optional Browser / WebDriver

| Variable | Default | Description |
|----------|---------|-------------|
| `AXON_WEBDRIVER_URL` | — | Primary WebDriver endpoint (e.g. `http://127.0.0.1:4444`) |
| `WEBDRIVER_URL` | — | Legacy fallback for `AXON_WEBDRIVER_URL` |
| `AXON_CHROME_REMOTE_URL` | — | Remote Chrome DevTools endpoint |
| `CHROME_URL` | — | spider-rs native CDP env var (alternative to `AXON_CHROME_REMOTE_URL`) |
| `AXON_CHROME_PROXY` | — | Proxy URL for Chrome requests |
| `AXON_CHROME_USER_AGENT` | — | User-Agent override for Chrome |
| `AXON_CHROME_DIAGNOSTICS` | `false` | Enable browser diagnostics artifact collection |
| `AXON_CHROME_DIAGNOSTICS_SCREENSHOT` | — | Save diagnostic screenshots to disk when set |
| `AXON_CHROME_DIAGNOSTICS_EVENTS` | — | Log raw CDP events when set |
| `AXON_CHROME_DIAGNOSTICS_DIR` | — | Directory for diagnostics output (default: temp dir) |

### Optional Worker / Watchdog

| Variable | Default | Description |
|----------|---------|-------------|
| `AXON_JOB_STALE_TIMEOUT_SECS` | `300` | Seconds before a running job is considered stale |
| `AXON_JOB_STALE_CONFIRM_SECS` | `60` | Seconds to confirm stale status before reclaiming |
| `AXON_NO_WIPE` | — | Prevent destructive cache wipes when set |

### Optional Output / Misc

| Variable | Default | Description |
|----------|---------|-------------|
| `AXON_NO_COLOR` | — | Disable ANSI color output when set |
| `AXON_DOMAINS_DETAILED` | — | Enable detailed `domains` command output |
| `AXON_EXTRACT_EST_COST_PER_1K_TOKENS` | — | Override extract cost estimate (USD/1K tokens) |
| `AXON_LOG_FILE` | `logs/axon.log` | Structured JSON log file path (always on) |
| `AXON_LOG_MAX_BYTES` | `10485760` | Max bytes per log file before rotation (10MB) |
| `AXON_LOG_MAX_FILES` | `3` | Total log files to keep (`axon.log`, `.1`, `.2`) |

### Legacy Aliases

`NUQ_DATABASE_URL`, `NUQ_RABBITMQ_URL`, `REDIS_URL` are accepted as fallbacks for `AXON_PG_URL`, `AXON_AMQP_URL`, `AXON_REDIS_URL` respectively. `WEBDRIVER_URL` is accepted as a fallback for `AXON_WEBDRIVER_URL`.

Notes:
- Container runtime uses service DNS names (`axon-postgres`, `axon-redis`, etc.).
- Local runtime rewrites those to mapped localhost ports automatically.
- `./scripts/axon` sources `.env`; running `cargo run --bin axon -- ...` directly does not.

## Worker Model (s6 Supervised)

`axon-workers` uses `s6-overlay` and runs four long-lived worker services in one container:

- `crawl-worker` -> `/usr/local/bin/axon crawl worker`
- `batch-worker` -> `/usr/local/bin/axon batch worker`
- `extract-worker` -> `/usr/local/bin/axon extract worker`
- `embed-worker` -> `/usr/local/bin/axon embed worker`

Startup loads `.env` via `docker/s6/cont-init.d/10-load-axon-env`. Health checks verify each worker process via s6-svstat. The container is resource-limited to 4 CPUs / 4 GB RAM with a 512 MB / 1 CPU reservation.

Worker behavior notes:
- Workers run startup stale-job reclaim sweeps plus periodic stale sweeps.
- Stale timeout and confirmation window are tunable via `AXON_JOB_STALE_TIMEOUT_SECS` / `AXON_JOB_STALE_CONFIRM_SECS`.

## Commands

| Command | Purpose | Async? |
|---------|---------|--------|
| `scrape <url>` | Single-page scrape to markdown | No |
| `crawl <url>` | Full site crawl, saves markdown files | Yes (default) |
| `map <url>` | Discover all URLs without scraping | No |
| `batch <urls...>` | Bulk scrape multiple URLs | Yes (default) |
| `extract <urls...>` | LLM-powered structured data extraction | Yes (default) |
| `search <query>` | Web search (requires search provider) | No |
| `embed [input]` | Embed file/dir/URL into Qdrant | Yes (default) |
| `query <text>` | Semantic vector search | No |
| `retrieve <url>` | Fetch stored document chunks from Qdrant | No |
| `ask <question>` | RAG: search + LLM answer | No |
| `evaluate <question>` | RAG vs baseline + LLM judge (accuracy · relevance · completeness · verdict) | No |
| `suggest [focus]` | Suggest complementary docs URLs not already indexed | No |
| `sources` | List all indexed URLs + chunk counts | No |
| `domains` | List indexed domains + stats | No |
| `stats` | Qdrant collection stats | No |
| `status` | Show async job queue status | No |
| `doctor` | Diagnose service connectivity | No |
| `debug` | Run doctor + LLM-assisted troubleshooting | No |
| `dedupe` | Remove duplicate vectors from Qdrant collection | No |

### Job Subcommands (for crawl / batch / extract / embed)

```bash
axon crawl status <job_id>
axon crawl cancel <job_id>
axon crawl errors <job_id>
axon crawl list
axon crawl cleanup
axon crawl clear
axon crawl recover    # reclaim stale/interrupted jobs
axon crawl worker     # run a worker inline
```

### Global Flags Reference

All flags are global (usable with any subcommand).

#### Core Behavior

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--wait <bool>` | bool | `false` | Run synchronously and block until completion. Without this, async commands enqueue and return immediately. |
| `--yes` | flag | `false` | Skip confirmation prompts (non-interactive mode). |
| `--json` | flag | `false` | Machine-readable JSON output on stdout. |

#### Crawl & Scrape

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--max-pages <n>` | u32 | `0` | Page cap for crawl (0 = uncapped). |
| `--max-depth <n>` | usize | `5` | Maximum crawl depth from start URL. |
| `--render-mode <mode>` | enum | `auto-switch` | `http`, `chrome`, or `auto-switch`. Auto-switch tries HTTP first, falls back to Chrome if >60% thin pages. |
| `--format <fmt>` | enum | `markdown` | Output format: `markdown`, `html`, `rawHtml`, `json`. |
| `--include-subdomains <bool>` | bool | `true` | Include subdomains during crawl. **Note:** defaults `true` — may crawl more than expected. |
| `--respect-robots <bool>` | bool | `false` | Respect `robots.txt` directives. **Note:** defaults `false` — consider legal/ethical implications. |
| `--discover-sitemaps <bool>` | bool | `true` | Discover and backfill URLs from sitemap.xml after crawl. |
| `--max-sitemaps <n>` | usize | `512` | Maximum sitemap URLs to backfill per crawl. |
| `--min-markdown-chars <n>` | usize | `200` | Minimum markdown character count; pages below this are flagged as "thin". |
| `--drop-thin-markdown <bool>` | bool | `true` | Skip thin pages — do not save or embed them. |
| `--delay-ms <ms>` | u64 | `0` | Delay between requests in milliseconds. |
| `--exclude-path-prefix <prefixes>` | csv | *(locale list)* | Comma-separated URL path prefixes to exclude (e.g. `/fr,/de`). Defaults to a broad locale-prefix list. |
| `--url-glob <patterns>` | csv | — | Comma-separated brace-glob patterns for URL expansion. |
| `--start-url <url>` | string | `https://example.com` | Seed URL override. |

#### Browser / WebDriver

| Flag | Type | Default | Env Var | Description |
|------|------|---------|---------|-------------|
| `--webdriver-url <url>` | string | — | `AXON_WEBDRIVER_URL` | WebDriver endpoint for browser fallback (e.g. `http://127.0.0.1:4444`). |
| `--chrome-remote-url <url>` | string | — | `AXON_CHROME_REMOTE_URL` | Remote Chrome DevTools endpoint. |
| `--chrome-proxy <url>` | string | — | `AXON_CHROME_PROXY` | Proxy URL for Chrome requests. |
| `--chrome-user-agent <ua>` | string | — | `AXON_CHROME_USER_AGENT` | User-Agent override for Chrome. |
| `--chrome-headless <bool>` | bool | `true` | — | Run Chrome in headless mode. |
| `--chrome-anti-bot <bool>` | bool | `true` | — | Enable anti-bot evasion in Chrome. |
| `--chrome-intercept <bool>` | bool | `true` | — | Enable request interception in Chrome. |
| `--chrome-stealth <bool>` | bool | `true` | — | Enable stealth mode in Chrome. |
| `--chrome-bootstrap <bool>` | bool | `true` | — | Enable Chrome bootstrap. |
| `--chrome-bootstrap-timeout-ms <ms>` | u64 | `3000` | — | Bootstrap timeout in ms. |
| `--chrome-bootstrap-retries <n>` | usize | `2` | — | Bootstrap retry count. |

#### Caching

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--cache <bool>` | bool | `true` | Enable response caching. |
| `--cache-skip-browser <bool>` | bool | `false` | Skip cache for browser-rendered pages. |

#### Output

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--output-dir <dir>` | path | `.cache/axon-rust/output` | Directory for saved markdown/HTML output files. |
| `--output <path>` | path | — | Explicit output file path (overrides `--output-dir` for single-file commands). |

#### Vector & Embedding

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--collection <name>` | string | `cortex` | Qdrant collection name. Also settable via `AXON_COLLECTION` env var. |
| `--embed <bool>` | bool | `true` | Auto-embed scraped content into Qdrant. |
| `--limit <n>` | usize | `10` | Result limit for search/query commands. |
| `--query <text>` | string | — | Query text (alternative to positional argument for some commands). |
| `--urls <csv>` | string | — | Comma-separated URL list (alternative to positional arguments). |

#### Performance Tuning

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--performance-profile <p>` | enum | `high-stable` | `high-stable`, `extreme`, `balanced`, `max`. Sets defaults for concurrency, timeouts, retries. |
| `--batch-concurrency <n>` | usize | `16` | Concurrent connections for batch operations (clamped 1–512). |
| `--concurrency-limit <n>` | usize | — | Override all three concurrency limits (crawl, sitemap, backfill) at once. |
| `--crawl-concurrency-limit <n>` | usize | *profile* | Override crawl concurrency. |
| `--sitemap-concurrency-limit <n>` | usize | *profile* | Override sitemap backfill concurrency. |
| `--backfill-concurrency-limit <n>` | usize | *profile* | Override backfill concurrency. |
| `--request-timeout-ms <ms>` | u64 | *profile* | Per-request timeout in milliseconds. |
| `--fetch-retries <n>` | usize | *profile* | Number of retries on failed fetches. |
| `--retry-backoff-ms <ms>` | u64 | *profile* | Backoff between retries in milliseconds. |

#### Scheduled / Cron

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--cron-every-seconds <n>` | u64 | — | Repeat a command every N seconds. |
| `--cron-max-runs <n>` | usize | — | Maximum number of cron repetitions (unset = unlimited). |

#### Watchdog

| Flag | Type | Default | Env Var | Description |
|------|------|---------|---------|-------------|
| `--watchdog-stale-timeout-secs <n>` | i64 | `300` | `AXON_JOB_STALE_TIMEOUT_SECS` | Seconds before a running job is considered stale. |
| `--watchdog-confirm-secs <n>` | i64 | `60` | `AXON_JOB_STALE_CONFIRM_SECS` | Seconds to confirm stale status before reclaiming. |

#### Service URLs (override env vars)

| Flag | Env Var | Fallback |
|------|---------|----------|
| `--pg-url <url>` | `AXON_PG_URL` / `NUQ_DATABASE_URL` | `postgresql://axon:postgres@127.0.0.1:53432/axon` |
| `--redis-url <url>` | `AXON_REDIS_URL` / `REDIS_URL` | `redis://127.0.0.1:53379` |
| `--amqp-url <url>` | `AXON_AMQP_URL` / `NUQ_RABBITMQ_URL` | `amqp://axon:axonrabbit@127.0.0.1:45535/%2f` |
| `--qdrant-url <url>` | `QDRANT_URL` | `http://127.0.0.1:53333` |
| `--tei-url <url>` | `TEI_URL` | *(empty)* |
| `--openai-base-url <url>` | `OPENAI_BASE_URL` | *(empty)* |
| `--openai-api-key <key>` | `OPENAI_API_KEY` | *(empty)* |
| `--openai-model <name>` | `OPENAI_MODEL` | *(empty)* |

#### Queue Configuration

| Flag | Env Var | Default |
|------|---------|---------|
| `--shared-queue <bool>` | — | `true` |
| `--crawl-queue <name>` | `AXON_CRAWL_QUEUE` | `axon.crawl.jobs` |
| `--batch-queue <name>` | `AXON_BATCH_QUEUE` | `axon.batch.jobs` |
| `--extract-queue <name>` | `AXON_EXTRACT_QUEUE` | `axon.extract.jobs` |
| `--embed-queue <name>` | `AXON_EMBED_QUEUE` | `axon.embed.jobs` |

## Performance Profiles

Concurrency tuned relative to available CPU cores:

| Profile | Crawl concurrency | Sitemap concurrency | Backfill concurrency | Timeout | Retries | Backoff |
|---------|------------------|---------------------|----------------------|---------|---------|---------|
| `high-stable` (default) | CPUs×8 (64–192) | CPUs×12 (64–256) | CPUs×6 (32–128) | 20s | 2 | 250ms |
| `balanced` | CPUs×4 (32–96) | CPUs×6 (32–128) | CPUs×3 (16–64) | 30s | 2 | 300ms |
| `extreme` | CPUs×16 (128–384) | CPUs×20 (128–512) | CPUs×10 (64–256) | 15s | 1 | 100ms |
| `max` | CPUs×24 (256–1024) | CPUs×32 (256–1536) | CPUs×20 (128–1024) | 12s | 1 | 50ms |

## Troubleshooting

- `axon doctor` for service reachability (Postgres/Redis/AMQP/Qdrant/TEI/OpenAI)
- `axon debug` to run doctor + LLM-assisted troubleshooting with your configured OpenAI-compatible endpoint
- `docker compose logs -f axon-workers` to inspect worker failures
- Jobs stuck in pending: ensure `axon-workers` is healthy and AMQP/Redis are reachable
- Manually reclaim stale jobs if needed:
  - `axon crawl recover`
  - `axon batch recover`
  - `axon extract recover`
  - `axon embed recover`
- `ask`/`extract` failures: verify `OPENAI_BASE_URL` is a base URL (e.g. `http://host/v1`, not `/chat/completions`)
- `embed`/`query` failures: verify `TEI_URL` and `QDRANT_URL`
- Browser fallback failures: verify `AXON_WEBDRIVER_URL` points to a live WebDriver endpoint (e.g. `http://127.0.0.1:4444`). The `axon-webdriver` compose service exposes this at `127.0.0.1:4444` when running.

## Monolith Guardrails

Axon enforces a ratcheting monolith policy on changed code:

- File size limit (changed Rust files): `500` lines
- Rust function size limit (changed functions): `80` lines
- Only Rust source files (`*.rs`) are checked for file size
- Test/config paths are exempt (`tests/**`, `**/*_test.*`, `**/*.test.*`, `**/*.spec.*`, `benches/**`, `config/**`, `**/config/**`, `**/config.rs`)
- Temporary file-level exceptions can be added to `.monolith-allowlist`

Axon also enforces a legacy symbol deny-list in hooks/CI to prevent reintroducing removed v1 paths.

Install local pre-commit enforcement (lefthook):

```bash
# install lefthook once (choose one)
brew install lefthook
# or
cargo install --locked lefthook

# install git hooks for this repo
./scripts/install-git-hooks.sh
```

The same policy runs in CI on pull requests and pushes.

Detailed policy and exception workflow: `docs/monolith-policy.md`.

## Database Schema

Tables are auto-created on first worker/command start via `CREATE TABLE IF NOT EXISTS` in each `*_jobs.rs` file's `ensure_schema()` function.

### axon_crawl_jobs

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| `id` | UUID | NOT NULL | — | Primary key, job identifier |
| `url` | TEXT | NOT NULL | — | Target URL for the crawl |
| `status` | TEXT | NOT NULL | — | `pending` / `running` / `completed` / `failed` / `canceled` |
| `created_at` | TIMESTAMPTZ | NOT NULL | `NOW()` | Job creation timestamp |
| `updated_at` | TIMESTAMPTZ | NOT NULL | `NOW()` | Last status change |
| `started_at` | TIMESTAMPTZ | NULL | — | When worker began processing |
| `finished_at` | TIMESTAMPTZ | NULL | — | When job completed/failed/canceled |
| `error_text` | TEXT | NULL | — | Error message on failure |
| `result_json` | JSONB | NULL | — | Crawl results (pages found, stats) |
| `config_json` | JSONB | NOT NULL | — | Serialized job configuration |

**Index:** `idx_axon_crawl_jobs_status` on `status`.

### axon_batch_jobs

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| `id` | UUID | NOT NULL | — | Primary key |
| `status` | TEXT | NOT NULL | — | `pending` / `running` / `completed` / `failed` / `canceled` |
| `created_at` | TIMESTAMPTZ | NOT NULL | `NOW()` | Job creation timestamp |
| `updated_at` | TIMESTAMPTZ | NOT NULL | `NOW()` | Last status change |
| `started_at` | TIMESTAMPTZ | NULL | — | When worker began processing |
| `finished_at` | TIMESTAMPTZ | NULL | — | When job completed/failed/canceled |
| `error_text` | TEXT | NULL | — | Error message on failure |
| `urls_json` | JSONB | NOT NULL | — | Array of URLs to batch-scrape |
| `result_json` | JSONB | NULL | — | Batch results |
| `config_json` | JSONB | NOT NULL | — | Serialized job configuration |

### axon_extract_jobs

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| `id` | UUID | NOT NULL | — | Primary key |
| `status` | TEXT | NOT NULL | — | `pending` / `running` / `completed` / `failed` / `canceled` |
| `created_at` | TIMESTAMPTZ | NOT NULL | `NOW()` | Job creation timestamp |
| `updated_at` | TIMESTAMPTZ | NOT NULL | `NOW()` | Last status change |
| `started_at` | TIMESTAMPTZ | NULL | — | When worker began processing |
| `finished_at` | TIMESTAMPTZ | NULL | — | When job completed/failed/canceled |
| `error_text` | TEXT | NULL | — | Error message on failure |
| `urls_json` | JSONB | NOT NULL | — | Array of URLs for LLM extraction |
| `result_json` | JSONB | NULL | — | Extracted structured data |
| `config_json` | JSONB | NOT NULL | — | Serialized job configuration |

### axon_embed_jobs

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| `id` | UUID | NOT NULL | — | Primary key |
| `status` | TEXT | NOT NULL | — | `pending` / `running` / `completed` / `failed` / `canceled` |
| `created_at` | TIMESTAMPTZ | NOT NULL | `NOW()` | Job creation timestamp |
| `updated_at` | TIMESTAMPTZ | NOT NULL | `NOW()` | Last status change |
| `started_at` | TIMESTAMPTZ | NULL | — | When worker began processing |
| `finished_at` | TIMESTAMPTZ | NULL | — | When job completed/failed/canceled |
| `error_text` | TEXT | NULL | — | Error message on failure |
| `input_text` | TEXT | NOT NULL | — | Input path, URL, or text to embed |
| `result_json` | JSONB | NULL | — | Embedding results (chunk count, point IDs) |
| `config_json` | JSONB | NOT NULL | — | Serialized job configuration |

## Gotchas

### `--wait false` (default) = fire-and-forget
By default, `crawl`, `batch`, `extract`, and `embed` enqueue jobs and return immediately. Use `--wait true` to block until completion. Without workers running, enqueued jobs will pend forever.

### `render-mode auto-switch`
The default mode. Runs an HTTP crawl first; if >60% of pages are thin (<200 chars) or total coverage is too low, automatically retries with Chrome. Chrome requires `axon-webdriver` running — if unreachable, the HTTP result is kept.

### `crawl_raw()` vs `crawl()`
When Chrome feature is compiled in, `crawl()` expects a Chrome instance. `crawl_raw()` is pure HTTP and always works. `engine.rs` calls `crawl_raw()` for `RenderMode::Http` and `crawl()` for Chrome/AutoSwitch.

### `ask` LLM call pattern
`ask` constructs the URL as: `{OPENAI_BASE_URL}/chat/completions`
- **Correct:** `OPENAI_BASE_URL=http://host/v1`
- **Wrong:** `OPENAI_BASE_URL=http://host/v1/chat/completions` — double path

### TEI batch size / 413 handling
`tei_embed()` in `vector/ops_v2/tei.rs` auto-splits batches on HTTP 413 (Payload Too Large). Set `TEI_MAX_CLIENT_BATCH_SIZE` env var to control default chunk size (default: 64, effective max: 128).

### Text chunking
`chunk_text()` splits at 2000 chars with 200-char overlap. Each chunk = one Qdrant point. Very long pages produce many points.

### Thin page filtering
Pages with fewer than `--min-markdown-chars` (default: 200) are flagged as thin. If `--drop-thin-markdown true` (default), thin pages are skipped — not saved to disk or embedded.

### Collection must exist before upsert
`ensure_collection()` issues a PUT to Qdrant to create or update the collection with the correct vector dimension. This is idempotent — safe to call on every embed.

### Default collection name
The default Qdrant collection is `cortex` (set via `AXON_COLLECTION` or `--collection`). If you previously used an older build that defaulted to `spider_rust`, pass `--collection spider_rust` explicitly.

### Sitemap backfill
After a crawl, `append_sitemap_backfill()` discovers URLs via sitemap.xml that the crawler missed and fetches them individually. Respects `--max-sitemaps` (default: 512) and `--include-subdomains`.

### Docker build context
The `Dockerfile` is at `docker/Dockerfile`. Run `docker compose build` from this directory (not a parent workspace). The binary built inside the container is `axon`.

## Development

### Build

```bash
cargo build --bin axon                        # debug
cargo build --release --bin axon              # release
cargo check                                   # fast type check
```

### Lint

```bash
cargo clippy
cargo fmt --check
```

### Run directly

```bash
# Debug binary
./target/debug/axon scrape https://example.com

# With env overrides
QDRANT_URL=http://localhost:53333 \
TEI_URL=http://myserver:52000 \
./target/release/axon query "embedding pipeline" --collection my_col
```

### Diagnose service connectivity

```bash
axon doctor
```

Checks: Postgres, Redis, RabbitMQ, Qdrant, TEI, LLM endpoint reachability.

## Code Style

- Rust standard style — run `cargo fmt` before committing
- `cargo clippy` clean before committing
- Errors bubble via `Box<dyn Error>` at command boundaries; internal helpers return typed errors
- Structured log output via `log_info` / `log_warn` (not `println!` in library code)
- `--json` flag enables machine-readable output on all commands that print results
