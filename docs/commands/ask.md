# axon ask
Last Modified: 2026-02-25

Version: 1.0.0
Last Updated: 01:26:53 | 02/25/2026 EST

RAG-powered Q&A. Retrieves relevant chunks from the local Qdrant knowledge base, reranks them by relevance, builds a context window, and calls the configured LLM to generate a grounded answer.

## Synopsis

```bash
axon ask <question> [FLAGS]
axon ask --query "<question>" [FLAGS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<question>` | Question to answer (positional, or via `--query`) |

## Required Environment Variables

| Variable | Description |
|----------|-------------|
| `TEI_URL` | TEI embeddings base URL. Used to embed the question before Qdrant search. |
| `QDRANT_URL` | Qdrant base URL. Searched for relevant chunks. |
| `OPENAI_BASE_URL` | OpenAI-compatible API base URL (e.g. `http://host/v1`). **Do not append `/chat/completions`**. |
| `OPENAI_MODEL` | Model name for answer generation. |

## Flags

All global flags apply. Key flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--query <text>` | — | Question text (alternative to positional argument). |
| `--collection <name>` | `cortex` | Qdrant collection to search. |
| `--limit <n>` | `10` | Initial chunk limit (overridden by `AXON_ASK_CHUNK_LIMIT` tuning). |
| `--diagnostics` | `false` | Print retrieved chunks and scores for debugging. |
| `--json` | `false` | Machine-readable JSON output. |

Note: `ask` runs synchronously and does not support `--wait`.

## Examples

```bash
# Basic ask
axon ask "how does spider.rs handle JavaScript-heavy sites?"

# Using --query flag
axon ask --query "what is the default chunk size for TEI batch requests?"

# Specific collection
axon ask "list all indexed rust crates" --collection rust-libs

# Debug: show retrieved chunks and scores
axon ask "qdrant HNSW parameters" --diagnostics

# JSON output
axon ask "what is the max crawl depth?" --json
```

## RAG Pipeline

1. Embed the question via TEI
2. Query Qdrant for top `AXON_ASK_CANDIDATE_LIMIT` (default: 64) candidate chunks
3. Filter chunks below `AXON_ASK_MIN_RELEVANCE_SCORE` (default: 0.45)
4. Rerank by score; take top `AXON_ASK_CHUNK_LIMIT` (default: 10)
5. For top `AXON_ASK_FULL_DOCS` (default: 4) documents, backfill additional chunks from the same document
6. Assemble context up to `AXON_ASK_MAX_CONTEXT_CHARS` (default: 120,000) characters
7. Call the LLM with context + question
8. Apply response-quality gates (citations + policy checks)
9. Print the normalized answer

## RAG Tuning

The retrieval pipeline is tunable via environment variables. See the [Environment section](../../README.md#ask-rag-tuning) in the README for the full table. Short reference:

| Variable | Default | Effect |
|----------|---------|--------|
| `AXON_ASK_MIN_RELEVANCE_SCORE` | `0.45` | Raise to tighten relevance (0.6–0.7 for high-precision); lower if you get "no candidates" |
| `AXON_ASK_CANDIDATE_LIMIT` | `64` | More candidates = better recall, slower reranking |
| `AXON_ASK_CHUNK_LIMIT` | `10` | Chunks in final LLM context |
| `AXON_ASK_MAX_CONTEXT_CHARS` | `120000` | Total context characters; raise for large-context models |
| `AXON_ASK_AUTHORITATIVE_DOMAINS` | `` | Optional comma-separated domains to boost in reranking |
| `AXON_ASK_AUTHORITATIVE_BOOST` | `0.0` | Score boost for authoritative-domain matches |
| `AXON_ASK_AUTHORITATIVE_ALLOWLIST` | `` | Optional strict domain allowlist for retrieval candidates |
| `AXON_ASK_MIN_CITATIONS_NONTRIVIAL` | `2` | Minimum unique citations for non-trivial answers |

## Notes

- `OPENAI_BASE_URL` must be the base URL only: `http://host/v1` — **not** `http://host/v1/chat/completions`.
- If you get "No candidates met relevance threshold", lower `AXON_ASK_MIN_RELEVANCE_SCORE` or run `axon crawl`/`axon embed` to add more content to the collection.
- `ask` queries the local knowledge base only. To search the live web, use `axon research`.
- For benchmarking RAG quality vs a baseline, use `axon evaluate`.
- `ask` enforces output policy gates:
  - Procedural queries must cite at least one official docs source.
  - Config/schema queries must cite at least one exact page (not just a root domain).
  - Non-trivial responses must satisfy `AXON_ASK_MIN_CITATIONS_NONTRIVIAL`.
  - Failed gates are converted to structured insufficient-evidence output.
