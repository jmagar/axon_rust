# Deployment Guide
Last Modified: 2026-02-25

Version: 1.0.0
Last Updated: 01:26:53 | 02/25/2026 EST

## Table of Contents

1. Scope
2. Deployment Targets
3. Prerequisites
4. Configuration
5. Standard Deploy Procedure
6. Validation Checklist
7. Rollback Procedure
8. Upgrade Procedure
9. Notes for Web UI Paths
10. Source Map

## Scope

This guide defines how to deploy and roll back Axon safely in self-hosted environments using Docker Compose.

## Deployment Targets

- Primary: Docker Compose stack defined in `docker-compose.yaml`
- Optional runtime clients: CLI (`./scripts/axon`) and Next.js app (`apps/web`)

## Prerequisites

Required:

- Docker Engine + Compose plugin
- Valid `.env` file (copied from `.env.example`)
- Reachable TEI (Text Embeddings Inference) service
- Reachable OpenAI-compatible LLM endpoint

Recommended:

- Sufficient disk under `AXON_DATA_DIR`
- CPU/memory headroom for worker container limits

## Configuration

Populate `.env` before first deploy:

- Service credentials (`POSTGRES_*`, `REDIS_PASSWORD`, `RABBITMQ_*`)
- Service URLs (`AXON_PG_URL`, `AXON_REDIS_URL`, `AXON_AMQP_URL`, `QDRANT_URL`)
- Model endpoints (`TEI_URL`, `OPENAI_*`)
- Optional tuning (`AXON_*_QUEUE`, watchdog, performance settings)

Ensure `.env` is never committed. `.env.example` remains tracked.

## Standard Deploy Procedure

1. Pull/build images:

```bash
docker compose build
```

1. Start infrastructure (workers and web run locally, not in Docker):

```bash
docker compose up -d axon-postgres axon-redis axon-rabbitmq axon-qdrant axon-chrome
```

Then start workers and web locally:

```bash
# Each in a separate terminal
cargo run --bin axon -- crawl worker
cargo run --bin axon -- embed worker
cargo run --bin axon -- extract worker
cd apps/web && pnpm dev
```

1. Verify health:

```bash
docker compose ps
./scripts/axon doctor
```

1. Smoke test:

```bash
./scripts/axon scrape https://example.com --wait true
./scripts/axon status
```

1. Observe workers:

Workers run in the foreground locally — output is in the terminal directly. For infra logs:

```bash
docker compose logs --tail=200 axon-postgres axon-redis axon-rabbitmq axon-qdrant
```

## Validation Checklist

- All infra containers report healthy (postgres, redis, rabbitmq, qdrant, chrome).
- Worker processes are running in their terminals.
- Web frontend is reachable at http://localhost:49010.
- `doctor` passes critical services.
- At least one sync command succeeds (`scrape`).
- At least one async command enqueues and reaches terminal state.
- No repeated watchdog reclaim warnings after warm-up.

## Rollback Procedure

Rollback is compose-based and image-based.

1. Stop current stack:

```bash
docker compose down
```

1. Revert deployment inputs:

- previous git revision for compose/docker files
- previous env values if changed
- previous image tags if using tagged images

1. Rebuild/restart:

```bash
docker compose build
docker compose up -d axon-postgres axon-redis axon-rabbitmq axon-qdrant axon-chrome
```

Restart workers and web locally as in the standard deploy procedure.

1. Re-run validation checklist.

## Upgrade Procedure

For code/config upgrades:

1. Review release diff (especially schema, queue names, env vars).
1. Update `.env` with any new required values.
1. Rebuild and redeploy with standard procedure.
1. Run lifecycle checks:

```bash
./scripts/axon status
./scripts/axon crawl list
./scripts/axon ingest list
```

1. If stale runs from prior version appear, run recover commands.

## Notes for Web UI Paths

- Active UI: `apps/web` (Next.js)
- Core runtime bridge: `axon serve` (`crates/web.rs` + `crates/web/*`) backing `/ws`, `/ws/shell`, `/download/*`, and `/output/*`
- Deprecated piece: only the old standalone static serve page UX

If deploying Next.js:

- Ensure web runtime sees required env vars (`OPENAI_*`, `TEI_URL`, `QDRANT_URL` as needed).
- Validate `/api/pulse/chat` and `/api/omnibox/files` responses.

## Source Map

- `docker-compose.yaml`
- `README.md`
- `docs/OPERATIONS.md`
- `docs/JOB-LIFECYCLE.md`
- `apps/web/app/api/*`
