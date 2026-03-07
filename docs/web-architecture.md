# Web Architecture

**Tracking issue:** A-M-01
**Status:** Documentation only — consolidation not yet started
**Last updated:** 2026-03-04

---

## Table of Contents

1. [Current State](#current-state)
2. [Identified Issues](#identified-issues)
3. [Recommended Consolidation Path](#recommended-consolidation-path)
4. [What Would Need to Change](#what-would-need-to-change)

---

## Current State

Axon runs two independent web servers:

### Server 1: axum HTTP/WebSocket (port 49000)

Source: `crates/web.rs` + `crates/web/`

Serves:
- `GET /` — static HTML shell (embedded via `include_str!` in release, disk reads in debug)
- `GET /static/*` — CSS, JS assets
- `WebSocket /ws` — command execution bridge (runs axon subcommands as subprocesses, streams stdout/stderr back)
- Docker stats broadcast (bollard integration, pushed over WebSocket)

Authentication: Token-based via `AXON_SHELL_WS_TOKEN` env var (checked in WebSocket upgrade handler).

Started by: `cargo run --bin axon -- serve` (local process, runs on port 49000). In Docker this was managed by `docker/s6-rc.d/axon-workers/`.

### Server 2: Next.js dev server (port 49010)

Source: `apps/web/`

Serves:
- Omnibox UI (`/`) — command input, result display
- Pulse workspace (`/pulse`) — Claude Code CLI integration
- `/api/*` — proxy routes that forward to the axum server (port 49000)

Authentication: `AXON_WEB_API_TOKEN` enforced by `apps/web/proxy.ts` on all `/api/*` routes. Client attaches via `x-api-key` header.

Started by: `cd apps/web && pnpm dev` (local process, port 49010). In Docker this was managed by `docker/s6-rc.d/pnpm-dev/` in the `axon-web` container.

### Communication Flow

```
Browser → Next.js (49010) → proxy.ts → axum (49000) → subprocesses
                                     → WebSocket /ws
```

---

## Identified Issues

### 1. Dual authentication layers

- axum has its own token check (`AXON_SHELL_WS_TOKEN`)
- Next.js proxy has its own token check (`AXON_WEB_API_TOKEN`)
- These are different tokens, requiring both to be set correctly
- If they diverge (different values in `.env`), requests silently fail with auth errors that are difficult to diagnose

### 2. Two error response formats

- axum returns raw JSON or plain text depending on the handler
- Next.js API routes wrap errors in Next.js error format
- A client consuming `/api/*` sees Next.js errors; direct access to port 49000 sees axum errors
- The MCP HTTP transport hits axum directly (port 49000), bypassing the Next.js error formatting

### 3. Proxy complexity and latency

Every browser request:
1. Hits Next.js on port 49010
2. Is validated by `proxy.ts`
3. Is forwarded to axum on port 49000
4. Response is returned upstream

This adds latency and a failure point. If axum is unavailable, the Next.js proxy returns an opaque 502 with no diagnostic detail.

### 4. Static asset duplication

- axum embeds `crates/web/static/` assets (neural.js, app.js, style.css, index.html)
- Next.js has its own complete asset pipeline
- Both are served, but to different UIs — the axum UI (port 49000 direct) and the Next.js UI (port 49010)
- Changes to shared behavior (e.g., command format) must be updated in both UIs

### 5. Port management

- Port 49000 is bound by the workers container (also runs job workers)
- Port 49010 is the Next.js dev server
- External clients must target 49010 (the Next.js server) for the full UI
- The MCP HTTP transport targets 49000 directly
- SWAG reverse proxy must route traffic to the correct internal port

---

## Recommended Consolidation Path

Absorb axum's responsibilities into Next.js API routes. The result is a single server on a single port.

### Target Architecture

```
Browser / MCP client → Next.js (49010 only)
  /api/execute      ← replaces axum WebSocket execute endpoint
  /api/ws           ← replaces axum WebSocket (via Next.js WebSocket support or SSE)
  /api/pulse/*      ← existing (unchanged)
  /api/mcp/*        ← existing MCP HTTP transport
```

### Key Changes Required

1. **Port 49000 goes away.** The axum server is removed or demoted to an internal-only health endpoint.

2. **WebSocket execution moves to Next.js.** Next.js supports WebSockets natively (experimental) or the execute endpoint can switch to Server-Sent Events (SSE) for the streaming response. SSE is simpler and works without WebSocket server support.

3. **Docker stats stream moves to Next.js.** Replace bollard → axum WebSocket broadcast with bollard → SSE endpoint in a Next.js API route.

4. **Single auth token.** Remove `AXON_SHELL_WS_TOKEN`. Use `AXON_WEB_API_TOKEN` everywhere. All authenticated routes go through the same `proxy.ts` middleware.

5. **Remove the proxy hop.** API routes call axon subprocesses directly via `child_process.spawn` (Node.js) rather than proxying to axum. This eliminates the internal proxy and the second auth layer.

6. **Static assets served by Next.js.** `crates/web/static/` and `crates/web.rs` can be removed. The Next.js build pipeline handles all static assets.

---

## What Would Need to Change

### Remove / demote

- `crates/web.rs` — axum HTTP server (remove or reduce to health endpoint only)
- `crates/web/execute.rs` — subprocess execution via axum (move to Next.js API route)
- `crates/web/docker_stats.rs` — bollard stats broadcaster (move to Next.js API route)
- `crates/web/static/` — static assets (superseded by apps/web build)
- `docker/s6-rc.d/axon-workers/` — no longer starts HTTP server on 49000

### Update

- `apps/web/proxy.ts` — simplified (no forward proxy, direct subprocess spawn)
- `apps/web/app/api/` — add execute, ws/stats endpoints
- `.env.example` — remove `AXON_SHELL_WS_TOKEN`, keep `AXON_WEB_API_TOKEN`
- CLAUDE.md / SWAG config — update port documentation

### Prerequisite

The Next.js API routes must be able to spawn axon subprocesses. This works in the current container setup (the axon binary is available at the same path). The `apps/web/app/api/pulse/chat/route.ts` file already does this for Claude CLI — the same pattern applies.

### Risk

The axum WebSocket provides full duplex, real-time bidirectional communication. SSE is server-push only. If the WebSocket is used for client-to-server messages beyond the initial command (e.g., cancel signals mid-execution), SSE requires a companion POST endpoint for those signals. Evaluate current WebSocket message types before committing to SSE.

Current WebSocket message types from client → server:
- `type: "execute"` — start command execution
- `type: "cancel"` — cancel running command

SSE approach: `POST /api/execute` to start, `POST /api/cancel/{id}` to cancel. Responses stream via SSE.
