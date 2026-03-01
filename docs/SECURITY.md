# Security Model
Last Modified: 2026-02-25

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

### Command Surface Hardening

WebSocket command execution (`crates/web/execute/mod.rs`):

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

Compose ports bind to loopback (`127.0.0.1`) by default:

- Postgres
- Redis
- RabbitMQ
- Qdrant
- Chrome management/CDP endpoints

This limits exposure to local host unless additional reverse proxy/network routing is configured.

## API and Command Surface Hardening

Pulse/Copilot API routes:

- schema validation with Zod
- explicit error mapping (`400`, `502`, `503`, `500`)
- timeout on upstream LLM calls

Worker startup:

- validates required env vars for DB/Redis/AMQP before long-running lane execution

## Residual Risks

1. DNS rebinding TOCTOU window:
- URL is validated before request, but resolver behavior at connect time can still change.
- Mitigation options: resolver pinning or additional network egress controls.

2. No built-in auth for local websocket/UI surfaces:
- intended for trusted local/homelab network.
- if exposed externally, add authn/authz at edge proxy.

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

- `crates/core/http.rs`
- `crates/web.rs`
- `crates/web/download.rs`
- `crates/web/execute/mod.rs`
- `apps/web/app/api/omnibox/files/route.ts`
- `apps/web/app/api/pulse/chat/route.ts`
- `apps/web/app/api/ai/copilot/route.ts`
- `docker-compose.yaml`
- `.gitignore`
