# Axon MCP Server Guide
Last Modified: 2026-03-03

## Purpose
`axon mcp` exposes Axon through one MCP tool named `axon`.

- Transport: RMCP streamable HTTP (`/mcp`, stateful sessions)
- Tool count: 1
- Tool name: `axon`
- Routing fields: `action` + `subaction` for lifecycle families
- Response behavior field: `response_mode` (`path|inline|both`, default `path`)

Canonical schema and action contract:
- `docs/MCP-TOOL-SCHEMA.md`

Implementation:
- `crates/mcp/schema.rs`
- `crates/mcp/server.rs`
- `crates/mcp/config.rs`

## Runtime Model
`axon mcp` is expected to run in the same environment as Axon workers.

Core stack env vars are reused:
- `AXON_PG_URL`
- `AXON_REDIS_URL`
- `AXON_AMQP_URL`
- `QDRANT_URL`
- `TEI_URL`
- `OPENAI_BASE_URL`
- `OPENAI_API_KEY`
- `OPENAI_MODEL`
- `TAVILY_API_KEY`

MCP transport env vars:
- `AXON_MCP_HTTP_HOST` (default `0.0.0.0`)
- `AXON_MCP_HTTP_PORT` (default `8001`)

OAuth broker env vars (required for protected `/mcp` access):
- `GOOGLE_OAUTH_CLIENT_ID`
- `GOOGLE_OAUTH_CLIENT_SECRET`

Optional OAuth overrides:
- `GOOGLE_OAUTH_AUTH_URL`
- `GOOGLE_OAUTH_TOKEN_URL`
- `GOOGLE_OAUTH_REDIRECT_PATH`
- `GOOGLE_OAUTH_REDIRECT_HOST`
- `GOOGLE_OAUTH_REDIRECT_URI`
- `GOOGLE_OAUTH_BROKER_ISSUER`
- `GOOGLE_OAUTH_SCOPES`
- `GOOGLE_OAUTH_DCR_TOKEN`
- `GOOGLE_OAUTH_REDIRECT_POLICY`
- `GOOGLE_OAUTH_REDIS_URL` (falls back to `AXON_REDIS_URL`)
- `GOOGLE_OAUTH_REDIS_PREFIX`

`GOOGLE_OAUTH_REDIRECT_POLICY` modes:
- `loopback_or_https` (default): allow loopback HTTP callbacks (`localhost`, `127.0.0.1`, `::1`) and any HTTPS callback
- `loopback_only`: allow only loopback HTTP callbacks
- `any`: allow any HTTP/HTTPS callback URI

If OAuth is not configured, requests to `/mcp` return unauthorized.

## Transport Notes
`axon mcp` starts the HTTP server (`run_http_server`) and serves `/mcp`.

There is also an internal stdio server implementation (`run_stdio_server`) in code, but it is not what the `axon mcp` command launches.

## OAuth Endpoints and Flow
Implemented endpoints:
- `GET /oauth/google/status`
- `GET /oauth/google/login`
- `GET /oauth/google/callback`
- `GET /oauth/google/token`
- `GET|POST /oauth/google/logout`
- `GET /.well-known/oauth-protected-resource`
- `GET /.well-known/oauth-authorization-server`
- `POST /oauth/register`
- `GET /oauth/authorize`
- `POST /oauth/token`

High-level flow:
1. Client discovers metadata from the `/.well-known/*` endpoints.
2. Client registers (`/oauth/register`) if needed.
3. User authenticates via Google (`/oauth/google/login` -> Google -> `/oauth/google/callback`).
4. Authorization code flow completes via `/oauth/authorize` and `/oauth/token`.
5. Client calls `/mcp` with bearer token.

## Token Persistence
OAuth state is persisted in Redis when available; otherwise in-memory fallback is used.

Stored record types:
- pending login state
- browser session tokens
- dynamic clients
- auth codes
- access tokens
- refresh tokens
- rate-limit buckets

Cookie:
- `__Host-axon_oauth_session`

TTL semantics (current behavior):
- OAuth session: 7 days
- Refresh tokens: 30 days
- Auth code: 10 minutes
- Pending login state: 15 minutes
- Access token: per-issued token expiry

## Request Pattern
Primary pattern:

```json
{
  "action": "<operation>",
  "...": "operation fields"
}
```

Lifecycle pattern when needed:

```json
{
  "action": "ingest|extract|embed|crawl|refresh",
  "subaction": "start|status|cancel|list|cleanup|clear|recover|schedule",
  "...": "subaction fields"
}
```

## Preferred Action Names (Top-Level)
Use CLI-identical action names:
- `ingest`, `extract`, `embed`, `crawl`, `refresh`
- `query`, `retrieve`
- `doctor`, `domains`, `sources`, `stats`
- `search`, `map`
- `artifacts` (with subactions `head|grep|wc|read`)
- `scrape`, `research`, `ask`, `screenshot`, `help`, `status`

Examples:
- `action: "ingest", subaction: "start"`
- `action: "extract", subaction: "list"`
- `action: "query"`
- `action: "doctor"`

## Parser Rules
The server uses strict deserialization:
- `action` is required and must match canonical schema names exactly
- `subaction` is required for lifecycle families (`crawl|extract|embed|ingest|refresh|artifacts`)
- No fallback fields (`command|op|operation`)
- No action alias remapping
- No token normalization (`-`/spaces/case are not rewritten)

## Online Operations
Direct actions:
- `help`
- `scrape`
- `research`
- `ask`
- `screenshot`

Lifecycle families:
- `crawl`: `start|status|cancel|list|cleanup|clear|recover`
- `extract`: `start|status|cancel|list|cleanup|clear|recover`
- `embed`: `start|status|cancel|list|cleanup|clear|recover`
- `ingest`: `start|status|cancel|list|cleanup|clear|recover`
- `refresh`: `start|status|cancel|list|cleanup|clear|recover|schedule`

Refresh schedule subactions:
- `list`
- `create`
- `delete`
- `enable`
- `disable`

No top-level aliases are supported.

## Response Pattern
Success responses are normalized:

```json
{
  "ok": true,
  "action": "...",
  "subaction": "...",
  "data": { "...": "..." }
}
```

## mcporter Smoke Tests
```bash
# Primary MCP smoke path (includes resource checks via help + schema)
./scripts/test-mcp-tools-mcporter.sh

# Optional expanded run (network-heavy/side-effect actions)
./scripts/test-mcp-tools-mcporter.sh --full

# Individual calls
mcporter list axon --schema
mcporter call axon.axon action:help
mcporter call axon.axon action:doctor
mcporter call axon.axon action:scrape url:https://example.com
mcporter call axon.axon action:query query:'rust mcp sdk'
mcporter call axon.axon action:ingest subaction:start source_type:github target:owner/repo
mcporter call axon.axon action:crawl subaction:list limit:5 offset:0
mcporter call axon.axon action:refresh subaction:list limit:5 offset:0
mcporter call axon.axon action:refresh subaction:schedule schedule_subaction:list
mcporter call axon.axon action:artifacts subaction:head path:.cache/axon-mcp/help-actions.json limit:20
```
