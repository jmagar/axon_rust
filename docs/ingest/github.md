# GitHub Ingest

> CLI reference (flags, subcommands, examples): [`docs/commands/github.md`](../commands/github.md)

Ingests a GitHub repository — source code, documentation, issues, pull requests, and wiki pages — into Qdrant via a hybrid approach: raw reqwest for file content, octocrab for metadata/issues/PRs, and `git clone` for wiki pages.

## What Gets Indexed

| Content | Condition |
|---------|-----------|
| Documentation files | Always: `.md`, `.mdx`, `.rst`, `.txt` |
| Source code files | When `--include-source` flag is set: `.rs`, `.py`, `.go`, `.ts`, `.js`, `.tsx`, `.jsx`, `.toml`, `.c`, `.cpp`, `.h`, `.hpp`, `.java`, `.kt`, `.rb`, `.php`, `.sh`, `.yaml`, `.yml`, `.json`, `.swift`, `.cs` |
| Issues | Open and closed, title + body |
| Pull requests | Open and closed, title + body |
| Wiki pages | When the repo has a public wiki |

**Excluded** regardless of flag: `target/`, `node_modules/`, `dist/`, `__pycache__/`, `.lock` files, `-lock.json` files. See `is_indexable_source_path()` in `crates/ingest/github/mod.rs` for the full list.

## Prerequisites

A running Qdrant + TEI stack. `GITHUB_TOKEN` is optional but strongly recommended for any repo with more than a handful of files.

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `GITHUB_TOKEN` | Optional | Personal access token (classic) with `repo` scope, or fine-grained token with `contents:read`. Without this: **60 req/hr**. With this: **5,000 req/hr**. Large repos hit the unauthenticated limit quickly. Required for private repos. |
| `AXON_COLLECTION` | Optional | Qdrant collection name (default: `cortex`) |
| `TEI_URL` | Required | TEI embedding service URL |

```bash
# .env
GITHUB_TOKEN=ghp_your_token_here
```

## URL / Name Parsing

The argument accepts:
- `owner/repo` — canonical form
- `https://github.com/owner/repo` — full URL (prefix stripped)
- `https://github.com/owner/repo.git` — `.git` suffix stripped

## How It Works

1. Validates and normalizes `owner/repo` from the input
2. Fetches the full file tree via `GET /repos/{owner}/{repo}/git/trees/{sha}?recursive=1`
3. Filters files through `is_indexable_doc_path()` (always) and `is_indexable_source_path()` (if `--include-source`)
4. Fetches file contents in parallel via `GET /repos/{owner}/{repo}/contents/{path}`
5. Fetches issues (all states) and PRs (all states) via octocrab with automatic pagination; embeds repo metadata (description, language, topics, license) from the `GET /repos/{owner}/{repo}` response; clones the wiki via `git clone --depth=1` and walks `.md`/`.rst`/`.txt` files
6. All content embedded via `embed_text_with_metadata()` → TEI → Qdrant with the GitHub URL as source metadata

## Known Limitations

| Limitation | Detail |
|-----------|--------|
| **Rate limits without token** | 60 req/hr unauthenticated. Any repo with 60+ files will exhaust this in one run. Set `GITHUB_TOKEN`. |
| **Private repos** | Require a token with `repo` (classic) or `contents:read` (fine-grained) scope |
| **Very large repos** | Tree-first + per-file fetching is O(file count). Large repos (thousands of files) take minutes even with a token. |
| **Binary files** | Excluded by extension list. The list is hardcoded; PRs welcome for additions. |
| **Forked repos** | Ingests the fork only, not upstream. |

## Troubleshooting

**`403 Forbidden` / rate limit errors**

Set `GITHUB_TOKEN` in `.env`. Verify the token has `contents:read` access (fine-grained) or `repo` scope (classic).

**`repository not found`**

Repo is private or doesn't exist. Check the owner/repo spelling and token permissions.

**Slow ingestion on large repos**

Expected — tree walk + per-file API calls for thousands of files is inherently sequential-ish (parallelism is bounded by GitHub's rate limit). Consider indexing only docs (`--include-source` off) or using a token to maximize rate allowance.
