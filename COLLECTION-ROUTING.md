# Collection Routing Design

## Goal

Replace the current flat "everything goes to `cortex`" model with **provenance-based collection routing** — content is routed to a collection named after its source, so all knowledge about a given project or community lives together regardless of how it was ingested.

### Why This Matters

When you query the `jmagar/axon_rust` collection directly, you get:
- Claude/Codex/Gemini session logs written while working on the project
- GitHub issues, PRs, and code from the repo

Reddit and YouTube content lives in its own source collection (`r/selfhosted`, `@Fireship`), but is **automatically tagged** with any GitHub projects it references — enabling future cross-collection project queries without duplicating data.

---

## Collection Routing Rules

### Web Operations (unchanged)

| Command | Collection | Source |
|---------|-----------|--------|
| `scrape <url>` | `cortex` | global default |
| `crawl <url>` | `cortex` | global default |
| `embed <file/url>` | `cortex` | global default |
| `search`, `research` | `cortex` | global default |

`cortex` remains the main documentation brain — crawled docs, scraped references, embedded files.

### Ingest Operations (new routing)

| Command | Collection | Derived From | Example |
|---------|-----------|--------------|---------|
| `github <repo>` | `{owner}/{repo}` | repo argument | `github jmagar/axon_rust` → `jmagar/axon_rust` |
| `reddit <target>` | `r/{subreddit}` | subreddit extracted from URL or name | `reddit r/selfhosted` → `r/selfhosted` |
| `youtube <url>` | `@{channel}` | channel name via yt-dlp `%(channel)s` | video from Fireship → `@Fireship` |
| `sessions [path]` | `{owner}/{repo}` | git remote origin of cwd (or provided path) | session in `~/workspace/axon_rust` → `jmagar/axon_rust` |

### Override (always wins)

`--collection <name>` explicitly provided by the user overrides all derived routing for that run.

```bash
# Force github content into cortex
axon github jmagar/axon_rust --collection cortex

# Force a session into a custom collection
axon sessions --collection my-custom-collection
```

---

## Derivation Logic

### `github` — trivial
The collection name IS the repo argument, already normalized by `parse_github_repo()`.

```
"jmagar/axon_rust"            → jmagar/axon_rust
"https://github.com/foo/bar"  → foo/bar
```

### `reddit` — extract subreddit
Strip URL noise, normalize to `r/{name}`:

```
"r/selfhosted"                                              → r/selfhosted
"https://www.reddit.com/r/selfhosted/"                     → r/selfhosted
"https://www.reddit.com/r/selfhosted/comments/8ejqr7/..."  → r/selfhosted
```

### `youtube` — resolve channel via yt-dlp
Before downloading the transcript, run:
```bash
yt-dlp --print channel -- <url>
```
Use the output as the collection name, prefixed with `@`:
```
"Fireship"     → @Fireship
"ThePrimeTime" → @ThePrimeTime
```
Fallback if yt-dlp `--print channel` fails or returns empty: `youtube` (flat fallback, not an error).

### `sessions` — git remote of working directory
Run `git remote get-url origin` in the directory containing the session files, then normalize the remote URL to `owner/repo` format:

```
"https://github.com/jmagar/axon_rust"     → jmagar/axon_rust
"git@github.com:jmagar/axon_rust.git"     → jmagar/axon_rust
"https://github.com/jmagar/axon_rust.git" → jmagar/axon_rust
```

Fallback chain if git remote resolution fails:
1. Try `git remote get-url origin` in cwd
2. Try basename of cwd (e.g. `axon_rust`)
3. Fall back to `sessions`

---

## Project Association via Metadata

Reddit and YouTube content lives in its own collection — **no data is duplicated**. Instead, every embedded document carries a `related_projects` metadata field populated by scanning the content for GitHub repository references.

### How it works

When ingesting a Reddit post or YouTube video, extract all GitHub URLs from the text (post body, comments, video description, transcript) and normalize them to `owner/repo`:

```
"Check out https://github.com/jmagar/axon_rust for self-hosted RAG"
                                          → related_projects: ["jmagar/axon_rust"]

"Source code at https://github.com/foo/bar and https://github.com/baz/qux"
                                          → related_projects: ["foo/bar", "baz/qux"]
```

If no GitHub URLs are found, `related_projects` is an empty array (field still present, just empty).

### Storage

`related_projects` is stored as a keyword array in the Qdrant point payload alongside the existing metadata fields (`source_type`, `url`, `title`, etc.):

```json
{
  "url": "https://www.reddit.com/r/selfhosted/comments/abc/...",
  "source_type": "reddit",
  "title": "Big list of self-hosted services",
  "related_projects": ["jmagar/axon_rust", "awesome-selfhosted/awesome-selfhosted"]
}
```

### What this enables (future)

A future `--related-to <owner/repo>` flag or `--all-collections` mode can filter any collection by `related_projects` metadata, giving a cross-collection project view without changing where data lives:

```
query "vector search" --collection r/selfhosted --filter related_projects=jmagar/axon_rust
```

This is out of scope for the current change but the metadata is cheap to store now and makes that feature trivial to add later.

### Scope

`related_projects` extraction applies to:
- `reddit` — post title + body + all comment bodies
- `youtube` — video description + full transcript text

It does NOT apply to `github` or `sessions` (those already route directly to the project collection).

---

## Implementation Plan

### 1. `config.rs` — make `--collection` optional
Change `collection: String` (hardcoded default `"cortex"`) to distinguish between "user explicitly set" vs "use derived default":

```rust
// New: raw CLI value — None means "derive from target"
pub collection_override: Option<String>,

// Existing field stays for non-ingest commands (query, retrieve, ask, scrape, etc.)
pub collection: String,  // resolved: collection_override OR AXON_COLLECTION OR "cortex"
```

### 2. Add derivation helpers
New module or functions (likely in `crates/ingest/` or `crates/core/`):

```rust
pub fn collection_from_github_repo(repo: &str) -> String
pub fn collection_from_reddit_target(target: &str) -> String
pub async fn collection_from_youtube_url(url: &str) -> String  // subprocess call
pub async fn collection_from_cwd() -> String                   // git remote call
```

### 3. Add `extract_github_refs` helper
New pure function (likely in `crates/ingest/` alongside the other derivation helpers):

```rust
/// Extract all GitHub `owner/repo` references from arbitrary text.
/// Matches `github.com/{owner}/{repo}` patterns (https, http, or bare).
/// Returns deduplicated, normalized strings.
pub fn extract_github_refs(text: &str) -> Vec<String>
```

Regex pattern: `github\.com[/:]([\w.-]+)/([\w.-]+?)(?:\.git)?(?:[/?#]|$)`

Strips `.git` suffix, deduplicates, ignores `github.com/` with no repo path.

### 4. Update `embed_text_with_metadata` signature
Add `related_projects: Vec<String>` parameter (or an `ExtraMetadata` struct if more fields are anticipated):

```rust
pub async fn embed_text_with_metadata(
    cfg: &Config,
    text: &str,
    url: &str,
    source_type: &str,
    title: Option<&str>,
    related_projects: Vec<String>,  // new — empty vec for github/sessions
) -> Result<usize, Box<dyn Error>>
```

Store `related_projects` as a Qdrant payload field of type `keyword[]`.

All existing callers pass `vec![]` — no behavior change for github/sessions/scrape/crawl.

### 5. Update each ingest command handler

**github / sessions** — derive collection, pass `related_projects: vec![]`:
```rust
let collection = cfg.collection_override.clone()
    .unwrap_or_else(|| collection_from_github_repo(repo));
let cfg = Config { collection, ..cfg.clone() };
// embed calls unchanged — related_projects: vec![]
```

**reddit** — derive collection, extract refs from all text before embedding:
```rust
let collection = cfg.collection_override.clone()
    .unwrap_or_else(|| collection_from_reddit_target(target));
let cfg = Config { collection, ..cfg.clone() };

// per post/comment:
let refs = extract_github_refs(&full_text);
embed_text_with_metadata(&cfg, &full_text, &url, "reddit", title, refs).await?;
```

**youtube** — resolve channel for collection, extract refs from description + transcript:
```rust
let collection = cfg.collection_override.clone()
    .unwrap_or_else(|| collection_from_youtube_url(url).await);
let cfg = Config { collection, ..cfg.clone() };

let full_text = format!("{description}\n\n{transcript}");
let refs = extract_github_refs(&full_text);
embed_text_with_metadata(&cfg, &transcript, &url, "youtube", title, refs).await?;
```

### 6. Update `sessions` command
Resolve collection from git remote of the session file directory before embedding. Pass `related_projects: vec![]`.

### 7. `.env.example` + docs
- Remove `AXON_COLLECTION` from ingest-specific docs (it no longer applies to ingest commands by default)
- Add note: `AXON_COLLECTION` still controls the default for `scrape`/`crawl`/`embed`/`query`

---

## Behavior Changes

| Scenario | Before | After |
|----------|--------|-------|
| `axon github jmagar/axon_rust` | → `cortex` | → `jmagar/axon_rust` |
| `axon reddit r/selfhosted` | → `cortex` | → `r/selfhosted` + `related_projects` tagged |
| `axon youtube <fireship-url>` | → `cortex` | → `@Fireship` + `related_projects` tagged |
| `axon sessions` (in axon_rust/) | → `cortex` | → `jmagar/axon_rust` |
| `axon scrape https://docs.rs` | → `cortex` | → `cortex` (unchanged) |
| `axon query "..."` | searches `cortex` | searches `cortex` (unchanged) |
| Any command `--collection foo` | → `foo` | → `foo` (unchanged) |
| Reddit post mentioning github.com/foo/bar | no metadata | `related_projects: ["foo/bar"]` in payload |
| YouTube video with no GitHub links | no metadata | `related_projects: []` in payload |

---

## Open Questions

- **Existing data in `cortex`**: GitHub/Reddit/session data already in `cortex` stays there. Re-ingest to migrate if desired; no automatic migration.
- **Cross-collection query**: `axon query` currently searches one collection. A future `--related-to <owner/repo>` flag or `--all-collections` mode would filter by `related_projects` payload across collections — out of scope for this change, but the metadata is stored now.
- **Private repos**: `sessions` git remote resolution may expose token in URL (`https://token@github.com/...`). Strip credentials before using as collection name.
- **Monorepos / nested repos**: `sessions` called from `~/workspace/axon_rust/crates/ingest/` should still resolve to `jmagar/axon_rust` (walk up to repo root before reading remote).
- **`embed_text_with_metadata` callers**: adding `related_projects` parameter touches every embed callsite in github/, reddit/, youtube/, sessions/, and the vector ops layer. Consider an `ExtraMetadata` struct to avoid future signature churn.
