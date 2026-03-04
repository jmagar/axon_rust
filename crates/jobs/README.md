# crates/jobs
Last Modified: 2026-03-03

Asynchronous job runtime, queue integration, and lifecycle management.

## Purpose
- Provide durable async execution for crawl, extract, embed, and ingest jobs.
- Track job lifecycle in Postgres while RabbitMQ handles delivery.
- Expose operational controls (`status`, `cancel`, `errors`, `list`, `recover`, `worker`).

## Responsibilities
- Queue publish/consume wiring.
- Atomic claim/run/complete/fail state transitions.
- Worker lane orchestration and stale-job watchdog/recovery.
- Per-domain job family implementations (crawl/extract/embed/ingest).

## Key Files
- `status.rs`: shared job status model.
- `worker_lane.rs`: worker lane runtime orchestration (module root file).
- `common/amqp.rs`: queue transport helpers.
- `common/job_ops.rs`: atomic DB lifecycle operations.
- `common/watchdog.rs`: stale-job reclaim logic.
- `crawl.rs` + `crawl/*`: crawl worker runtime and persistence paths.
- `extract.rs` + `extract/worker.rs`: extract worker path.
- `embed.rs` + `embed/worker.rs`: embed worker path.
- `ingest.rs`: shared ingest worker path for GitHub/Reddit/YouTube.

## Integration Points
- Enqueue operations are initiated from `crates/cli/commands/*`.
- Crawl execution delegates into `crates/crawl`.
- Embed/query workflows interact with `crates/vector`.
- Runtime config and connections come from `crates/core/config` and `crates/jobs/common/pool.rs`.

## Notes
- Postgres is the source of truth for job state; queue delivery alone is not sufficient for lifecycle integrity.
- Recovery behavior should stay aligned with stale timeout and confirmation config.

## Related Docs
- [Repository README](../../README.md)
- [Architecture](../../docs/ARCHITECTURE.md)
- [Docs Index](../../docs/README.md)
