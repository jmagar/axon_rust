# apps/web — Axon Next.js UI
Last Modified: 2026-03-03

Next.js 16 App Router frontend for the Axon RAG system. Runs on port `49010` in Docker via the `axon-web` service.

## Commands

```bash
pnpm dev          # Dev server (Turbopack) — hot reload
pnpm build        # Production build (output: standalone)
pnpm test         # Vitest (node environment, __tests__/**/*.test.{ts,tsx})
pnpm lint         # Biome check (lint + format check)
pnpm format       # Biome format --write (auto-fix)
```

## Architecture

```
app/layout.tsx           → Providers → AppShell → {children}
app/providers.tsx        → AxonWsContext + split WsMessages contexts + TooltipProvider
components/app-shell.tsx → PulseSidebar + CmdKPalette
app/page.tsx             → DashboardPage (Omnibox + ResultsPanel + NeuralCanvas)
```

App-level navigation buttons (`/mcp`, `/agents`, `/settings`) are hosted in `AppShell`, not in individual pages.

### Pages

| Route | Component | Purpose |
|-------|-----------|---------|
| `/` | `DashboardPage` | Omnibox + results + Pulse workspace |
| `/editor` | `EditorPage` | Pulse AI editor (Plate.js) |
| `/mcp` | `McpPage` | MCP server management |
| `/agents` | `AgentsPage` | Available Claude agents |
| `/jobs` | `JobsPage` | Async job dashboard |
| `/terminal` | `TerminalPage` | xterm.js shell via node-pty |
| `/cortex` | cortex layout | RAG dashboards (stats/domains/sources/doctor/status) |
| `/settings` | `SettingsPage` | App configuration |

### API Routes

| Route | Purpose |
|-------|---------|
| `/api/pulse/chat` | Stream Claude CLI subprocess output (NDJSON) |
| `/api/pulse/source` | Fetch and sanitize remote source text (SSRF-guarded) |
| `/api/pulse/save` | Create/update Pulse docs (`.cache/pulse/*.md`) |
| `/api/pulse/doc` | Load a Pulse doc by filename |
| `/api/cortex/stats` | Qdrant + Postgres metrics |
| `/api/cortex/domains` | Indexed domain list |
| `/api/cortex/sources` | Indexed URL list |
| `/api/cortex/doctor` | Service health check |
| `/api/cortex/status` | Job queue status |
| `/api/mcp` | MCP server config (read/write) |
| `/api/ai/command` | Plate.js AI editor commands (edit/generate/comment) |
| `/api/ai/copilot` | Plate.js ghost-text copilot |
| `/api/jobs` | Job list + job detail (strict validated filters) |

### WebSocket Proxy (next.config.ts)

```
/ws         → AXON_BACKEND_URL/ws         (Rust axon-workers, port 49000)
/ws/shell   → 127.0.0.1:SHELL_SERVER_PORT (authenticated node-pty shell, port 49011)
/download/* → AXON_BACKEND_URL/download/*
/output/*   → AXON_BACKEND_URL/output/*
```

`next.config.ts` also applies global security headers: CSP, `X-Frame-Options`, `Referrer-Policy`, `X-Content-Type-Options`, and HSTS outside development.
It also sets cache headers for `/api/cortex/*`: `s-maxage=30, stale-while-revalidate=60`.

WS client: `hooks/use-axon-ws.ts` — exponential backoff reconnect (1s → 30s), pending message queue. Reconnects on `online`, `pageshow`, and `visibilitychange`.

WS protocol types: `lib/ws-protocol.ts` — all message shapes for client↔server. **Modes must match `ALLOWED_MODES` in `crates/web/execute.rs`.**

### Pulse Chat

`/api/pulse/chat/route.ts` spawns `claude` CLI as subprocess via `child_process.spawn`.

- Args built in `app/api/pulse/chat/claude-stream-types.ts:buildClaudeArgs()`
- Output: NDJSON stream, parsed by `stream-parser.ts`
- MCP config: `/home/node/.claude/mcp.json` (`--strict-mcp-config` — ignores `~/.claude.json`)
- Timeout: 300s (`CLAUDE_TIMEOUT_MS`)
- Context budget: 800k chars (~200k tokens)
- `--dangerously-skip-permissions` on by default (no TTY in container); disable with `PULSE_SKIP_PERMISSIONS=false`

### API Contracts

`/api/jobs` query filters are validated against strict allowlists:
- `type`: `crawl | extract | embed | github | reddit | youtube`
- `status`: `pending | running | completed | failed | canceled`
- `status=failed` includes both failed and canceled jobs
- invalid filters return `400` with structured error body

`/api/pulse/source` blocks private/loopback/local-network SSRF targets and returns `code: "ssrf_blocked"` on blocked URLs.

### API Error Format

Server routes use a shared JSON error envelope:

```json
{
  "error": "Message",
  "code": "optional_machine_code",
  "errorId": "optional_debug_id",
  "detail": {}
}
```

### Pulse File Storage

Docs stored in `.cache/pulse/*.md` (resolved from workspace root via `lib/pulse/workspace-root.ts`).

Format: YAML frontmatter + markdown body.
```
---
title: "My Doc"
createdAt: "..."
updatedAt: "..."
tags: []
collections: ["cortex"]
---

# Content here
```

`lib/pulse/storage.ts` — `savePulseDoc`, `updatePulseDoc` (update-in-place), `loadPulseDoc`, `listPulseDocs`.

**Autosave pattern** (`hooks/use-pulse-autosave.ts`): `filenameRef` + `docMetaRef` pattern — refs track filenames and cached `{createdAt,updatedAt,tags,collections}` to avoid stale closure bugs.

### Editor (Plate.js)

`components/editor/` — Plate.js v52 rich text editor with AI features.

- `use-chat.ts` — live AI streaming via `/api/ai/command`
- `use-chat-fake-stream.ts` — local dev fake stream for testing UI without LLM
- AI menu (`components/ui/ai-menu.tsx`) — floating AI commands on selection
- Requires `OPENAI_BASE_URL` + `OPENAI_API_KEY` + `OPENAI_MODEL` to be set

### NeuralCanvas

Bioluminescent animated background (`components/neural-canvas/`). Canvas intensity is driven by:
- Docker container CPU stats (via WS `stats` messages)
- Command execution state (full intensity while processing)
- Command completion pulse (0.15 for 3s, then back to 0)

Profile stored in `localStorage` key `axon.web.neural-canvas.profile`. Options: `current`, `subtle`, `cinematic`, `electric`.

## Environment Variables

```bash
# Backend URL (where Rust axon-workers serve HTTP/WS)
AXON_BACKEND_URL=http://localhost:49000     # default: http://localhost:3939

# Override WS URL for the client (optional — defaults to /ws path)
NEXT_PUBLIC_AXON_WS_URL=

# Shell WebSocket port (node-pty)
SHELL_SERVER_PORT=49011                    # default

# API auth token required by middleware.ts (unless insecure dev bypass is enabled)
AXON_WEB_API_TOKEN=CHANGE_ME

# Comma-separated allowed origins for /api and /ws/shell (optional)
AXON_WEB_ALLOWED_ORIGINS=

# Development-only localhost bypass for auth gates (do not enable in production)
AXON_WEB_ALLOW_INSECURE_DEV=false

# Optional shell-specific token/origin overrides
AXON_SHELL_WS_TOKEN=
AXON_SHELL_ALLOWED_ORIGINS=

# Optional client-side tokens used by shell websocket URL wiring
NEXT_PUBLIC_AXON_API_TOKEN=
NEXT_PUBLIC_SHELL_WS_TOKEN=

# Optional allowlist for Pulse chat `--betas` values.
# Defaults to: interleaved-thinking
AXON_ALLOWED_CLAUDE_BETAS=interleaved-thinking

# Qdrant collection (used in Pulse doc defaults)
AXON_COLLECTION=cortex                    # default

# Claude CLI permissions (disable for interactive dev)
PULSE_SKIP_PERMISSIONS=true               # default

# Plate.js AI (required for editor AI features)
OPENAI_BASE_URL=http://YOUR_LLM_HOST/v1
OPENAI_API_KEY=your-key
OPENAI_MODEL=your-model-name
```

## Code Style

Biome 2.4.4 — `biome.json` at root:
- **Single quotes**, **no semicolons**
- **2-space indent**, **100 char line width**
- ESM only, named exports, no default exports
- `@` alias resolves to project root (`apps/web/`)

## Testing

```bash
pnpm test              # run all tests
pnpm test -- --watch   # watch mode
pnpm test -- <pattern> # filter by filename
```

- Vitest 4, `node` environment (not jsdom — most tests are logic/API tests)
- Test files: `__tests__/**/*.test.{ts,tsx}`
- Path alias: `@` → project root (same as build)
- No snapshot tests for UI components (use `omnibox-snapshot.test.tsx` as reference)

## Gotchas

### Claude CLI User Ownership
Claude CLI runs as `node` (UID 1000) via `s6-setuidgid`. Bind-mount dirs under `${AXON_DATA_DIR}/axon/claude` must be owned by `node`, not `root`. Fix:
```bash
# On host:
sudo chown -R jmagar:jmagar /path/to/appdata/axon/claude/
```
Container fix: `docker/web/cont-init.d/15-fix-claude-dir-ownership` runs `chown -R node:node /home/node/.claude` on every start.

### MCP Config Path
Claude CLI is given `--mcp-config /home/node/.claude/mcp.json --strict-mcp-config`. It ignores `~/.claude.json`. To add MCP servers, edit the project-owned `mcp.json`, not the user-level config.

### Always Dark Mode
`app/layout.tsx` hardcodes `<html className="dark">`. Do not add theme toggling without updating this.

### Turbopack
`next.config.ts` sets `turbopack: { root: __dirname }`. Do NOT use webpack plugins that lack Turbopack equivalents in dev.

### Platejs Packages Require Transpile
Next.js standalone build requires `transpilePackages` for all `@platejs/*` and `platejs` packages (already set in `next.config.ts`). Adding new Plate plugins: add to `transpilePackages`.

### pnpm Auto-Sync in Container
The `axon-web` container runs a `pnpm-watcher` s6 service that polls `pnpm-lock.yaml` every 3s and reinstalls if changed. `pnpm add <pkg>` on the host takes effect in the container within ~3s — no rebuild needed. The `node_modules` anonymous volume is root-owned; the watcher runs as root.

### Shell Server
`shell-server.mjs` is the node-pty WebSocket bridge. Runs on `SHELL_SERVER_PORT` (default 49011). It is started separately from Next.js — not part of `pnpm dev`.

- Auth required via bearer/x-api-key/query token (`AXON_SHELL_WS_TOKEN` or fallback `AXON_WEB_API_TOKEN`)
- Origin validation enforced (`AXON_SHELL_ALLOWED_ORIGINS` / `AXON_WEB_ALLOWED_ORIGINS` / same-host fallback)
- PTY child env is allowlisted and no longer inherits full `process.env`

### WS Modes Must Match Rust Allow-List
`lib/ws-protocol.ts` defines `MODES`. Any mode added here must also be added to `ALLOWED_MODES` in `crates/web/execute.rs`, or the backend will reject the request.

### Pulse Autosave — Phantom Re-Save Guard
`use-pulse-autosave.ts`: `docMetaRef` is only reset when `incoming !== filenameRef.current`. Do NOT reset it on every render or prop change — this causes ghost re-saves on first-save filename sync.

### Qdrant Pre-Delete Race (Pulse Source Updates)
When updating a Pulse doc's Qdrant embedding, always use `?wait=true` on the delete endpoint before upsert. Without it, the upsert can race the delete and stale vectors accumulate.
