# `axon reddit`

Ingest posts and comments from a subreddit or a specific thread into Qdrant.

## Usage

```bash
# Ingest 25 hot posts from a subreddit (default)
axon reddit rust

# Ingest top posts of the year from r/selfhosted
axon reddit selfhosted --sort top --time year --max-posts 100

# Ingest a specific thread with deep comment recursion
axon reddit https://www.reddit.com/r/rust/comments/... --depth 5

# Ingest and scrape external links
axon reddit selfhosted --scrape-links
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--sort <mode>` | `hot` | Subreddit sorting: `hot`, `top`, `new`, `rising` |
| `--time <range>` | `day` | Time range for `top` sort: `hour`, `day`, `week`, `month`, `year`, `all` |
| `--max-posts <n>` | `25` | Maximum posts to fetch (0 for unlimited) |
| `--min-score <n>` | `0` | Minimum score threshold for posts and comments |
| `--depth <n>` | `2` | Comment traversal depth |
| `--scrape-links` | `false` | Scrape content of linked URLs in link posts |
| `--wait <bool>` | `false` | Block until ingestion is complete |

## Retrieval Strategy

Axon optimizes Reddit data for RAG (Retrieval-Augmented Generation):

1.  **Per-Comment Embedding**: Each post body and each individual comment is embedded as a separate point in Qdrant.
2.  **Context Chaining**: Child comments are embedded with the **Post Title** and **Parent Comment** text injected into their context. This makes individual chunks semantically self-contained.
3.  **Deduplication**: Axon checks Qdrant before embedding. Re-running ingestion is idempotent and only adds new content.
4.  **Link Enrichment**: When `--scrape-links` is enabled, Axon fetches the target of external links and includes its markdown in the post's embedding.
