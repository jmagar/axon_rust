# ⚡ **Axon**

Self-hosted web crawling and RAG pipeline powered by Spider.rs. This is the standalone Axon Rust repo target: `jmagar/axon`.

## Overview

Axon provides one CLI surface for scraping, crawling, extraction, embedding, and local semantic retrieval. It runs against a local Docker stack (Postgres, Redis, RabbitMQ, Qdrant) plus external model endpoints (TEI and OpenAI-compatible API).

## Features

- Unified CLI commands: `scrape`, `crawl`, `map`, `search`, `batch`, `extract`, `embed`, `query`, `retrieve`, `ask`, `status`, `doctor`
- Async job model for `crawl`/`batch`/`extract`/`embed` with queue-backed workers
- Qdrant vector storage with TEI embeddings
- OpenAI-compatible extraction and RAG answer path
- JSON output mode for automation (`--json`)

## Architecture / Services

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

All services run on the `cortex` bridge network with persistent host volumes under `/home/jmagar/appdata/axon-*`.

## Quick Start

```bash
# 1) from axon_rust
cp .env.example .env
# edit .env with your TEI/OpenAI endpoints

# 2) start stack

docker compose up -d

docker compose ps
```

```bash
# 3) run CLI
cargo run --bin axon -- doctor
cargo run --bin axon -- scrape https://example.com --wait true
cargo run --bin axon -- crawl https://docs.rs/spider --wait false
cargo run --bin axon -- status
```

```bash
# Optional local alias to match standalone naming
alias axon='cargo run --bin axon --'

axon doctor
axon query "spider crawler"
axon ask "what does spider.rs support?"
```

## Env Setup

Copy `.env.example` to `.env` and set:

- `AXON_PG_URL`, `AXON_REDIS_URL`, `AXON_AMQP_URL`
- `QDRANT_URL`
- `TEI_URL` (required for embed/query/ask)
- `OPENAI_BASE_URL`, `OPENAI_API_KEY`, `OPENAI_MODEL` (required for extract/ask)
- Optional queue overrides: `AXON_CRAWL_QUEUE`, `AXON_BATCH_QUEUE`, `AXON_EXTRACT_QUEUE`, `AXON_EMBED_QUEUE`

Notes:
- Container runtime uses service DNS names (`axon-postgres`, `axon-redis`, etc.).
- Local runtime rewrites those to mapped localhost ports automatically.

## Worker Model (s6 Supervised)

`axon-workers` uses `s6-overlay` and runs four long-lived worker services in one container:

- `crawl-worker` -> `axon_cli_rust crawl worker`
- `batch-worker` -> `axon_cli_rust batch worker`
- `extract-worker` -> `axon_cli_rust extract worker`
- `embed-worker` -> `axon_cli_rust embed worker`

Startup loads `.env` via `docker/s6/cont-init.d/10-load-axon-env`. Health checks verify each worker process and its log service.

## Troubleshooting

- `axon doctor` (or cargo equivalent) for service reachability (Postgres/Redis/AMQP/Qdrant/TEI/OpenAI)
- `docker compose logs -f axon-workers` to inspect worker failures
- Jobs stuck in pending: ensure `axon-workers` is healthy and AMQP/Redis are reachable
- `ask`/`extract` failures: verify `OPENAI_BASE_URL` is a base URL (for example `http://host/v1`, not `/chat/completions`)
- `embed`/`query` failures: verify `TEI_URL` and `QDRANT_URL`
