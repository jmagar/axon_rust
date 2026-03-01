# crates/crawl
Last Modified: 2026-02-25

Crawl engine and crawl artifact manifest logic for Axon.

## Purpose
- Execute crawl runs for one or more URLs with selected render mode (`http`, `chrome`, `auto-switch`).
- Normalize and persist crawl outputs and metadata.
- Expand coverage using sitemap backfill.

## Responsibilities
- Core crawl orchestration and page collection.
- Render-mode fallback policy (including auto-switch behavior).
- Thin-page filtering and crawl output assembly.
- Crawl manifest generation and bookkeeping.

## Key Files
- `engine.rs`: crawl orchestration entry and render-mode control.
- `engine/collector.rs`: crawl collection pipeline.
- `engine/sitemap.rs`: sitemap discovery/backfill logic.
- `manifest.rs`: crawl manifest model and persistence helpers.
- `engine/tests.rs`: crawl engine behavior tests.

## Integration Points
- Invoked by `crates/cli/commands/crawl*`.
- Downstream async processing and status tracking live in `crates/jobs/crawl/*`.
- Embedding handoff flows into `crates/vector/ops` when enabled.

## Notes
- Keep crawl behavior and job lifecycle concerns separated: traversal belongs here; queue and persistence state belong in `crates/jobs`.
- Manifest format changes should be validated against downstream consumers that read crawl artifacts.
