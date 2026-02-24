# crates/jobs — AMQP Job Workers

Async job workers backed by RabbitMQ (lapin) + PostgreSQL (sqlx).

## Module Layout

```text
jobs/
├── common/          # Shared infra: pool, AMQP channel, claim/mark/enqueue
├── crawl/           # manifest, processor, repo, sitemap, watchdog, worker, runtime
├── extract/         # Extract worker
├── embed/           # Embed worker
├── ingest.rs        # Ingest job schema + worker (github/reddit/youtube)
├── status.rs        # JobStatus enum
└── worker_lane.rs   # Multi-lane coordination for ingest
```

## Critical Patterns

### Job Lifecycle
Always use `common::` functions — never write raw SQL job state updates:

```text
claim_next_pending() → mark_job_started() → mark_job_completed() / mark_job_failed()
```

### JobStatus Enum (`status.rs`)
Use `JobStatus::Pending` etc. — **never** raw strings like `"pending"`, `"running"`, `"completed"`, `"failed"`, `"canceled"`. Serializes to the SQL strings automatically.

### PgPool — Create Once, Pass Down
PgPool is expensive. Each worker creates one pool at startup and passes `&PgPool` to all helper functions. Helpers are named `*_with_pool()`. Do not create pools inside loops or per-job handlers.

### AMQP Channel (`common/`)
`open_amqp_channel()` has a **5-second connection timeout**. On failure it returns an error — callers should backoff and retry at the worker loop level, not in the channel helper itself.

### Bounded Channels
All internal async channels use `tokio::sync::mpsc::channel(256)` — **never** `unbounded_channel()`. Unbounded channels hide backpressure bugs and cause OOM under load.

### Stale Job Recovery
- `watchdog.rs` (crawl_jobs): marks jobs stuck in `running` state as `failed` after `AXON_JOB_STALE_TIMEOUT_SECS` (default 300s) + `AXON_JOB_STALE_CONFIRM_SECS` (60s) grace period
- `axon crawl recover` subcommand: reclaims all stale jobs (re-queues them as `pending`)

### worker_lane.rs (Ingest)
`AXON_INGEST_LANES` (default 2) controls how many ingest jobs run in parallel. Each lane holds one AMQP consumer. Lane count is separate from per-job concurrency.

## ingest_jobs Schema Difference
`axon_ingest_jobs` uses `source_type` + `target` columns instead of `url`/`urls_json` used by all other job tables. When querying or listing ingest jobs, join/filter on `source_type` (`github`/`reddit`/`youtube`) not on `url`.

## Testing

```bash
cargo test jobs           # all job-related unit tests
cargo test common         # shared infra tests (pool, channel, claim/mark)
cargo test crawl_jobs     # crawl pipeline tests
cargo test status         # JobStatus enum serialization tests
cargo test -- --nocapture # show log output from tests
```

**Important:** Integration tests that exercise `make_pool`, `open_amqp_channel`, or `claim_next_pending` require live Postgres + RabbitMQ connections. Run `docker compose up -d axon-postgres axon-rabbitmq` before running integration tests. Unit tests (enum, serialization, rule engine) run without services.

## Adding a New Job Type
1. Create `<name>_jobs.rs` (or `<name>_jobs/` module if complex)
2. Call `ensure_schema()` in the worker startup — it's idempotent
3. Reuse `common::make_pool`, `open_amqp_channel`, `claim_next_pending`, `enqueue_job`
4. Add `CommandKind::<Name>` to `config.rs`
5. Add s6 worker script in `docker/s6/s6-rc.d/<name>-worker/`
