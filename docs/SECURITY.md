# Security Model
Last Modified: 2026-03-04

Version: 1.0.0
Last Updated: 01:26:53 | 02/25/2026 EST

## Table of Contents

1. Scope
2. Threat Model
3. Security Controls
4. Secrets Management
5. Network Exposure
6. API and Command Surface Hardening
7. Residual Risks
8. Operational Security Checklist
9. Source Map

## Scope

This document captures security controls present in code and deployment configuration for Axon.

## Threat Model

In scope:

- SSRF attempts through user-provided URLs
- Path traversal attempts in file/download APIs
- Command injection attempts through websocket `execute`
- Secret leakage through repository commits and logs
- Local service exposure beyond host boundary

Out of scope:

- Host kernel compromise
- Supply-chain integrity beyond pinned images/dependencies
- Full multi-tenant isolation (system is designed for trusted self-hosted operation)

## Security Controls

### URL Validation and SSRF Controls

Implemented in `crates/core/http.rs`:

- scheme allowlist: `http` and `https` only
- blocked hosts: `localhost`, `.localhost`, `.internal`, `.local`
- blocked IP ranges:
  - loopback
  - link-local
  - RFC-1918 private ranges
  - IPv4-mapped IPv6 private/loopback
  - IPv6 unique-local
- additional crawler blacklist patterns for defense-in-depth

### File Path Safety

`/output/{*path}` (`crates/web.rs`):

- rejects `..` and NUL bytes
- canonicalizes base and target
- enforces target path under output root

`/download/{job_id}/...` (`crates/web/download.rs`):

- validates job id format
- resolves files only from registered job output dirs
- path traversal guarded
- max files limit enforced via `AXON_DOWNLOAD_MAX_FILES`

`/api/omnibox/files` (`apps/web/app/api/omnibox/files/route.ts`):

- id parsing with `source:path` model
- rejects unsafe ids and `..`
- resolves within source root only

### WebSocket Authentication Gate

`/ws` upgrade path (`crates/web.rs`):

- Gate is active when `AXON_WEB_API_TOKEN` is set; disabled (open) when unset
- One token type: `AXON_WEB_API_TOKEN` — the same static secret used by `proxy.ts` for `/api/*` routes
- Browser sends it as `?token=` query param (appended by `hooks/use-axon-ws.ts`)
- MCP OAuth clients (`atk_` tokens) do not have access to `/ws` — they use the MCP tool API instead
- Non-loopback connections to `/ws/shell` rejected with 403; IPv4-mapped loopback (`::ffff:127.0.0.1`) accepted correctly
- Rejected upgrades return 401 before the WebSocket handshake completes

### Command Surface Hardening

WebSocket command execution (`crates/web/execute.rs`):

- explicit `ALLOWED_MODES` list
- explicit `ALLOWED_FLAGS` list
- blocked forwarding of sensitive infra flags (db/redis/amqp/openai/qdrant/tei URL flags)
- asynchronous mode semantics controlled server-side

## Secrets Management

Required practice:

- secrets in `.env` only
- `.env` is gitignored
- `.env.example` is the tracked template

Do not:

- commit real credentials
- print API keys in logs
- hardcode endpoint credentials in source

## Network Exposure

Most infra services bind to loopback (`127.0.0.1`) in compose:

- Postgres
- Redis
- RabbitMQ
- Qdrant
- Chrome management/CDP endpoints

`axon-web` is published as `49010:49010` by default (host-accessible unless firewall/reverse-proxy constrained).

Hardening guidance:
- For local-only web UI, publish `127.0.0.1:49010:49010`.
- If exposed externally, enforce TLS + auth at reverse proxy.
- Keep worker/internal service ports loopback-bound unless explicitly required.

## API and Command Surface Hardening

Pulse/Copilot API routes:

- schema validation with Zod
- explicit error mapping (for example `400`, `401`, `408`, `500`)
- timeout on upstream LLM calls

Worker startup:

- validates required env vars for DB/Redis/AMQP before long-running lane execution

## Residual Risks

1. DNS rebinding TOCTOU window:
- URL is validated before request, but resolver behavior at connect time can still change.
- Mitigation options: resolver pinning or additional network egress controls.

2. WebSocket auth requires explicit env config:
- Gate is disabled if `AXON_WEB_API_TOKEN` is not set — any client can connect to `/ws`.
- For production / externally-exposed deployments, always set `AXON_WEB_API_TOKEN` to activate the gate.

3. Upstream model endpoints:
- security posture depends on TEI/LLM deployment hardening outside this repo.

## Operational Security Checklist

Before deploy:

1. Confirm `.env` exists and is not tracked.
2. Confirm no secrets in changed files:

```bash
git diff -- . ':!*.lock'
```

Note: `git diff` only checks current uncommitted/staged changes. It does not detect
secrets already present in commit history.

For history scans, run a dedicated secret scanner on recent commits, for example:

```bash
gitleaks detect --source=. --log-opts="HEAD~5..HEAD"
```

3. Validate local-only bindings in compose.
4. Run `./scripts/axon doctor`.

After deploy:

1. Confirm healthy containers.
2. Check logs for repeated auth/network failures.
3. Ensure API routes return expected status codes on invalid requests.

## Source Map

- `crates/core/http.rs` — SSRF / URL validation
- `crates/web.rs` — WS OAuth gate, shell WS loopback restriction, output file path safety
- `crates/web/download.rs` — download path safety
- `crates/web/execute.rs` — ALLOWED_MODES / ALLOWED_FLAGS command surface
- `crates/web/execute/cancel.rs` — cancel mode guard (H-04)
- `crates/mcp/server/oauth_google/` — MCP OAuth server (issues `atk_` tokens; separate from WS auth)
- `apps/web/hooks/use-axon-ws.ts` — WS URL construction with `?token=` passthrough
- `apps/web/proxy.ts` — `/api/*` origin check + API token validation helpers
- `apps/web/app/api/omnibox/files/route.ts`
- `apps/web/app/api/pulse/chat/route.ts`
- `apps/web/app/api/ai/copilot/route.ts`
- `docker-compose.yaml`
- `.gitignore`
