# axon_cli — Axon CLI (Rust + Spider.rs)

Web crawl, scrape, batch, extract, embed, and query — all in one binary backed by a self-hosted RAG stack.

## Quick Start

```bash
# Start infrastructure (Postgres, Redis, RabbitMQ, Qdrant)
docker compose up -d

# Recommended: use the wrapper script (auto-sources .env)
./scripts/axon doctor
./scripts/axon scrape https://example.com --wait true

# Or build and run the binary directly
cargo build --release --bin axon
./target/release/axon --help

# Or build + run in one shot (does NOT auto-source .env)
cargo run --bin axon -- scrape https://example.com --wait true
```

> **Note:** The binary is named `axon`. Build with `cargo build --bin axon`.

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
| `evaluate <question>` | RAG vs baseline + independent LLM judge (accuracy, relevance, completeness, specificity, verdict) | No |
| `suggest [focus]` | Suggest new docs URLs to crawl | No |
| `sources` | List all indexed URLs + chunk counts | No |
| `domains` | List indexed domains + stats | No |
| `stats` | Qdrant collection stats | No |
| `status` | Show async job queue status | No |
| `doctor` | Diagnose service connectivity | No |
| `debug` | Run doctor + LLM-assisted troubleshooting | No |

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

All flags are `--global` (usable with any subcommand).

#### Core Behavior

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--wait <bool>` | bool | `false` | Run synchronously and block until completion. Without this, async commands enqueue and return immediately. |
| `--yes` | flag | `false` | Skip confirmation prompts (non-interactive mode). |
| `--json` | flag | `false` | Machine-readable JSON output on stdout. |

#### Crawl & Scrape

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--max-pages <n>` | u32 | `0` | Page cap for crawl (0 = uncapped, default). |
| `--max-depth <n>` | usize | `5` | Maximum crawl depth from start URL. |
| `--render-mode <mode>` | enum | `auto-switch` | `http`, `chrome`, or `auto-switch`. Auto-switch tries HTTP first, falls back to Chrome if >60% thin pages. |
| `--format <fmt>` | enum | `markdown` | Output format: `markdown`, `html`, `rawHtml`, `json`. |
| `--include-subdomains <bool>` | bool | `true` | Include subdomains during crawl. **Note:** defaults `true` — may crawl more than expected. |
| `--respect-robots <bool>` | bool | `false` | Respect `robots.txt` directives. **Note:** defaults `false` — legal/ethical implications. |
| `--discover-sitemaps <bool>` | bool | `true` | Discover and backfill URLs from sitemap.xml after crawl. |
| `--max-sitemaps <n>` | usize | `512` | Maximum sitemap URLs to backfill per crawl. |
| `--min-markdown-chars <n>` | usize | `200` | Minimum markdown character count; pages below this are flagged as "thin". |
| `--drop-thin-markdown <bool>` | bool | `true` | Skip thin pages — do not save or embed them. |
| `--delay-ms <ms>` | u64 | `0` | Delay between requests in milliseconds. Useful for polite crawling. |

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
| `--crawl-concurrency-limit <n>` | usize | *profile* | Override crawl concurrency (profile default: CPUs x multiplier). |
| `--sitemap-concurrency-limit <n>` | usize | *profile* | Override sitemap backfill concurrency. |
| `--backfill-concurrency-limit <n>` | usize | *profile* | Override backfill concurrency. |
| `--request-timeout-ms <ms>` | u64 | *profile* | Per-request timeout in milliseconds. |
| `--fetch-retries <n>` | usize | *profile* | Number of retries on failed fetches. |
| `--retry-backoff-ms <ms>` | u64 | *profile* | Backoff between retries in milliseconds. |

#### Service URLs (override env vars)

| Flag | Type | Env Var | Fallback |
|------|------|---------|----------|
| `--pg-url <url>` | string | `AXON_PG_URL` / `NUQ_DATABASE_URL` | `postgresql://axon:postgres@127.0.0.1:53432/axon` |
| `--redis-url <url>` | string | `AXON_REDIS_URL` / `REDIS_URL` | `redis://127.0.0.1:53379` |
| `--amqp-url <url>` | string | `AXON_AMQP_URL` / `NUQ_RABBITMQ_URL` | `amqp://axon:axonrabbit@127.0.0.1:45535/%2f` |
| `--qdrant-url <url>` | string | `QDRANT_URL` | `http://127.0.0.1:53333` |
| `--tei-url <url>` | string | `TEI_URL` | *(empty)* |
| `--openai-base-url <url>` | string | `OPENAI_BASE_URL` | *(empty)* |
| `--openai-api-key <key>` | string | `OPENAI_API_KEY` | *(empty)* |
| `--openai-model <name>` | string | `OPENAI_MODEL` | *(empty)* |

#### Queue Configuration

| Flag | Type | Env Var | Default |
|------|------|---------|---------|
| `--shared-queue <bool>` | bool | — | `true` |
| `--crawl-queue <name>` | string | `AXON_CRAWL_QUEUE` | `axon.crawl.jobs` |
| `--batch-queue <name>` | string | `AXON_BATCH_QUEUE` | `axon.batch.jobs` |
| `--extract-queue <name>` | string | `AXON_EXTRACT_QUEUE` | `axon.extract.jobs` |
| `--embed-queue <name>` | string | `AXON_EMBED_QUEUE` | `axon.embed.jobs` |

## Architecture

```
axon_rust/
├── mod.rs                  # Library root — run() dispatch (parse_args() is in crates/core/config.rs)
├── crates/
│   ├── mod.rs              # pub mod cli, core, crawl, extract, jobs, vector
│   ├── cli/
│   │   ├── mod.rs
│   │   └── commands/       # One file per command (scrape, crawl, map, batch, …)
│   │       ├── common.rs   # URL parsing utilities: parse_urls, expand_url_glob_seed
│   │       └── probe.rs    # HTTP probe helpers used by doctor
│   ├── core/
│   │   ├── config.rs       # CLI parsing (clap), Config struct, performance profiles
│   │   ├── content.rs      # HTML→markdown, URL→filename, transform pipeline
│   │   ├── health.rs       # redis_healthy() connectivity check
│   │   ├── http.rs         # build_client(), fetch_html(), validate_url() (SSRF guard — blocks private IPs/ports)
│   │   ├── logging.rs      # log_info(), log_warn(), log_done() structured output
│   │   └── ui.rs           # ANSI color helpers (primary, accent, muted, status_text)
    ├── crawl/
│   │   ├── mod.rs
│   │   └── engine.rs       # crawl_and_collect_map(), run_crawl_once(),
│   │                       # crawl_sitemap_urls(), append_sitemap_backfill(),
│   │                       # try_auto_switch(), should_fallback_to_chrome()
│   ├── jobs/               # AMQP-backed async job workers
│   │   ├── common.rs       # Shared infra: make_pool, open_amqp_channel, claim_next_pending
│   │   ├── crawl_jobs/     # Crawl pipeline (manifest, processor, repo, sitemap, watchdog, worker, runtime)
│   │   ├── batch_jobs.rs, extract_jobs.rs, embed_jobs.rs
│   └── vector/
│       ├── mod.rs, ops/    # Vector ops: commands, input, qdrant, ranking, stats, tei
├── docker/
│   ├── Dockerfile          # Multi-stage build; s6-overlay for service supervision
│   └── s6/
│       ├── cont-init.d/    # 10-load-axon-env: loads .env on container startup
│       └── s6-rc.d/        # crawl-worker, batch-worker, extract-worker, embed-worker (+ user bundle)
├── docker-compose.yaml     # Full stack: postgres, redis, rabbitmq, qdrant, axon-workers
├── .env                    # Secrets (gitignored)
└── .env.example            # Template — copy to .env and fill in
```

## Infrastructure

### Docker Services

| Service | Image | Exposed Port | Purpose |
|---------|-------|-------------|---------|
| `axon-postgres` | postgres:17-alpine | `53432` | Job persistence |
| `axon-redis` | redis:8.2-alpine | `53379` | Queue state / caching |
| `axon-rabbitmq` | rabbitmq:4.0-management | `45535` | AMQP job queue |
| `axon-qdrant` | qdrant/qdrant:v1.13.1 | `53333`, `53334` (gRPC) | Vector store |
| `axon-webdriver` | selenium/standalone-chrome:4.34.0 | `4444` (WebDriver), `7900` (VNC) | Browser fallback |
| `axon-workers` | built from Dockerfile | — | 4 workers (crawl/batch/extract/embed) |

All services live on the `axon` bridge network. Data persisted to `/home/jmagar/appdata/axon-*`.

```bash
# Start all services
docker compose up -d

# Start just infrastructure (no workers)
docker compose up -d axon-postgres axon-redis axon-rabbitmq axon-qdrant

# Check health
docker compose ps

# Tail worker logs
docker compose logs -f axon-workers
```

### External Service: TEI (Text Embeddings Inference)

TEI is **not** in docker-compose — it's an external self-hosted service. Set `TEI_URL` in `.env`.

```bash
TEI_URL=http://YOUR_TEI_HOST:52000
```

## Environment Variables

Copy `.env.example` → `.env` and fill in values:

```bash
# Postgres
AXON_PG_URL=postgresql://axon:postgres@axon-postgres:5432/axon

# Redis
AXON_REDIS_URL=redis://:CHANGE_ME@axon-redis:6379

# RabbitMQ
AXON_AMQP_URL=amqp://axon:CHANGE_ME@axon-rabbitmq:5672

# Qdrant
QDRANT_URL=http://axon-qdrant:6333

# TEI embeddings (external — required for embed/query/ask)
TEI_URL=http://REPLACE_WITH_TEI_HOST:52000

# LLM (required for extract and ask commands)
OPENAI_BASE_URL=http://YOUR_LLM_HOST/v1
OPENAI_API_KEY=your-key-or-empty
OPENAI_MODEL=your-model-name

# WebDriver for browser fallback (axon-webdriver runs at localhost:4444)
AXON_WEBDRIVER_URL=http://127.0.0.1:4444

# Optional queue name overrides
AXON_CRAWL_QUEUE=axon.crawl.jobs
AXON_BATCH_QUEUE=axon.batch.jobs
AXON_EXTRACT_QUEUE=axon.extract.jobs
AXON_EMBED_QUEUE=axon.embed.jobs
AXON_COLLECTION=cortex              # Qdrant collection (default: cortex)
```

### Dev vs Container URL Resolution

The CLI auto-detects whether it's running inside Docker:
- **Inside Docker** (`/.dockerenv` exists): uses container-internal DNS (`axon-postgres:5432`, etc.)
- **Outside Docker** (local dev): rewrites to localhost with mapped ports (`127.0.0.1:53432`, etc.)

**So `.env` can use container DNS** — `normalize_local_service_url()` in `config.rs` handles translation transparently.

## Gotchas

### `--wait false` (default) = fire-and-forget
By default, `crawl`, `batch`, `extract`, and `embed` enqueue jobs and return immediately. Use `--wait true` to block until completion. Without workers running, enqueued jobs will pend forever.

### `render-mode auto-switch`
The default mode. Runs an HTTP crawl first; if >60% of pages are thin (<200 chars) or total coverage is too low, automatically retries with Chrome. Chrome requires a running Chrome instance — if none is available, the HTTP result is kept.

### `crawl_raw()` vs `crawl()`
When Chrome feature is compiled in, `crawl()` expects a Chrome instance. `crawl_raw()` is pure HTTP and always works. `engine.rs` calls `crawl_raw()` for `RenderMode::Http` and `crawl()` for Chrome/AutoSwitch.

### `ask` LLM call pattern
`ask` constructs the URL as: `{OPENAI_BASE_URL}/chat/completions`
- **Correct:** `OPENAI_BASE_URL=http://host/v1`
- **Wrong:** `OPENAI_BASE_URL=http://host/v1/chat/completions` — double path

### TEI batch size / 413 handling
`tei_embed()` in `vector/ops/tei.rs` auto-splits batches on HTTP 413 (Payload Too Large). Set `TEI_MAX_CLIENT_BATCH_SIZE` env var to control default chunk size (default: 64, max: 128).

### Text chunking
`chunk_text()` splits at 2000 chars with 200-char overlap. Each chunk = one Qdrant point. Very long pages produce many points.

### Thin page filtering
Pages with fewer than `--min-markdown-chars` (default: 200) are flagged as thin. If `--drop-thin-markdown true` (default), thin pages are skipped — not saved to disk or embedded.

### Collection must exist before upsert
`ensure_collection()` issues a PUT to Qdrant to create or update the collection with the correct vector dimension. This is idempotent — safe to call on every embed.

### Sitemap backfill
After a crawl, `append_sitemap_backfill()` discovers URLs via sitemap.xml that the crawler missed and fetches them individually. Respects `--max-sitemaps` (default: 512) and `--include-subdomains`.

### Docker build context
The `Dockerfile` builds from `docker/Dockerfile`. The build command inside the container is:
```
cargo build --release --bin axon
```
`docker-compose.yaml` sets `context: .` — run `docker compose build` from this directory, not from a parent workspace.

## Performance Profiles

Concurrency tuned relative to available CPU cores:

| Profile | Crawl concurrency | Sitemap concurrency | Backfill concurrency | Timeout | Retries | Backoff |
|---------|------------------|---------------------|----------------------|---------|---------|---------|
| `high-stable` (default) | CPUs×8 (64–192) | CPUs×12 (64–256) | CPUs×6 (32–128) | 20s | 2 | 250ms |
| `balanced` | CPUs×4 (32–96) | CPUs×6 (32–128) | CPUs×3 (16–64) | 30s | 2 | 300ms |
| `extreme` | CPUs×16 (128–384) | CPUs×20 (128–512) | CPUs×10 (64–256) | 15s | 1 | 100ms |
| `max` | CPUs×24 (256–1024) | CPUs×32 (256–1536) | CPUs×20 (128–1024) | 12s | 1 | 50ms |

## Development

### Build

```bash
cargo build --bin axon                          # debug
cargo build --release --bin axon                # release
cargo check                                     # fast type check
```

### Test

```bash
cargo test                    # run all tests
cargo test http               # SSRF / URL validation tests (21)
cargo test engine             # crawl engine tests (8)
cargo test chunk_text         # text chunking tests (7)
cargo test -- --nocapture     # show println! output
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

### Monolith Policy

Changed `.rs` files are enforced at CI and via lefthook pre-commit:
- File size: ≤ 500 lines (hard fail)
- Function size: warn at 80 lines, hard fail at 120 lines
- Exempt: `tests/**`, `benches/**`, `config/**`, `**/config.rs`
- Exceptions: add to `.monolith-allowlist`

```bash
./scripts/install-git-hooks.sh  # install lefthook once
```

### Diagnose service connectivity

```bash
axon doctor
```

Checks: Postgres, Redis, RabbitMQ, Qdrant, TEI, LLM endpoint reachability.

## Database Schema

Tables are auto-created via `ensure_schema()` in each `*_jobs.rs`. Full column detail: [`docs/schema.md`](docs/schema.md).

| Table | Key columns |
|-------|-------------|
| `axon_crawl_jobs` | `id`, `url`, `status`, `config_json`, `result_json` — index on `status` |
| `axon_batch_jobs` | `id`, `status`, `urls_json`, `config_json`, `result_json` |
| `axon_extract_jobs` | `id`, `status`, `urls_json`, `config_json`, `result_json` |
| `axon_embed_jobs` | `id`, `status`, `input_text`, `config_json`, `result_json` |

All tables share: `created_at`, `updated_at`, `started_at`, `finished_at` (TIMESTAMPTZ), `error_text` (TEXT).

## Code Style

- Rust standard style — run `cargo fmt` before committing
- `cargo clippy` clean before committing
- Errors bubble via `Box<dyn Error>` at command boundaries; internal helpers return typed errors
- Structured log output via `log_info` / `log_warn` (not `println!` in library code)
- `--json` flag enables machine-readable output on all commands that print results
