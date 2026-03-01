# crates/mcp — Axon MCP Server Guide
Last Modified: 2026-02-25

## Purpose
`crates/mcp` implements the Axon Model Context Protocol server (`axon-mcp`) that exposes crawler/RAG capabilities through a single MCP tool.

- Binary entrypoint: `mcp_main.rs`
- Transport: stdio
- MCP tool: `axon`
- Routing model: consolidated `action` + `subaction`

## Source-of-Truth References
- Wire contract schema doc: `docs/MCP-TOOL-SCHEMA.md`
- MCP runtime/design doc: `docs/MCP.md`
- Tool request/response types: `crates/mcp/schema.rs`
- Tool router and handlers: `crates/mcp/server.rs`
- Env/config loader: `crates/mcp/config.rs`

If documentation and code diverge, update both in the same change.

## Consolidated Tool Pattern
The single `axon` tool is the only public MCP tool. All operations route through:

```json
{
  "action": "<domain>",
  "subaction": "<operation>",
  "...": "operation params"
}
```

Domains (`action`):
- `help`
- `crawl`
- `extract`
- `embed`
- `ingest`
- `query`
- `retrieve`
- `search`
- `map`
- `doctor`
- `domains`
- `sources`
- `stats`
- `artifacts`

This pattern is mandatory. Do not add separate MCP tools for each operation.

## Current Action Map

### `crawl`
- `start`, `status`, `cancel`, `list`, `cleanup`, `clear`, `recover`
- Integration: `crates/jobs/crawl.rs`

### `extract`
- `start`, `status`, `cancel`, `list`, `cleanup`, `clear`, `recover`
- Integration: `crates/jobs/extract.rs`

### `embed`
- `start`, `status`, `cancel`, `list`, `cleanup`, `clear`, `recover`
- Integration: `crates/jobs/embed.rs`

### `ingest`
- `start`, `status`, `cancel`, `list`, `cleanup`, `clear`, `recover`
- Integration: `crates/jobs/ingest.rs`

### `query` / `retrieve`
- Integration: `crates/vector/ops/tei.rs`, `crates/vector/ops/qdrant/*`

### `search` / `map` / `scrape`
- Integration: `crates/core/http.rs`, `crates/core/content.rs`, `crates/crawl/engine.rs`, `spider_agent`

### `doctor` / `domains` / `sources` / `stats`
- Integration: lightweight probes + qdrant endpoints

### `artifacts`
- `head`, `grep`, `wc`, `read`
- Integration: artifact files in `.cache/axon-mcp/`

### `help`
- `run` (implicit direct action)
- Returns all actions/subactions/resources

## Error Contract
Use MCP-native errors:
- invalid request/params -> `ErrorData::invalid_params(...)`
- runtime/system failure -> `ErrorData::internal_error(...)`

Rules:
- Validate required fields early.
- Return deterministic error messages (action/subaction context).
- Never leak secrets in errors.

## Response Contract
Success responses are normalized by `AxonToolResponse`:

```json
{
  "ok": true,
  "action": "...",
  "subaction": "...",
  "data": { ... }
}
```

Keep payloads stable and additive. Avoid breaking field renames.

Default response behavior is artifact-first:
- `response_mode` defaults to `path`
- Large outputs persist in `.cache/axon-mcp/`
- Inline responses are capped and include artifact pointers

## Configuration Model
`load_mcp_config()` in `config.rs` must reuse existing Axon env vars. Do not create a parallel MCP env namespace.

Expected runtime model:
- `axon-mcp` runs inside the same stack environment as workers.
- Existing `.env`/container env should be sufficient.

## Implementation Rules
1. Keep one tool (`axon`) only.
2. Add new capability by extending `action/subaction`, not adding new tool names.
3. Update all three layers together:
   - `schema.rs`
   - `server.rs`
   - `docs/MCP-TOOL-SCHEMA.md`
4. If behavior changes materially, also update `docs/MCP.md` and root docs references.
5. Prefer direct calls into existing Axon job/vector APIs over shelling out.

## Testing Workflow
Build:

```bash
cargo build --bin axon-mcp
```

Schema/introspection:

```bash
mcporter list axon --schema
```

Smoke calls:

```bash
mcporter call axon.axon action:doctor
mcporter call axon.axon action:sources limit:5
mcporter call axon.axon action:crawl subaction:list limit:5
```

When adding a new subaction, add at least one smoke example here.

## Change Checklist (Mandatory)
- [ ] `schema.rs` updated
- [ ] `server.rs` routing/handler updated
- [ ] docs contract updated (`docs/MCP-TOOL-SCHEMA.md`)
- [ ] `cargo check --bin axon-mcp` passes
- [ ] `cargo check --bin axon` still passes

This is the way.
