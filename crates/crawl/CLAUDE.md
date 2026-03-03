# crates/crawl — Spider.rs Crawl Engine
Last Modified: 2026-03-03

Wraps spider.rs for site crawling with HTTP and Chrome rendering paths.

## Key Files
- `engine.rs` — `crawl_and_collect_map()`, `run_crawl_once()`, `crawl_sitemap_urls()`, `append_sitemap_backfill()`, `try_auto_switch()`, `should_fallback_to_chrome()`

## Critical Patterns

### crawl_raw() vs crawl()
- `crawl_raw()` — pure HTTP, always available, no Chrome dependency
- `crawl()` — Chrome-aware, requires a running Chrome instance

`engine.rs` calls:
- `crawl_raw()` for `RenderMode::Http`
- `crawl()` for `RenderMode::Chrome` and `RenderMode::AutoSwitch`

If Chrome is unavailable and mode is AutoSwitch, `try_auto_switch()` falls back and keeps the HTTP result.

### configure_website() Chain
Called once per crawl in `engine.rs`. Two entry points:
- `configure_website(cfg, url, mode)` — CLI path (no crawl_id, control thread is a no-op)
- `configure_website_with_crawl_id(cfg, url, mode, Some(id))` — worker path (sets crawl_id for `spider::utils::shutdown()`)

Both are `pub(super)` in `runtime.rs`. Fixed internal calls (do NOT remove):
```rust
website.with_retry(retries as u8)   // clamp to u8 — must not exceed 255
       .with_normalize()             // URL normalization — required for dedup
       .with_tld(false);             // hardcoded — do not change
```
`with_no_control_thread(false)` enables spider's control handler — **do not set to `true`**, it breaks graceful cancel.

`scrape.rs`, `thin_refetch.rs`, and `content/engine.rs` have their **own independent** `Website::new()` paths — keep retry, `custom_headers`, and UA settings in sync when changing `configure_website()` behavior.

### Auto-Switch Logic
`try_auto_switch()` triggers Chrome fallback when:
- >60% of pages are thin (below `--min-markdown-chars`, default 200 chars), **OR**
- total pages crawled is below a minimum coverage threshold

Chrome requires `AXON_CHROME_REMOTE_URL` set. If not set, HTTP result is kept.

### Junk URL Filter (`is_junk_discovered_url`)
`engine.rs` registers `website.set_on_link_find()` during `configure_website()` which calls `is_junk_discovered_url()` on every discovered link **before** the blacklist regex and before any fetch. Rejecting here prevents bad URLs from entering the crawl queue at all.

Heuristics (each sufficient to reject):
- URL length > 2048 characters
- Encoded HTML tags in URL path (`%3C`/`%3E`)
- Template literal placeholders (`%7B`/`%7D`)
- 3 or more `%20` sequences in the URL path
- JS string concat artifact: `%20)` anywhere in path

Returns `CaseInsensitiveString::default()` to reject; returns the original string to allow. Only checks the URL path, not the query string, to avoid false positives on legitimate query parameters.

### Mid-Crawl Cancellation (Redis + Spider Control)
Two-layer cancel: Redis for cross-process signaling, spider `control` feature for in-process graceful shutdown.

**Redis layer** — `run_active_crawl_job` in `process.rs` races the crawl future against `poll_cancel_key`:
- Polls Redis key `axon:crawl:cancel:{job_id}` every **3 seconds**
- **Fail-safe:** returns `false` on any Redis error — a Redis outage never false-cancels a crawl
- Cancel a running crawl: `axon crawl cancel <job_id>` (sets the Redis key)

**Spider control layer** — when the Redis cancel key is detected:
1. Calls `spider::utils::shutdown("{crawl_id}{url}")` — signals spider's in-process control thread via `AtomicI8`
2. Spider stops dispatching new pages immediately, drains in-flight requests gracefully
3. The crawl future is **awaited** (not dropped) with a 30s timeout, returning partial results
4. Partial results (`pages_crawled`, `md_created`, `elapsed_ms`) are saved to `result_json` in the DB
5. Job is marked `canceled` (not `failed`) — the `WHERE status='running'` guard prevents racing with natural completion

**crawl_id wiring:** `configure_website_with_crawl_id()` in `runtime.rs` sets `website.with_crawl_id(job_uuid)`. The control target is `"{job_uuid}{start_url}"` — must match spider's `target_id()` format exactly.

**Fallback:** If the 30s drain timeout expires or spider errors during shutdown, the cancel path falls back to the original hard-cancel behavior (no partial results saved).

### readability: false (DO NOT CHANGE)
`build_transform_config()` in `crates/core/content.rs` sets `readability: false`. Changing to `true` causes Mozilla Readability to score VitePress/sidebar docs as low-quality and strip them to just the title — produces ~97% thin pages on most doc sites. `main_content: true` handles structural extraction without the scoring penalty.

### Sitemap Backfill
`append_sitemap_backfill()` runs after the main crawl, discovers URLs from sitemap.xml that the crawler missed, and fetches them individually. Respects `--max-sitemaps` (default 512) and `--include-subdomains`. Safe to skip if `--discover-sitemaps false`.

Use `--sitemap-since-days N` to restrict backfill to URLs whose `<lastmod>` falls within the last N days. Implemented via `extract_loc_with_lastmod()` in `content.rs` which parses `<url>` blocks and extracts `<loc>` + `<lastmod>` pairs. URLs without `<lastmod>` are always included (unknown age = don't filter). Date filtering is skipped entirely for sitemap index entries (`<sitemapindex>`) since those point to child sitemaps, not pages.

### Locale Path Filtering
`--exclude-path-prefix` (and the built-in locale list) treats `/` and `-` as word boundaries. `/ja` blocks both `/ja/docs` and `/ja-jp/docs`. Pass `none` to disable all locale filtering.

## Testing

```bash
cargo test engine         # crawl engine tests (8): auto-switch, sitemap, thin detection
cargo test -- --nocapture # show spider log output during test crawls
```

Engine tests use a live HTTP server (via `httpmock`) — no Docker services required.

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| 97%+ thin pages | `readability: true` stripping docs | Verify `readability: false` in `content.rs` |
| Thin pages on known-good site | JS-rendered SPA | Use `--render-mode chrome` |
| Chrome fallback not triggering | Chrome not reachable | Check `AXON_CHROME_REMOTE_URL`; verify `axon-chrome` is up |
| Crawl stops at first level | `--max-depth 0` set accidentally | Default is 5; check CLI args |
| Crawling other subdomains instead of target host | `--include-subdomains true` enabled | Default is now `false`; only use `--include-subdomains true` when you intentionally want all `*.parent.com` |
| Locale pages being crawled | Default locale filter only blocks known prefixes | Pass `--exclude-path-prefix none` to disable, or add custom prefixes |

## Thin Page Lifecycle
```
fetch page → content.rs transform → check len < min_markdown_chars
    → thin: skip (if --drop-thin-markdown true, default)
    → ok: save to disk + enqueue embed
```
Chrome auto-switch monitors thin rate across the crawl batch, not per-page.
