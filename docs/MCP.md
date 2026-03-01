# Axon MCP Server Guide
Last Modified: 2026-02-25

## Purpose
`axon-mcp` exposes Axon through one MCP tool named `axon`.

- Transport: stdio
- Tool count: 1
- Tool name: `axon`
- Routing fields: `action` + `subaction` for lifecycle families
- Response behavior field: `response_mode` (`path|inline|both`, default `path`)

Canonical schema and action contract:
- `docs/MCP-TOOL-SCHEMA.md`

Implementation:
- `mcp_main.rs`
- `crates/mcp/schema.rs`
- `crates/mcp/server.rs`
- `crates/mcp/config.rs`

## Runtime Model
`axon-mcp` is expected to run in the same environment as Axon workers.

It reuses existing stack env vars (no MCP-only env namespace):
- `AXON_PG_URL`
- `AXON_REDIS_URL`
- `AXON_AMQP_URL`
- `QDRANT_URL`
- `TEI_URL`
- `OPENAI_BASE_URL`
- `OPENAI_API_KEY`
- `OPENAI_MODEL`
- `TAVILY_API_KEY`

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
  "action": "ingest|extract|embed|crawl",
  "subaction": "start|status|cancel|list|cleanup|clear|recover",
  "...": "subaction fields"
}
```

## Preferred Action Names (Top-Level)
Use CLI-identical action names:
- `ingest`, `extract`, `embed`, `crawl`
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
- `subaction` is required for lifecycle families (`crawl|extract|embed|ingest|artifacts`)
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
# Comprehensive script (includes resource checks via help + schema)
./scripts/test-mcp-tools-mcporter.sh

# Optional expanded run (network-heavy/side-effect actions)
./scripts/test-mcp-tools-mcporter.sh --full

# Individual calls
mcporter list axon --schema
mcporter call axon.axon action:help
mcporter call axon.axon action:doctor
mcporter call axon.axon action:scrape url:https://example.com
mcporter call axon.axon action:query query:'rust mcp sdk'
mcporter call axon.axon action:ingest source_type:github target:owner/repo
mcporter call axon.axon action:crawl subaction:list limit:5 offset:0
mcporter call axon.axon action:artifacts subaction:head path:.cache/axon-mcp/help-actions.json limit:20
```
