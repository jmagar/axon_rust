# crates/web — WebSocket Execution Bridge
Last Modified: 2026-02-26

## Role

`crates/web` is the axum WebSocket bridge consumed by `apps/web` (Next.js). It has no static UI of its own.

- The active frontend is `apps/web` — all UI decisions live there.
- `crates/web` handles: WebSocket connection lifecycle, CLI subprocess execution, Docker stats broadcasting, output file serving, and crawl artifact download endpoints.

## Source of Truth

For branding, theme, layout, and frontend UX decisions: `apps/web`.

## Directory Intent

- `crates/web.rs`: Axum server wiring and routes
- `crates/web/execute/`: subprocess execution + output streaming
- `crates/web/docker_stats.rs`: container stats streaming
- `crates/web/download.rs`: artifact download endpoints
- `crates/web/pack.rs`: output packaging helpers

## Agent Guidance

When asked to review or polish the frontend visual system, audit and update `apps/web` first.
