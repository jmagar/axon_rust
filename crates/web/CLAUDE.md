# crates/web — WebSocket Execution Bridge
Last Modified: 2026-02-27

## Role

`crates/web` is the axum WebSocket bridge consumed by `apps/web` (Next.js). It has no static UI of its own.

- The active frontend is `apps/web` — all UI decisions live there.
- `crates/web` handles: WebSocket connection lifecycle, CLI subprocess execution, Docker stats broadcasting, output file serving, and crawl artifact download endpoints.

## Source of Truth

For branding, theme, layout, and frontend UX decisions: `apps/web`.

## Directory Intent

- `crates/web.rs`: Axum server wiring and routes
- `crates/web/execute.rs`: subprocess launch + WS output pump entry point
- `crates/web/execute/events.rs`: WS event type definitions
- `crates/web/execute/files.rs`: output file serving for completed jobs
- `crates/web/execute/tests/`: execute integration tests
- `crates/web/docker_stats.rs`: container stats streaming
- `crates/web/download.rs`: HTTP endpoints for crawl artifact downloads (individual files + zip archives)
- `crates/web/pack.rs`: output packaging helpers — assembles crawl results into downloadable bundles
- `crates/web/logs/`: log streaming support
- `crates/web/snapshots/`: insta snapshot files for integration tests (committed; update with `cargo insta review`)

## WebSocket Protocol

Single multiplexed WS connection. Messages keyed by `"type"`:

| Direction | type | Payload |
|-----------|------|---------|
| Client → Server | `execute` | `{ "mode": "...", "input": "..." }` |
| Client → Server | `cancel` | `{ "job_id": "..." }` |
| Server → Client | `output` | stdout JSON data |
| Server → Client | `log` | stderr progress/spinner text |
| Server → Client | `done` | job completed |
| Server → Client | `error` | job failed with message |
| Server → Client | `stats` | Docker container stats (polled every 500ms via bollard) |

ANSI codes are stripped from log output via `console::strip_ansi_codes()`.

## Security Model

`execute.rs` enforces strict whitelists before spawning any subprocess:
- **`ALLOWED_MODES`**: list of valid CLI subcommands (e.g. `scrape`, `crawl`, `ask`) — rejects anything not in this list
- **`ALLOWED_FLAGS`**: set of permitted CLI flags — rejects unknown flags

Unknown modes or flags return an error WS event without spawning a process. Do not bypass these whitelists when adding new execute routes.

## Docker Stats Caveat

`docker_stats.rs` polls bollard for container stats every 500ms and broadcasts to all WS clients. This requires `/var/run/docker.sock` to be mounted. When running inside `axon-workers`, the socket is **not** mounted — stats will be silently unavailable. HTTP/WS endpoints remain functional.

## Ports

| Binding | Purpose |
|---------|---------|
| `0.0.0.0:49000` | HTTP + WebSocket server (`axon serve`) |

Port 49000 is the backend server port. The Next.js frontend (`apps/web`) at port 49010 proxies to it.

## Adding a New HTTP Endpoint

1. Add the route in `crates/web.rs` (`Router::new().route(...)`)
2. Implement the handler as a free function in the appropriate `crates/web/` module
3. Add the handler to `ALLOWED_MODES` or `ALLOWED_FLAGS` if it involves subprocess execution
4. Write an integration test in `crates/web/execute/tests/` or a snapshot test if response shape is fixed

## Agent Guidance

When asked to review or polish the frontend visual system, audit and update `apps/web` first.

## Testing

```bash
cargo test web            # WS bridge + execute pipeline tests
cargo test download       # artifact download endpoint tests
cargo test -- --nocapture # show subprocess output during tests
```

Snapshot tests use `insta`. After intentional response shape changes, update snapshots:

```bash
cargo test web -- --nocapture   # run to generate new snapshots
cargo insta review              # approve/reject each diff interactively
```

Snapshot files live in `crates/web/snapshots/` and are committed to git.
