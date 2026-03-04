# axon research
Last Modified: 2026-03-03

Version: 1.0.0
Last Updated: 20:29:46 | 03/03/2026 EST

Web research pipeline: Tavily search plus one synthesis LLM call over returned snippets. Runs synchronously and prints extracted source previews plus a synthesized summary.

## Synopsis

```bash
axon research <query> [FLAGS]
axon research --query "<query>" [FLAGS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<query>` | Research query text (or use `--query`) |

## Required Environment Variables

| Variable | Description |
|----------|-------------|
| `TAVILY_API_KEY` | Tavily API key for source discovery. |
| `OPENAI_BASE_URL` | OpenAI-compatible base URL (for example `http://host/v1`). Must not include `/chat/completions`. |
| `OPENAI_MODEL` | Model name used for synthesis. |

## Flags

All global flags apply. Key flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--query <text>` | — | Query text (alternative to positional words). |
| `--limit <n>` | `10` | Maximum Tavily results processed. |
| `--openai-base-url <url>` | env/default | Override LLM base URL. |
| `--openai-model <name>` | env/default | Override LLM model. |

## Examples

```bash
# Basic research
axon research "Rust async cancellation patterns"

# Use --query and limit
axon research --query "Qdrant HNSW tuning" --limit 5

# Override LLM endpoint
axon research "Spider.rs rendering tradeoffs" --openai-base-url http://localhost:11434/v1 --openai-model llama3
```

## Pipeline

1. Tavily search fetches ranked results.
2. Each result contributes URL, title, and snippet as extracted evidence.
3. A single LLM completion synthesizes those snippets into a summary.

## Behavior Notes

- `OPENAI_BASE_URL` is validated and must not end with `/chat/completions`.
- `--search-time-range` is applied to the Tavily search step before synthesis.
- With `--json`, output is strict JSON on stdout.
- `research` does not enqueue jobs and does not auto-embed results into Qdrant.
