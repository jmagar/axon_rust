# crates/core
Last Modified: 2026-03-03

Shared runtime primitives used across CLI, jobs, crawl, vector, and web modules.

## Purpose
- Provide centralized config parsing/resolution.
- Standardize HTTP/content processing and safety checks.
- Keep logging/health/UI utilities reusable across subsystems.

## Responsibilities
- CLI/env configuration schema and merge logic.
- Runtime HTTP client and safety controls.
- HTML/content normalization and markdown extraction helpers.
- Health probes and user-facing output utilities.

## Key Files
- `config.rs`: config module root.
- `config/cli.rs`: clap argument schema.
- `config/parse.rs`: env + flag merge and normalization logic.
- `config/types.rs`: canonical `Config` shape consumed by handlers/workers.
- `config/parse/performance.rs`: performance profile defaults and overrides.
- `http.rs`: HTTP fetch and request safety behaviors.
- `content.rs` + `content/deterministic.rs`: deterministic content handling utilities.
- `logging.rs`: structured logging helpers used across runtime.
- `health.rs`: service health-check helpers.

## Integration Points
- `lib.rs` command dispatch consumes config produced here.
- `crates/cli` command handlers depend on `Config` and utility helpers.
- `crates/crawl` and `crates/vector` use HTTP/content layers.
- `crates/jobs` workers use config, health, and logging utilities.

## Notes
- Config changes should be coordinated with command handlers and test config builders that construct `Config` literals.
- Keep environment and flag precedence rules centralized in `config/parse.rs`.

## Related Docs
- [Repository README](../../README.md)
- [Architecture](../../docs/ARCHITECTURE.md)
- [Docs Index](../../docs/README.md)
