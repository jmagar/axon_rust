# Operations Runbook
Last Modified: 2026-02-25

Version: 1.0.0
Last Updated: 01:26:53 | 02/25/2026 EST

## Table of Contents

1. Scope
2. Day 0 Prerequisites
3. Day 1 Startup
4. Health Checks
5. Daily Operations
6. Incident Playbooks
7. Job Queue Operations
8. Data and Storage Hygiene
9. Logs and Diagnostics
10. Safe Shutdown
11. Source Map

## Scope

This is the operator runbook for local/homelab operation of Axon.

## Day 0 Prerequisites

1. Copy env template:

```bash
cp .env.example .env
```

2. Populate required values in `.env`:

- Postgres credentials and `AXON_PG_URL`
- Redis password and `AXON_REDIS_URL`
- RabbitMQ credentials and `AXON_AMQP_URL`
- `QDRANT_URL`
- `TEI_URL`
- `OPENAI_BASE_URL`, `OPENAI_API_KEY`, `OPENAI_MODEL`

3. Ensure Docker and Compose are healthy.

## Day 1 Startup

### Full stack

```bash
docker compose up -d
docker compose ps
```

### Infrastructure only

```bash
docker compose up -d axon-postgres axon-redis axon-rabbitmq axon-qdrant axon-chrome
docker compose ps
```

### Verify service reachability

```bash
./scripts/axon doctor
```

### Tail workers

```bash
docker compose logs -f axon-workers
```

## Health Checks

Expected healthy containers:

- `axon-postgres`
- `axon-redis`
- `axon-rabbitmq`
- `axon-qdrant`
- `axon-chrome`
- `axon-workers`

Quick checks:

```bash
docker compose ps
./scripts/axon status
```

If any are unhealthy, inspect logs before restart.

## Daily Operations

### Run crawl/scrape

```bash
./scripts/axon scrape https://example.com --wait true
./scripts/axon crawl https://docs.rs/spider --wait false
```

### Track async progress

```bash
./scripts/axon status
./scripts/axon crawl list
./scripts/axon crawl status <job_id>
```

### Query/RAG

```bash
./scripts/axon query "vector search"
./scripts/axon ask "what did we index for X?"
```

## Incident Playbooks

### Jobs stuck in `pending`

1. Confirm worker container health:

```bash
docker compose ps
docker compose logs --tail=200 axon-workers
```

2. Confirm AMQP and DB reachable:

```bash
./scripts/axon doctor
```

3. Restart worker service:

```bash
docker compose restart axon-workers
```

### Jobs stuck in `running`

1. Trigger manual recover:

```bash
./scripts/axon crawl recover
./scripts/axon extract recover
./scripts/axon embed recover
./scripts/axon ingest recover
```

2. If repeated, inspect worker logs and watchdog configuration.

### Pulse/API returning 503

Cause: missing LLM env vars in web runtime.

Required:

- `OPENAI_BASE_URL`
- `OPENAI_API_KEY`

Also needed for retrieval features:

- `TEI_URL`
- `QDRANT_URL`

## Job Queue Operations

Runbook commands:

```bash
./scripts/axon crawl list
./scripts/axon crawl errors <job_id>
./scripts/axon crawl cancel <job_id>
./scripts/axon crawl cleanup
```

Same pattern applies to `extract`, `embed`, and `ingest`.

## Data and Storage Hygiene

Persistent data roots are under `${AXON_DATA_DIR}/axon/...`:

- Postgres data
- Redis appendonly data
- RabbitMQ data
- Qdrant storage
- Worker output and logs
- MCP artifacts (`${AXON_DATA_DIR}/axon/artifacts` when `AXON_MCP_ARTIFACT_DIR` is set)

Cleanup caution:

- `clear` and aggressive cleanup commands are destructive.
- Use `list` and `status` first.

Cache and build-context guardrails:

```bash
# inspect local target/ + BuildKit cache sizes
just cache-status

# enforce size thresholds (prunes incremental/target and/or BuildKit cache)
just cache-prune

# run live Docker context-size probe for axon-workers + axon-web
just docker-context-probe
```

Threshold tuning (optional):

- `AXON_TARGET_MAX_GB` (default `30`)
- `AXON_BUILDKIT_MAX_GB` (default `120`)
- `AXON_WORKERS_CONTEXT_MAX_MB` (default `500`)
- `AXON_WEB_CONTEXT_MAX_MB` (default `100`)
- `AXON_CONTEXT_PROBE_TIMEOUT_SECS` (default `30`)

`scripts/rebuild-fresh.sh` runs cache guard + context probe automatically unless disabled:

- `AXON_AUTO_CACHE_GUARD=false`
- `AXON_ENFORCE_DOCKER_CONTEXT_PROBE=false`

## Logs and Diagnostics

Primary logs:

```bash
docker compose logs -f axon-workers
docker compose logs -f axon-rabbitmq
docker compose logs -f axon-qdrant
```

Structured app logs are written under mounted logs volume for workers.

Chrome diagnostics:

- controlled by `AXON_CHROME_DIAGNOSTICS*` env vars
- output directory defaults to configured diagnostics path

## Safe Shutdown

Graceful shutdown:

```bash
docker compose down
```

If draining is needed first:

1. Stop new submissions.
2. Monitor active jobs with `status`.
3. Cancel remaining long-running jobs if required.
4. Bring stack down.

## Source Map

- `docker-compose.yaml`
- `README.md`
- `crates/jobs/*`
- `crates/web.rs`
- `apps/web/app/api/*`
