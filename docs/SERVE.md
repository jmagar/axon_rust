# `axon serve` — WebSocket Execution Bridge
Last Modified: 2026-02-26

Version: 1.1.0
Last Updated: 02/26/2026

## Overview

`axon serve` starts the axum WebSocket bridge used by `apps/web`. It has no static UI of its own — the frontend is the Next.js app in `apps/web`.

Current canonical WebSocket contract documentation lives in [`docs/API.md`](API.md).

## Usage

```bash
axon serve              # default port 49000
axon serve --port 8080  # custom port
```

The server exposes HTTP endpoints and a WebSocket at `/ws`. Connect the Next.js frontend (`apps/web`) to this backend via `AXON_BACKEND_URL`.

## Architecture

```
apps/web ──────▶ axum (single port, single binary)
                 │
                 ├── GET /output/{*path}          → serve generated output files
                 ├── GET /download/{id}/pack.md   → crawl artifact download
                 ├── GET /download/{id}/pack.xml
                 ├── GET /download/{id}/archive.zip
                 ├── GET /download/{id}/file/{*path}
                 │
                 └── WS /ws                       → multiplexed by "type" field
                     │
                     ├── client→server: {"type":"execute","mode":"scrape","input":"https://...","flags":{}}
                     │   server spawns: tokio::process::Command("axon scrape --json --wait true ...")
                     │   server→client: {"type":"output","line":"..."} per stdout line
                     │   server→client: {"type":"done","exit_code":0,"elapsed_ms":1823}
                     │
                     ├── client→server: {"type":"cancel","id":"<job_uuid>"}
                     │   server spawns: axon crawl cancel <id> --json
                     │
                     └── server→client (broadcast): {"type":"stats","containers":{...},"aggregate":{...}}
                         └── bollard polls Docker socket every 500ms
```

## Key Design Decisions

1. **Subprocess execution** — Commands run via `tokio::process::Command` spawning the same binary with `--json --wait true`. This means zero refactoring of existing commands, and a crashing command doesn't take down the server.

2. **`std::env::current_exe()`** — The server spawns itself with different args. Single binary, no external dependencies.

3. **Single WebSocket, multiplexed** — One WebSocket at `/ws` handles both command execution responses and Docker stats broadcasts. No separate connections needed.

4. **Flag whitelisting** — Only known flag names (`--max-pages`, `--limit`, `--collection`, etc.) are passed through to subprocess args. User input is never used as raw CLI args (command injection prevention).

5. **Bollard graceful degradation** — If the Docker socket is unavailable, stats broadcasting is silently disabled. The server still works for command execution.

## Modules

| File | Purpose | Lines |
|------|---------|-------|
| `crates/web.rs` | Axum server, routes, WS handler, shared state | ~177 |
| `crates/web/execute.rs` | Subprocess spawn, stdout streaming, flag whitelist | ~236 |
| `crates/web/docker_stats.rs` | Bollard Docker stats poller, rate calculations, broadcast | ~281 |
| `crates/cli/commands/serve.rs` | `run_serve()` entry point | ~6 |

## WebSocket Protocol

All messages are JSON with a `type` field:

### Client → Server

```json
{"type": "execute", "mode": "scrape", "input": "https://example.com", "flags": {"limit": 10}}
{"type": "cancel", "id": "uuid-of-crawl-job"}
```

### Server → Client

```json
{"type": "output", "line": "{\"url\":\"...\",\"markdown\":\"...\"}"}
{"type": "done", "exit_code": 0, "elapsed_ms": 1823}
{"type": "error", "message": "exit code 1", "stderr": "...", "elapsed_ms": 400}
{"type": "stats", "container_count": 6, "containers": {...}, "aggregate": {...}}
```

## Allowed Modes

Only these command modes can be executed from the UI (whitelist in `execute.rs`):

`scrape`, `crawl`, `map`, `extract`, `search`, `research`, `embed`, `debug`, `doctor`, `query`, `retrieve`, `ask`, `evaluate`, `suggest`, `sources`, `domains`, `stats`, `status`, `dedupe`, `github`, `reddit`, `youtube`, `sessions`, `screenshot`

## Allowed Flags

Only these flags can be passed from the UI (whitelist in `execute.rs`):

| JSON Key | CLI Flag |
|----------|----------|
| `max_pages` | `--max-pages` |
| `max_depth` | `--max-depth` |
| `limit` | `--limit` |
| `collection` | `--collection` |
| `format` | `--format` |
| `render_mode` | `--render-mode` |
| `include_subdomains` | `--include-subdomains` |
| `discover_sitemaps` | `--discover-sitemaps` |
| `embed` | `--embed` |
| `diagnostics` | `--diagnostics` |

## Docker Stats

The stats poller connects to the Docker socket via `bollard::Docker::connect_with_local_defaults()` and:

1. Lists containers matching `axon-*` prefix with status `running`
2. For each container, fetches one-shot stats
3. Computes: CPU% (docker stats formula), memory (usage - cache), network I/O rates, block I/O rates
4. Broadcasts the aggregated JSON to all connected WebSocket clients every 500ms
5. The frontend maps per-container CPU to neuron cluster EPSP injection, and network I/O to extra action potential firing

