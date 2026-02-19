---
description: Ask grounded questions over indexed docs (RAG)
argument-hint: "<question>" [--diagnostics] [--json]
allowed-tools: Bash(axon *)
---

# Ask AI-Grounded Questions

Execute:

```bash
axon ask $ARGUMENTS
```

## Behavior

`ask` performs retrieval-augmented generation:
1. Embeds the question via TEI
2. Searches Qdrant for candidate chunks
3. Reranks and filters candidates
4. Builds bounded context with source labels like `[S1]`
5. Calls `OPENAI_BASE_URL/chat/completions` using `OPENAI_MODEL`

The command returns a final answer (non-streaming) plus timing data.

## CLI Options

| Option | Description |
|---|---|
| `--diagnostics` | Emit retrieval/context diagnostics |
| `--json` | Emit JSON output instead of plaintext |

## Environment Knobs

| Variable | Default | Purpose |
|---|---|---|
| `AXON_ASK_CANDIDATE_LIMIT` | `64` | Search candidate pool size |
| `AXON_ASK_CHUNK_LIMIT` | `10` | High-signal chunk entries |
| `AXON_ASK_FULL_DOCS` | `4` | Full-document sources to attempt |
| `AXON_ASK_BACKFILL_CHUNKS` | `3` | Supplemental chunk count |
| `AXON_ASK_DOC_FETCH_CONCURRENCY` | `4` | Parallel doc retrieval fanout |
| `AXON_ASK_DOC_CHUNK_LIMIT` | `192` | Max chunks fetched per full doc |
| `AXON_ASK_MIN_RELEVANCE_SCORE` | `0.0` | Minimum rerank score threshold |
| `AXON_ASK_MAX_CONTEXT_CHARS` | `120000` | Hard cap for assembled context |

Required model settings:
- `OPENAI_BASE_URL`
- `OPENAI_MODEL`
- `OPENAI_API_KEY` (if endpoint requires auth)

## Output

Plaintext mode:
- `Conversation`
- `You: <question>`
- `Assistant: <answer>`
- Optional diagnostics
- `Timing: retrieval=... | context=... | llm=... | total=...`

JSON mode:
- `query`
- `answer`
- `diagnostics` (if enabled)
- `timing_ms` (`retrieval`, `context_build`, `llm`, `total`)
