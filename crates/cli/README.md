# crates/cli
Last Modified: 2026-02-25

CLI command routing and command handlers for the `axon` binary.

## Purpose
- Translate parsed command/config state into concrete command execution.
- Keep command-specific orchestration out of `lib.rs` dispatch.
- Provide shared command helpers for URL parsing, job control, and status output.

## Responsibilities
- Command entrypoint modules under `commands/` (`scrape`, `crawl`, `extract`, `embed`, `search`, `doctor`, `status`, ingest commands, and web serve command).
- Subcommand lifecycle actions for async jobs (`status`, `cancel`, `errors`, `list`, `cleanup`, `clear`, `recover`, `worker`) where applicable.
- Shared parsing and command wiring helpers in `commands/common.rs` and `commands/job_contracts.rs`.

## Key Files
- `commands.rs`: top-level command module surface.
- `commands/common.rs`: shared URL/argument helpers used across command modules.
- `commands/crawl.rs` + `commands/crawl/*`: crawl command flow and sync/runtime variants.
- `commands/status.rs` + `commands/status/metrics.rs`: queue and runtime status reporting.
- `commands/doctor.rs` + `commands/doctor/render.rs`: connectivity checks and doctor output rendering.
- `commands/ingest.rs` + `commands/ingest_common.rs`: shared ingest CLI wiring.

## Integration Points
- Receives `Config` resolved by `crates/core/config/*`.
- Calls crawl runtime in `crates/crawl`.
- Dispatches async workloads into `crates/jobs` workers/queues.
- Uses vector operations in `crates/vector/ops` for query/retrieve/ask/evaluate flows.
- Bridges web execution path via `commands/serve.rs`.

## Notes
- This module is orchestration-heavy; avoid embedding low-level business logic here.
- Shared command behavior should be centralized in `common.rs`/`ingest_common.rs` to prevent drift across subcommands.
