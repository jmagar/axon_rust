# axon research

Deep web research powered by Tavily AI search and an OpenAI-compatible LLM. The command searches the web for the query, scrapes and extracts key information from top results, then uses the LLM to synthesize a comprehensive answer with citations.

## Synopsis

```bash
axon research <query> [FLAGS]
axon research --query "<query>" [FLAGS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<query>` | Research question or topic (positional, or via `--query`) |

## Required Environment Variables

| Variable | Description |
|----------|-------------|
| `TAVILY_API_KEY` | Tavily AI Search API key. Get one at `https://tavily.com`. |
| `OPENAI_BASE_URL` | OpenAI-compatible API base URL (e.g. `http://host/v1`). **Do not include `/chat/completions`** — the command appends that path automatically. |
| `OPENAI_MODEL` | Model name for LLM synthesis (e.g. `gemini-2-flash`, `gpt-4o-mini`). |

Set in `.env`:

```bash
TAVILY_API_KEY=tvly-yourkey
OPENAI_BASE_URL=http://your-llm-host/v1
OPENAI_API_KEY=your-api-key-or-empty
OPENAI_MODEL=your-model-name
```

## Flags

All global flags apply. Key flags for this command:

| Flag | Default | Description |
|------|---------|-------------|
| `--query <text>` | — | Query text (alternative to positional argument). |
| `--limit <n>` | `10` | Maximum search results to retrieve and process. |
| `--json` | `false` | Machine-readable JSON output. |

Note: `research` runs synchronously and does not support `--wait`. It does not use the AMQP queue.

## Examples

```bash
# Basic research
axon research "Rust async runtime internals"

# Using --query flag
axon research --query "best practices for Qdrant collection design"

# Limit search results
axon research "tokio task spawning overhead" --limit 5

# JSON output for programmatic use
axon research "spider.rs crawl architecture" --json
```

## Output

The command prints:

1. **Search Results** — count of Tavily results retrieved
2. **Pages Extracted** — count of pages successfully scraped and extracted
3. Per-extraction: title, URL, and a 200-character preview of extracted data
4. **Summary** — LLM-synthesized answer grounded in the extracted content
5. **Token usage** — prompt, completion, and total token counts (if the LLM returns them)

## Pipeline

1. Sends the query to Tavily's search API, receiving ranked URLs with snippets
2. For each result, scrapes the page and extracts key facts using the LLM (`spider_agent`)
3. Synthesizes all extractions into a single grounded summary using the LLM

This is distinct from `axon ask`, which searches the local Qdrant knowledge base. `research` searches the live web.

## Notes

- `OPENAI_BASE_URL` must be the base URL only: `http://host/v1` — **not** `http://host/v1/chat/completions`. The command validates this and returns a clear error if the path is wrong.
- The Tavily API has rate limits based on your plan. Check `https://docs.tavily.com` for details.
- Results are not automatically embedded into Qdrant. To index research results for future `ask` queries, pipe the output through `axon embed` or use `axon search` (which auto-queues crawl jobs for result URLs).
- `research` uses `spider_agent`'s `ResearchOptions` with `synthesize: true`. The synthesis step calls the LLM once with all extracted content, so token cost scales with `--limit`.

## Comparison: research vs search vs ask

| Command | Data source | LLM synthesis | Embeds results? |
|---------|-------------|--------------|----------------|
| `ask` | Local Qdrant knowledge base | Yes | No |
| `search` | Tavily web search | No | Auto-queues crawl jobs |
| `research` | Tavily web search + page extraction | Yes | No |
