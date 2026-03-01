# Axon MCP Routing Cheatsheet

Canonical docs:
- `docs/MCP-TOOL-SCHEMA.md` — wire contract (source of truth)
- `docs/MCP.md` — runtime model + smoke tests

## Direct Actions (no subaction)

| Action | Required | Optional |
|--------|----------|----------|
| `scrape` | `url` | `response_mode` |
| `search` | `query` | `limit` |
| `research` | `query` | — |
| `ask` | `query` | `limit` |
| `query` | `query` | `limit`, `offset` |
| `retrieve` | `url` | — |
| `map` | `url` | — |
| `screenshot` | `url` | — |
| `help` | — | — |
| `doctor` | — | — |
| `status` | — | — |
| `domains` | — | `limit` |
| `sources` | — | `limit` |
| `stats` | — | — |

## Lifecycle Families (require subaction)

Families: `crawl`, `extract`, `embed`, `ingest`

| Subaction | Required | Notes |
|-----------|----------|-------|
| `start` | family-specific (see below) | Default when `subaction` omitted |
| `status` | `job_id` | |
| `cancel` | `job_id` | |
| `list` | — | Optional `limit`, `offset` |
| `cleanup` | — | Remove completed/failed jobs |
| `clear` | — | Remove all jobs |
| `recover` | — | Reclaim stale/interrupted jobs |

### Start Fields by Family

| Family | Required |
|--------|----------|
| `crawl` | `urls` (non-empty array) |
| `extract` | `urls` (non-empty array) |
| `embed` | `input` |
| `ingest` | `source_type` + `target` |

## Artifact Inspection

Action: `artifacts`, subactions: `head | read | wc | grep`

| Subaction | Required |
|-----------|----------|
| `head` | `path` |
| `read` | `path` |
| `wc` | `path` |
| `grep` | `path`, `pattern` |

Optional: `limit`, `offset` for paginated output.

## Parser Rules (Strict)

- `action` is **required** — must match canonical names exactly
- `subaction` is **required** for lifecycle families and `artifacts`
- **No fallback fields** — `command`, `op`, `operation` are not accepted
- **No alias remapping** — action names must be exact
- **No normalization** — no case folding, no dash/space rewriting
