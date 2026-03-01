# Axon MCP Tool Schema (Source of Truth)
Last Modified: 2026-02-25

## Contract
- MCP server binary: `axon-mcp`
- Tool count: `1`
- Tool name: `axon`
- Primary route field: `action`
- Canonical route form: `action` + optional `subaction`
- Response control field: `response_mode` (`path|inline|both`, default `path`)

Code references:
- `/home/jmagar/workspace/axon_rust/crates/mcp/schema.rs`
- `/home/jmagar/workspace/axon_rust/crates/mcp/server.rs`

## Canonical Success Envelope
```json
{
  "ok": true,
  "action": "<resolved action>",
  "subaction": "<resolved subaction>",
  "data": { "...": "..." }
}
```

## Parser Rules
Incoming request map is parsed strictly with serde:

- `action` is required and must match canonical schema names
- `subaction` is required for lifecycle families (`crawl|extract|embed|ingest|artifacts`)
- No fallback fields (`command`, `op`, `operation`)
- No token normalization or case folding
- No action alias remapping

## Preferred Client Actions
Use CLI-identical top-level actions:
- `ingest`, `extract`, `embed`, `crawl`, `refresh`
- `query`, `retrieve`
- `doctor`, `domains`, `sources`, `stats`
- `search`, `map`, `scrape`, `research`, `ask`, `screenshot`, `help`, `status`

For lifecycle management (`status|cancel|list|cleanup|clear|recover`), use canonical families with `subaction`. `refresh` also supports `schedule` subaction with `schedule_subaction` param (`list`, `create`, `delete`, `enable`, `disable`):

```json
{ "action": "ingest", "subaction": "status", "job_id": "..." }
```

## Response Policy (Context-Safe Defaults)
- Default is artifact-first (`response_mode=path`).
- Heavy operations write result artifacts to `.cache/axon-mcp/`.
- Tool response returns compact metadata only by default:
  - `path`, `bytes`, `line_count`, `sha256`, `preview`, `preview_truncated`
- Inline modes are capped/truncated and always include artifact pointers.

## Direct Actions
These actions do not require `subaction`:
- `help`
- `scrape`
- `research`
- `ask`
- `screenshot`

## Crawl Start Parameters
Optional fields accepted on `{ "action": "crawl", "subaction": "start", ... }`:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `url` | string | — | Seed URL (required) |
| `max_pages` | u32 | 0 (uncapped) | Page limit |
| `max_depth` | usize | 5 | Max crawl depth |
| `include_subdomains` | bool | true | Include subdomains |
| `respect_robots` | bool | false | Honour robots.txt |
| `discover_sitemaps` | bool | true | Run sitemap backfill after crawl |
| `sitemap_since_days` | u32 | 0 | Only backfill sitemap URLs with `<lastmod>` within last N days (0 = no filter) |
| `render_mode` | enum | `auto_switch` | `http`, `chrome`, `auto_switch` |
| `delay_ms` | u64 | 0 | Per-request delay ms |

## Lifecycle Action Families
- `crawl`: `start|status|cancel|list|cleanup|clear|recover`
- `extract`: `start|status|cancel|list|cleanup|clear|recover`
- `embed`: `start|status|cancel|list|cleanup|clear|recover`
- `ingest`: `start|status|cancel|list|cleanup|clear|recover`
- `refresh`: `start|status|cancel|list|cleanup|clear|recover|schedule`
- `query`: `query`
- `retrieve`: `retrieve`
- `search`: `search`
- `map`: `map`
- `scrape`: `scrape`
- `doctor`: `doctor`
- `domains`: `domains`
- `sources`: `sources`
- `stats`: `stats`
- `artifacts`: `head|grep|wc|read`

`artifacts` fields:
- `path` (required)
- `pattern` (required for `grep`)
- `limit` and `offset` for paginated inspection

## Pagination Defaults
List/search style endpoints default to low limits and accept `limit` + `offset`.

## MCP Resources
Implemented resource(s):
- `axon://schema/mcp-tool`

## Runtime Dependencies
No MCP-specific env namespace. Server reads existing Axon stack vars:
- `AXON_PG_URL`, `AXON_REDIS_URL`, `AXON_AMQP_URL`
- `QDRANT_URL`, `TEI_URL`
- `OPENAI_BASE_URL`, `OPENAI_API_KEY`, `OPENAI_MODEL`
- `TAVILY_API_KEY`

## Error Semantics
- Input or shape failures -> MCP `invalid_params`
- Runtime failures -> MCP `internal_error`
