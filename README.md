# ⚡ **Axon**

Self-hosted web crawling and RAG pipeline powered by Spider.rs. This is the standalone Axon Rust repo target: `jmagar/axon_rust`.

## Overview

Axon is a single CLI for crawl/scrape/extract plus local vector retrieval and Q&A. It runs on a local Docker stack (Postgres, Redis, RabbitMQ, Qdrant) and external model endpoints (TEI and OpenAI-compatible API).

## Features

- Commands: `scrape`, `crawl`, `map`, `search`, `batch`, `extract`, `embed`, `query`, `retrieve`, `ask`, `sources`, `domains`, `stats`, `status`, `doctor`, `debug`
- Async queue-backed jobs for `crawl`/`batch`/`extract`/`embed`
- TEI embeddings + Qdrant vector storage
- OpenAI-compatible extraction and answer generation
- Automation-friendly JSON mode via `--json`

## Architecture

### Crate Layout (`crates/*`)

- `crates/cli` - command routing and UX
- `crates/core` - config, HTTP, health checks, logging, content transforms
- `crates/crawl` - crawling engine and sitemap backfill
- `crates/extract` - remote structured extraction
- `crates/jobs` - queue workers for crawl/batch/extract/embed
- `crates/vector` - embeddings + Qdrant operations (`query/retrieve/ask/sources/domains/stats`)

### Docker Services (`docker-compose.yaml`)

- `axon-postgres` -> `localhost:53432`
- `axon-redis` -> `localhost:53379`
- `axon-rabbitmq` -> `localhost:45535`
- `axon-qdrant` -> `localhost:53333` (HTTP), `53334` (gRPC)
- `axon-workers` (s6-supervised worker container)

Services run on the `cortex` bridge network with persistent volumes under `/home/jmagar/appdata/axon-*`.

## Quick Start

```bash
# 1) from repo root
cp .env.example .env
# edit .env

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
# Optional local alias to match standalone naming
alias axon='./scripts/axon'

axon doctor
axon query "spider crawler"
axon ask "what does spider.rs support?"
```

## Environment

Copy `.env.example` to `.env`, then set at minimum:

- `AXON_PG_URL`, `AXON_REDIS_URL`, `AXON_AMQP_URL`
- `QDRANT_URL`
- `TEI_URL` (required for embed/query/ask)
- `AXON_WEBDRIVER_URL` (optional browser fallback for dynamic sites; use a base URL like `http://127.0.0.1:4444`)
- `OPENAI_BASE_URL`, `OPENAI_API_KEY`, `OPENAI_MODEL` (required for extract/ask/debug)
- Optional queues: `AXON_CRAWL_QUEUE`, `AXON_BATCH_QUEUE`, `AXON_EXTRACT_QUEUE`, `AXON_EMBED_QUEUE`
- Optional watchdog tuning: `AXON_JOB_STALE_TIMEOUT_SECS`, `AXON_JOB_STALE_CONFIRM_SECS`
- Full required/optional env coverage is documented in `.env.example`

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

Startup loads `.env` via `docker/s6/cont-init.d/10-load-axon-env`. Health checks verify each worker process and its log service.

Worker behavior notes:
- Workers run startup stale-job reclaim sweeps plus periodic stale sweeps.
- `crawl`/`batch`/`extract`/`embed` workers run with 2 lanes for higher throughput.

## Troubleshooting

- `axon doctor` (or cargo equivalent) for service reachability (Postgres/Redis/AMQP/Qdrant/TEI/OpenAI)
- `axon debug` to run doctor + LLM-assisted troubleshooting with your configured OpenAI-compatible endpoint
- `docker compose logs -f axon-workers` to inspect worker failures
- Jobs stuck in pending: ensure `axon-workers` is healthy and AMQP/Redis are reachable
- Manually reclaim stale jobs if needed:
  - `axon crawl recover`
  - `axon batch recover`
  - `axon extract recover`
  - `axon embed recover`
- `ask`/`extract` failures: verify `OPENAI_BASE_URL` is a base URL (for example `http://host/v1`, not `/chat/completions`)
- `embed`/`query` failures: verify `TEI_URL` and `QDRANT_URL`
- Browser fallback failures: verify `AXON_WEBDRIVER_URL` points to a live WebDriver endpoint (for example `http://127.0.0.1:4444`, not `.../wd/hub`)

## Monolith Guardrails

Axon enforces a ratcheting monolith policy on changed code:

- File size limit (changed files): `400` lines
- Rust function size limit (changed functions): `80` lines
- Test files are exempt (`tests/**`, `**/*_test.*`, `**/*.test.*`, `**/*.spec.*`, `benches/**`)
- Temporary file-level exceptions can be added to `.monolith-allowlist`

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
