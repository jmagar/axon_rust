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

Axon operations go through `mcp__axon__axon` MCP tool by default, with CLI fallback when MCP is unavailable.

## Execution Order

1. **Try MCP first** — call `mcp__axon__axon` directly with action/subaction routing.
2. **If MCP fails** (connection refused, tool not found, server down) — fall back to the CLI via Bash:
   ```bash
   ./scripts/axon <command> [args] --json --wait true
   ```
3. **Never mix** — use one or the other per operation, not both.

### Detecting MCP failure

MCP is down if you get: `MCP error`, `tool not found`, `connection refused`, or `EPIPE`. On any of these, switch to CLI for the rest of the session (or until MCP reconnects).

## Quick Start

```json
mcp__axon__axon({ "action": "help" })
```

CLI equivalent:
```bash
./scripts/axon help --json
```

## Key Contract

- **MCP tool**: `mcp__axon__axon` (preferred — call directly)
- **CLI fallback**: `./scripts/axon <command> [args] --json --wait true` (auto-sources `.env`)
- **Routing**: `action` (required) + `subaction` (lifecycle families only)
- **Response default**: `response_mode=path` — artifacts written to `.cache/axon-mcp/`, compact metadata returned
- **Parser**: Strict serde — no fallback fields, no alias remapping, no case folding

## Action Catalog

### Direct Actions (no subaction needed)

| Action | Required Fields | Example Call |
|--------|----------------|-------------|
| `scrape` | `url` | `{ "action": "scrape", "url": "https://example.com" }` |
| `search` | `query` | `{ "action": "search", "query": "rust async patterns" }` |
| `research` | `query` | `{ "action": "research", "query": "MCP protocol design" }` |
| `ask` | `query` | `{ "action": "ask", "query": "what is the axon architecture?" }` |
| `query` | `query` | `{ "action": "query", "query": "embedding pipeline", "limit": 10 }` |
| `retrieve` | `url` | `{ "action": "retrieve", "url": "https://example.com/docs" }` |
| `map` | `url` | `{ "action": "map", "url": "https://docs.example.com" }` |
| `screenshot` | `url` | `{ "action": "screenshot", "url": "https://example.com" }` |
| `help` | — | `{ "action": "help" }` |
| `doctor` | — | `{ "action": "doctor" }` |
| `status` | — | `{ "action": "status" }` |
| `domains` | — | `{ "action": "domains", "limit": 20 }` |
| `sources` | — | `{ "action": "sources", "limit": 20 }` |
| `stats` | — | `{ "action": "stats" }` |

### Lifecycle Families (require subaction)

Families: `crawl`, `extract`, `embed`, `ingest`

| Subaction | Required | Example |
|-----------|----------|---------|
| `start` (default) | family-specific | `{ "action": "crawl", "urls": ["https://docs.example.com"] }` |
| `status` | `job_id` | `{ "action": "crawl", "subaction": "status", "job_id": "<uuid>" }` |
| `cancel` | `job_id` | `{ "action": "crawl", "subaction": "cancel", "job_id": "<uuid>" }` |
| `list` | — | `{ "action": "embed", "subaction": "list", "limit": 5 }` |
| `cleanup` | — | `{ "action": "extract", "subaction": "cleanup" }` |
| `clear` | — | `{ "action": "crawl", "subaction": "clear" }` |
| `recover` | — | `{ "action": "embed", "subaction": "recover" }` |

#### Start Fields by Family

| Family | Required | Example |
|--------|----------|---------|
| `crawl` | `urls` (array) | `{ "action": "crawl", "urls": ["https://example.com"] }` |
| `extract` | `urls` (array) | `{ "action": "extract", "urls": ["https://example.com/pricing"] }` |
| `embed` | `input` | `{ "action": "embed", "input": "https://example.com" }` |
| `ingest` | `source_type` + `target` | `{ "action": "ingest", "source_type": "github", "target": "owner/repo" }` |

#### Ingest Source Types

| `source_type` | `target` format | Example |
|---------------|----------------|---------|
| `github` | `owner/repo` | `"target": "anthropics/claude-code"` |
| `reddit` | subreddit name or thread URL | `"target": "rust"` |
| `youtube` | video URL | `"target": "https://youtube.com/watch?v=..."` |
| `sessions` | directory path | `"target": "./exports"` |

### Artifact Inspection

```json
{ "action": "artifacts", "subaction": "head", "path": "help-actions.json", "limit": 20 }
{ "action": "artifacts", "subaction": "read", "path": "query-test.json" }
{ "action": "artifacts", "subaction": "wc", "path": "crawl-start.json" }
{ "action": "artifacts", "subaction": "grep", "path": "search-rust.json", "pattern": "tokio" }
```

## Response Handling

All responses follow:
```json
{ "ok": true, "action": "<resolved>", "subaction": "<resolved>", "data": { } }
```

- `response_mode=path` (default): artifact saved to `.cache/axon-mcp/`, metadata returned (path, bytes, sha256, preview)
- `response_mode=inline`: full content inline (capped/truncated)
- `response_mode=both`: artifact + inline

Prefer `path` mode to keep context lean. Use `artifacts` action to inspect.

## Error Handling

| Error | Meaning | Action |
|-------|---------|--------|
| `invalid_params` | Bad action, missing field, wrong type | Fix the payload. Do not retry. |
| `internal_error` | Service down, timeout, crash | Run `{ "action": "doctor" }`. Retry may help. |

## Guardrails

- Prefer non-destructive reads (`status`, `list`) before `cancel`/`clear`/`cleanup`.
- Validate `ok: true` and verify returned `action`/`subaction` match the request.
- On `invalid_params`, fix the payload — do not retry the same request.
- Never fabricate `job_id` values — obtain from `list` or prior `start` responses.

## CLI Fallback Reference

When MCP is unavailable, map actions to CLI commands via `./scripts/axon`:

| MCP Call | CLI Equivalent |
|----------|---------------|
| `{ "action": "query", "query": "test", "limit": 5 }` | `./scripts/axon query "test" --limit 5 --json` |
| `{ "action": "ask", "query": "what is X?" }` | `./scripts/axon ask "what is X?" --json` |
| `{ "action": "scrape", "url": "https://..." }` | `./scripts/axon scrape "https://..." --json --wait true` |
| `{ "action": "crawl", "urls": ["https://..."] }` | `./scripts/axon crawl "https://..." --json --wait true` |
| `{ "action": "search", "query": "term" }` | `./scripts/axon search "term" --json` |
| `{ "action": "research", "query": "topic" }` | `./scripts/axon research "topic" --json` |
| `{ "action": "embed", "input": "file.md" }` | `./scripts/axon embed "file.md" --json --wait true` |
| `{ "action": "retrieve", "url": "https://..." }` | `./scripts/axon retrieve "https://..." --json` |
| `{ "action": "map", "url": "https://..." }` | `./scripts/axon map "https://..." --json` |
| `{ "action": "ingest", "source_type": "github", "target": "o/r" }` | `./scripts/axon github "o/r" --json --wait true` |
| `{ "action": "crawl", "subaction": "status", "job_id": "..." }` | `./scripts/axon crawl status "..." --json` |
| `{ "action": "crawl", "subaction": "list" }` | `./scripts/axon crawl list --json` |
| `{ "action": "doctor" }` | `./scripts/axon doctor --json` |
| `{ "action": "status" }` | `./scripts/axon status --json` |
| `{ "action": "stats" }` | `./scripts/axon stats --json` |
| `{ "action": "domains" }` | `./scripts/axon domains --json` |
| `{ "action": "sources" }` | `./scripts/axon sources --json` |
| `{ "action": "screenshot", "url": "https://..." }` | *(no CLI equivalent — MCP only)* |

**CLI rules:**
- Always pass `--json` for machine-readable output
- Pass `--wait true` for async commands (crawl, embed, extract, ingest) to block until completion
- The wrapper script (`./scripts/axon`) auto-sources `.env` — no manual env setup needed
- CLI runs from the repo root: `/home/jmagar/workspace/axon_rust`

## Additional Resources

- **[`references/routing-cheatsheet.md`](references/routing-cheatsheet.md)** — Quick-reference table for action routing and required fields.
