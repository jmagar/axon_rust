# Database Schema

Tables are auto-created on first worker/command start via `CREATE TABLE IF NOT EXISTS` in each `*_jobs.rs` file's `ensure_schema()` function.

## axon_crawl_jobs

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| `id` | UUID | NOT NULL | — | Primary key, job identifier |
| `url` | TEXT | NOT NULL | — | Target URL for the crawl |
| `status` | TEXT | NOT NULL | — | `pending` / `running` / `completed` / `failed` / `canceled` |
| `created_at` | TIMESTAMPTZ | NOT NULL | `NOW()` | Job creation timestamp |
| `updated_at` | TIMESTAMPTZ | NOT NULL | `NOW()` | Last status change |
| `started_at` | TIMESTAMPTZ | NULL | — | When worker began processing |
| `finished_at` | TIMESTAMPTZ | NULL | — | When job completed/failed/canceled |
| `error_text` | TEXT | NULL | — | Error message on failure |
| `result_json` | JSONB | NULL | — | Crawl results (pages found, stats) |
| `config_json` | JSONB | NOT NULL | — | Serialized job configuration |

**Index:** `idx_axon_crawl_jobs_status` on `status`.

## axon_extract_jobs

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| `id` | UUID | NOT NULL | — | Primary key |
| `status` | TEXT | NOT NULL | — | `pending` / `running` / `completed` / `failed` / `canceled` |
| `created_at` | TIMESTAMPTZ | NOT NULL | `NOW()` | Job creation timestamp |
| `updated_at` | TIMESTAMPTZ | NOT NULL | `NOW()` | Last status change |
| `started_at` | TIMESTAMPTZ | NULL | — | When worker began processing |
| `finished_at` | TIMESTAMPTZ | NULL | — | When job completed/failed/canceled |
| `error_text` | TEXT | NULL | — | Error message on failure |
| `urls_json` | JSONB | NOT NULL | — | Array of URLs for LLM extraction |
| `result_json` | JSONB | NULL | — | Extracted structured data |
| `config_json` | JSONB | NOT NULL | — | Serialized job configuration |

## axon_embed_jobs

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| `id` | UUID | NOT NULL | — | Primary key |
| `status` | TEXT | NOT NULL | — | `pending` / `running` / `completed` / `failed` / `canceled` |
| `created_at` | TIMESTAMPTZ | NOT NULL | `NOW()` | Job creation timestamp |
| `updated_at` | TIMESTAMPTZ | NOT NULL | `NOW()` | Last status change |
| `started_at` | TIMESTAMPTZ | NULL | — | When worker began processing |
| `finished_at` | TIMESTAMPTZ | NULL | — | When job completed/failed/canceled |
| `error_text` | TEXT | NULL | — | Error message on failure |
| `input_text` | TEXT | NOT NULL | — | Input path, URL, or text to embed |
| `result_json` | JSONB | NULL | — | Embedding results (chunk count, point IDs) |
| `config_json` | JSONB | NOT NULL | — | Serialized job configuration |

## axon_ingest_jobs

This table differs structurally from the other four: it uses `source_type` and `target` to identify the ingest target rather than a `url` or `urls_json` column. The `source_type` discriminator routes processing to the correct ingest backend (GitHub API, Reddit OAuth2, yt-dlp).

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| `id` | UUID | NOT NULL | — | Primary key, job identifier |
| `status` | TEXT | NOT NULL | — | `pending` / `running` / `completed` / `failed` / `canceled` |
| `source_type` | TEXT | NOT NULL | — | Ingest backend discriminator: `github`, `reddit`, or `youtube` |
| `target` | TEXT | NOT NULL | — | Ingest target: GitHub repo (`owner/repo`), subreddit name, or YouTube video/playlist/channel URL |
| `created_at` | TIMESTAMPTZ | NOT NULL | `NOW()` | Job creation timestamp |
| `updated_at` | TIMESTAMPTZ | NOT NULL | `NOW()` | Last status change |
| `started_at` | TIMESTAMPTZ | NULL | — | When worker began processing |
| `finished_at` | TIMESTAMPTZ | NULL | — | When job completed/failed/canceled |
| `error_text` | TEXT | NULL | — | Error message on failure |
| `result_json` | JSONB | NULL | — | Ingest results: `{"chunks_embedded": N}` |
| `config_json` | JSONB | NOT NULL | — | Serialized `IngestJobConfig` (source variant + collection name) |

**Index:** `idx_axon_ingest_jobs_pending` — partial index on `created_at ASC WHERE status = 'pending'` for efficient FIFO queue polling.

### source_type values

| Value | Target format | Command |
|-------|--------------|---------|
| `github` | `owner/repo` (e.g. `rust-lang/rust`) | `axon github <owner/repo>` |
| `reddit` | subreddit name (e.g. `rust`) or thread URL | `axon reddit <target>` |
| `youtube` | video, playlist, or channel URL | `axon youtube <url>` |

### Structural differences vs other job tables

| Table | Target column(s) | Notes |
|-------|-----------------|-------|
| `axon_crawl_jobs` | `url TEXT NOT NULL` | Single crawl seed URL |
| `axon_extract_jobs` | `urls_json JSONB NOT NULL` | Array of URLs |
| `axon_embed_jobs` | `input_text TEXT NOT NULL` | File path, URL, or raw text |
| `axon_ingest_jobs` | `source_type TEXT` + `target TEXT` | Discriminated source type + typed target |
