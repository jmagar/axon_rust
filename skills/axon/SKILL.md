---
name: axon
description: >-
  Axon is a self-hosted RAG engine that crawls, scrapes, extracts, embeds, and queries
  web content — building a searchable knowledge base that grows with every operation.
  This skill MUST be invoked before ANY call to the `axon` MCP tool. No exceptions.
  This skill MUST be used when the user asks to "conduct research", "crawl a site",
  "scrape a URL", "search the web", "embed content", "query the knowledge base",
  "ask a question", "ingest a GitHub repo", "check job status", or "run axon doctor".
---

# Axon MCP Skill

Axon handles all web operations — crawl, scrape, extract, embed, ingest, query, ask — through
a single MCP tool named `axon`. Every operation auto-indexes results into the Qdrant vector
store for future RAG queries.

## First Action: `help`

Before doing anything else, call `{ "action": "help" }`. This is the self-describing discovery
endpoint. It returns the complete action map (every action and its valid subactions), available
MCP resources, and server defaults (`response_mode`, `artifact_dir`). Use it to bootstrap — the
response is the live contract, always in sync with the running server.

```json
{ "action": "help" }
```

Response shape:
```json
{
  "ok": true,
  "action": "help",
  "data": {
    "tool": "axon",
    "actions": { "crawl": ["start","status","cancel",...], "query": ["query"], ... },
    "resources": ["axon://schema/mcp-tool"],
    "defaults": { "response_mode": "path", "artifact_dir": ".cache/axon-mcp" }
  }
}
```

When unsure which actions or subactions exist, call `help` — don't guess.

## Key Contract

- **Tool name**: `axon` (single tool, action-routed)
- **Transport**: stdio
- **Routing**: `action` (required) + `subaction` (lifecycle families only)
- **Response default**: `response_mode=path` — artifacts written to `.cache/axon-mcp/`, compact metadata returned
- **Parser**: Strict serde — no fallback fields, no alias remapping, no case folding

## Load Order

1. Call `{ "action": "help" }` — live contract from the running server (always authoritative).
2. Consult [`references/routing-cheatsheet.md`](references/routing-cheatsheet.md) — quick-reference for action routing.
3. If inside the `axon_rust` repo, read `docs/MCP-TOOL-SCHEMA.md` and `docs/MCP.md` for implementation details.

## Action Catalog

### Direct Actions (no subaction needed)

| Action | Required Fields | Purpose |
|--------|----------------|---------|
| `scrape` | `url` | Scrape URL to markdown, auto-embed |
| `search` | `query` | Web search via Tavily, auto-queue crawl |
| `research` | `query` | AI-synthesized research via Tavily |
| `ask` | `query` | RAG: vector search + LLM answer |
| `query` | `query` | Semantic vector search |
| `retrieve` | `url` | Fetch stored chunks by URL |
| `map` | `url` | Discover URLs without scraping |
| `screenshot` | `url` | Capture page screenshot |
| `help` | — | List available actions |
| `doctor` | — | Service connectivity check |
| `status` | — | Async job queue status |
| `domains` | — | Indexed domains + stats |
| `sources` | — | Indexed URLs + chunk counts |
| `stats` | — | Qdrant collection stats |

### Lifecycle Families (require subaction)

Families: `crawl`, `extract`, `embed`, `ingest`

Subactions: `start | status | cancel | list | cleanup | clear | recover`

| Family + subaction | Required Fields |
|--------------------|----------------|
| `crawl` + `start` | `urls` (non-empty array) |
| `extract` + `start` | `urls` (non-empty array) |
| `embed` + `start` | `input` |
| `ingest` + `start` | `source_type` + `target` (see below) |
| Any family + `status`/`cancel` | `job_id` |
| Any family + `list` | — (optional `limit`, `offset`) |
| Any family + `cleanup`/`clear`/`recover` | — |

Default: omitting `subaction` on lifecycle families resolves to `start`.

#### Ingest Source Types

| `source_type` | `target` format | Example |
|---------------|----------------|---------|
| `github` | `owner/repo` | `"target": "anthropics/claude-code"` |
| `reddit` | subreddit name or thread URL | `"target": "rust"` or full URL |
| `youtube` | video URL | `"target": "https://youtube.com/watch?v=..."` |
| `sessions` | directory path | `"target": "./exports"` |

### Artifact Inspection (subaction of `artifacts`)

| Subaction | Required Fields |
|-----------|----------------|
| `head` | `path` |
| `read` | `path` |
| `wc` | `path` |
| `grep` | `path`, `pattern` |

Optional: `limit`, `offset` for paginated inspection.

## Request Templates

```json
{ "action": "scrape", "url": "https://example.com" }
{ "action": "query", "query": "how does mcp routing work?", "limit": 10 }
{ "action": "ask", "query": "what is the axon architecture?" }
{ "action": "search", "query": "rust async patterns 2026" }
{ "action": "crawl", "urls": ["https://docs.example.com"] }
{ "action": "ingest", "source_type": "github", "target": "owner/repo" }
{ "action": "extract", "urls": ["https://example.com/pricing"] }
{ "action": "embed", "input": "https://example.com" }
{ "action": "crawl", "subaction": "status", "job_id": "<uuid>" }
{ "action": "crawl", "subaction": "list", "limit": 5 }
{ "action": "doctor" }
{ "action": "status" }
{ "action": "artifacts", "subaction": "head", "path": ".cache/axon-mcp/help-actions.json", "limit": 20 }
```

## Response Handling

All responses follow this envelope:

```json
{ "ok": true, "action": "<resolved>", "subaction": "<resolved>", "data": { } }
```

- `response_mode=path` (default): heavy output written to `.cache/axon-mcp/`, metadata returned (path, bytes, line_count, sha256, preview)
- `response_mode=inline`: full content inline (capped/truncated)
- `response_mode=both`: artifact + inline

Prefer `path` mode to keep context lean. Inspect artifacts with `head`/`read`/`grep` subactions.

## Error Handling

Failed responses return `ok: false` with an MCP error code:

| Error | Meaning | Action |
|-------|---------|--------|
| `invalid_params` | Request shape is wrong — bad action, missing field, wrong type | Fix the payload. Do not retry the same request. |
| `internal_error` | Server-side failure — service down, timeout, unexpected crash | Check `doctor` for service health. Retry may help. |

Error response shape:
```json
{ "ok": false, "error": { "code": "invalid_params", "message": "..." } }
```

## Guardrails

- Prefer non-destructive reads (`status`, `list`) before `cancel`/`clear`/`cleanup`.
- Validate `ok: true` and verify returned `action`/`subaction` match the request.
- On `invalid_params`, fix the payload — do not retry the same request.
- On `internal_error`, run `{ "action": "doctor" }` to diagnose service health.
- Never fabricate `job_id` values — obtain from `list` or prior `start` responses.

## Additional Resources

- **[`references/routing-cheatsheet.md`](references/routing-cheatsheet.md)** — Quick-reference table for action routing and required fields.
