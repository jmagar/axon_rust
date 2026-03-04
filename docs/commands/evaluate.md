# axon evaluate
Last Modified: 2026-03-03

Version: 1.0.0
Last Updated: 23:05:00 | 03/03/2026 EST

Evaluate RAG quality versus a baseline. The command generates:
1) RAG answer (with retrieved context)
2) baseline answer (no retrieved context)
3) judge analysis comparing both answers against retrieved reference material.

## Synopsis

```bash
axon evaluate <question> [FLAGS]
axon evaluate --query "<question>" [FLAGS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<question>` | Evaluation question (positional, or via `--query`). |

## Required Environment Variables

| Variable | Description |
|----------|-------------|
| `AXON_PG_URL` | Required by global config parsing (all commands). |
| `AXON_REDIS_URL` | Required by global config parsing (all commands). |
| `AXON_AMQP_URL` | Required by global config parsing (all commands). |
| `TEI_URL` | TEI embeddings base URL (retrieval and judge reference). |
| `QDRANT_URL` | Qdrant base URL. |
| `OPENAI_BASE_URL` | OpenAI-compatible API base URL (for RAG/baseline/judge calls). |
| `OPENAI_MODEL` | Model name for all evaluate LLM calls. |

## Flags

All global flags apply. Key flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--query <text>` | — | Question text (alternative to positional argument). |
| `--collection <name>` | `cortex` | Qdrant collection to retrieve from. |
| `--diagnostics` | `false` | Adds retrieval pool/context diagnostics in output. |
| `--responses-mode <mode>` | `side-by-side` | Live response rendering mode: `inline`, `side-by-side`, or `events` (NDJSON stream events). |
| `--json` | `false` | Emits one structured JSON object with answers + timing. |

Note: `evaluate` runs synchronously and does not enqueue jobs.

## Examples

```bash
# Basic evaluate run
axon evaluate "How does auto-switch choose Chrome fallback?"

# Using --query
axon evaluate --query "What does AXON_DOMAINS_DETAILED change?"

# Diagnostics + JSON
axon evaluate "How does ask citation gating work?" --diagnostics --json

# True side-by-side terminal rendering
axon evaluate "How does auto-switch choose Chrome fallback?" --responses-mode side-by-side

# Structured event stream (NDJSON)
axon evaluate "How does auto-switch choose Chrome fallback?" --responses-mode events
```

## Output

Human output streams three sections in order:
- `RAG Answer (with context)`
- `Baseline Answer (no context)`
- `Analysis`
- If judge scoring indicates RAG underperformed baseline, a follow-up section is appended:
  `Suggested Sources To Crawl`.
- Suggested sources are auto-enqueued as crawl jobs immediately after generation.

JSON output includes:
- `rag_answer`
- `baseline_answer`
- `analysis_answer`
- `crawl_suggestions` (present when generated)
- `crawl_enqueue_outcomes` (url + job_id or enqueue error)
- `timing_ms` (retrieval/context/rag_llm/baseline_llm/research/judge/total)

## Notes

- If streaming fails for any LLM phase, evaluate falls back to non-streaming for that phase.
- Judge reference retrieval is best-effort; evaluate continues even if reference gathering fails.
- In `--responses-mode events`, output is NDJSON events (`evaluate_start`, `token`, `stream_done`, `analysis_start`, `evaluate_complete`).
