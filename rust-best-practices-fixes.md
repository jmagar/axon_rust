# Rust Best Practices — Fix Backlog

Generated from full codebase audit against [Apollo GraphQL Rust Best Practices](https://github.com/apollographql/rust-best-practices) (9 chapters).

**Overall grade: B+ (78/100)**

---

## Priority Legend

| Level | Description |
|-------|-------------|
| 🔴 High | Affects CI reliability, error transparency, or data correctness |
| 🟡 Medium | Inconsistency or hidden risk that will compound over time |
| 🟢 Low | Style/idiom improvements; safe to batch |

---

## 🔴 High Priority

### ✅ FIX-01 — Add `[lints]` section to Cargo.toml (DONE)

**Chapter:** 2 — Clippy and Linting
**File:** `Cargo.toml`
**Problem:** No workspace-level lint configuration. Every clippy run is a one-off; there is no shared baseline that CI enforces. New contributors get no lint enforcement until they run clippy manually.

**Fix:** Add a `[lints]` table after the `[dev-dependencies]` block:

```toml
[lints.rust]
unsafe_code = "deny"
unused_import_braces = "warn"
unused_qualifications = "warn"

[lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
# known suppressions (tracked below)
too_many_arguments = "allow"   # streaming/evaluate functions — see FIX-07
module_inception = "allow"     # crates/crawl.rs — see FIX-05
```

**Verification:** `cargo clippy --all-targets --all-features -- -D warnings` should pass clean.

---

## 🟡 Medium Priority

### ✅ FIX-02 — Replace `Box<dyn Error>` with `anyhow` in qdrant/client.rs (DONE)

**Chapter:** 4 — Error Handling
**File:** `crates/vector/ops/qdrant/client.rs`
**Problem:** All internal functions return `Result<_, Box<dyn Error>>` while the rest of the codebase uses `anyhow::Result`. This causes silent double-boxing when `?` coerces `Box<dyn Error>` into `anyhow::Error` at the call site, and prevents useful `context()`/`with_context()` chaining.

Current:
```rust
use std::error::Error;

async fn qdrant_delete_with_retry(...) -> Result<(), Box<dyn Error>>
async fn qdrant_scroll_pages_raw(...) -> Result<(), Box<dyn Error>>
pub async fn qdrant_retrieve_by_url(...) -> Result<Vec<QdrantPoint>, Box<dyn Error>>
// ... and all other functions in this file
```

**Fix:**
1. Remove `use std::error::Error;`
2. Add `use anyhow::{anyhow, Context, Result};`
3. Change all `Result<_, Box<dyn Error>>` → `Result<_>` (anyhow's `Result<T>` = `Result<T, anyhow::Error>`)
4. Replace `.into()` error conversions with `anyhow!(...)` or `.context(...)`

Example:
```rust
// before
return Err(format!("{context}: qdrant request failed with status {status}").into());

// after
return Err(anyhow!("{context}: qdrant request failed with status {status}"));
```

---

### ✅ FIX-03 — Replace `Box<dyn Error>` with `anyhow` in http.rs (DONE)

**Chapter:** 4 — Error Handling
**File:** `crates/core/http.rs:9`
**Problem:** `http_client()` is the only public function in `http.rs` that returns `Box<dyn Error>`. All callers use `anyhow`, so this is an inconsistency at a high-call-frequency boundary.

Current:
```rust
pub fn http_client() -> Result<&'static reqwest::Client, Box<dyn Error>> {
    HTTP_CLIENT
        .as_ref()
        .map_err(|err| format!("failed to initialize HTTP client: {err}").into())
}
```

**Fix:**
```rust
use anyhow::{anyhow, Result};

pub fn http_client() -> Result<&'static reqwest::Client> {
    HTTP_CLIENT
        .as_ref()
        .map_err(|err| anyhow!("failed to initialize HTTP client: {err}"))
}
```

---

### ✅ FIX-04 — Audit and fix `unwrap`/`expect` in `sessions/gemini.rs` (DONE)

**Chapter:** 4 — Error Handling
**File:** `crates/ingest/sessions/gemini.rs`
**Problem:** 17 `unwrap()`/`expect()` occurrences in a session parser. Session parsers receive external file input — invalid JSON structure or missing fields should surface as a `Result::Err` to the caller, not a panic. Silent panics here cause `axon sessions` to crash on any malformed export file.

**Fix:**
1. Read the file and identify every `unwrap()`/`expect()` callsite
2. Replace JSON field access panics with `ok_or_else(|| anyhow!("missing field X"))` or `.context("...")`
3. Propagate via `?` to the `ingest_gemini_session()` return type (should already be `anyhow::Result<usize>`)
4. Add a unit test with a truncated/malformed Gemini JSON fixture to verify graceful error return

---

### FIX-05 — Replace `#[allow(clippy::...)]` with `#[expect(clippy::...)]` (7 locations)

**Chapter:** 2 — Clippy and Linting
**Files:** See table below
**Problem:** `#[allow]` silently keeps suppressing a lint even after the root cause is fixed. `#[expect]` emits a warning if the suppression becomes unnecessary, keeping the allow-list honest.

| File | Line | Lint | Action |
|------|------|------|--------|
| `crates/crawl.rs` | 1 | `module_inception` | → `#[expect]` + reason comment |
| `crates/vector/ops/commands/streaming.rs` | 55 | `too_many_arguments` | → `#[expect]` + reason, or fix (see FIX-07) |
| `crates/vector/ops/commands/streaming.rs` | 246 | `too_many_arguments` | → `#[expect]` + reason, or fix |
| `crates/vector/ops/commands/streaming.rs` | 286 | `too_many_arguments` | → `#[expect]` + reason, or fix |
| `crates/vector/ops/commands/evaluate.rs` | 297 | `too_many_arguments` | → `#[expect]` + reason, or fix |
| `crates/vector/ops/commands/evaluate.rs` | 371 | `too_many_arguments` | → `#[expect]` + reason, or fix |
| `crates/crawl/engine/collector.rs` | 15 | `too_many_arguments` | → `#[expect]` + reason, or fix |

**Minimum fix** (30 min):
```rust
// before
#[allow(clippy::too_many_arguments)]

// after
#[expect(clippy::too_many_arguments, reason = "args form LLM judging context; tracked in FIX-07")]
```

---

### FIX-06 — Fix `clone()` in reddit comment tree traversal

**Chapter:** 1 — Coding Styles / Ch 3 — Performance
**File:** `crates/ingest/reddit.rs`
**Problem:** The comment tree builder clones the full `serde_json::Value` subtree on every iteration to pass ownership into the recursive call. For subreddits with deep comment threads this clones large JSON trees repeatedly.

Current pattern:
```rust
for child in children.as_array().unwrap_or(&vec![]) {
    let child = child.clone();   // ← clones entire JSON subtree
    // ... uses child by value
    build_comment_tree(child, ...)
}
```

**Fix:** Change `build_comment_tree` to accept `&Value` throughout and borrow rather than clone:
```rust
fn build_comment_tree(comment: &serde_json::Value, depth: usize, ...) -> Option<CommentWithContext>

// caller:
for child in children.as_array().unwrap_or(&[]) {
    build_comment_tree(child, depth + 1, ...);
}
```

If ownership is unavoidable (e.g., moved into a struct), drain the array by index or use `std::mem::take`.

---

### FIX-07 — Extract parameter structs for 10-arg functions in streaming.rs / evaluate.rs

**Chapter:** 2 — Clippy / Ch 6 — Generics
**Files:** `crates/vector/ops/commands/streaming.rs`, `crates/vector/ops/commands/evaluate.rs`
**Problem:** `judge_user_msg` (10 params), `ask_llm_streaming` / `ask_llm_non_streaming` (≥6 params each) suppress the `too_many_arguments` lint. These are not incidental — the parameters form a coherent unit of data that should be a struct.

**Fix for `judge_user_msg`:**
```rust
struct JudgeContext<'a> {
    query: &'a str,
    rag_answer: &'a str,
    baseline_answer: &'a str,
    reference_chunks: &'a str,
    rag_sources_list: &'a str,
    ref_quality_note: &'a str,
    rag_ms: u128,
    baseline_ms: u128,
    source_count: usize,
    context_chars: usize,
}

fn judge_user_msg(ctx: &JudgeContext<'_>) -> String { ... }
```

This also eliminates the `#[allow]` / `#[expect]` suppression for these functions.

---

### FIX-08 — Add `//!` module-level documentation (3 priority files)

**Chapter:** 8 — Comments vs Documentation
**Problem:** Only one file in the entire codebase has a `//!` module header (`ranking_test.rs`, which is a test file). Production modules have zero module-level docs. The three highest-value targets:

**`crates/core/http.rs`** — Add at top:
```rust
//! HTTP client and URL validation utilities.
//!
//! [`http_client()`] returns a shared [`reqwest::Client`] backed by a [`LazyLock`].
//! [`validate_url()`] enforces SSRF protection: private IP ranges, loopback, and
//! metadata endpoints are rejected. Note that this is a best-effort check — DNS
//! rebinding can still bypass it at request time (TOCTOU).
```

**`crates/vector/ops/qdrant/client.rs`** — Add at top:
```rust
//! Low-level Qdrant HTTP client operations.
//!
//! ## Key invariants
//! - Use [`qdrant_url_facets`] (O(1) `/facet` POST) for URL counting and aggregation.
//!   Never use full scroll for aggregation — it loads the entire collection into memory.
//! - [`ensure_collection`] issues GET-first, PUT only on 404. Safe to call on every embed.
//! - All delete operations use [`qdrant_delete_with_retry`] with exponential backoff.
```

**`crates/jobs/common.rs`** — Add at top:
```rust
//! Shared job infrastructure: pool creation, AMQP channel, job lifecycle helpers.
//!
//! ## Patterns
//! - Create [`PgPool`] once per worker via [`make_pool`]; pass as `&PgPool` everywhere.
//! - All AMQP work goes through [`open_amqp_channel`] (5 s timeout).
//! - Use [`claim_next_pending`] → [`mark_job_started`] → [`mark_job_completed`] /
//!   [`mark_job_failed`] — never write raw SQL job state updates.
//! - All internal channels are bounded (`channel(256)`); never use `unbounded_channel`.
```

---

### FIX-09 — Fix `unwrap_or(..).to_string()` allocation in facet hot path

**Chapter:** 1 — Coding Styles / Ch 3 — Performance
**File:** `crates/vector/ops/qdrant/client.rs` — `qdrant_domain_facets`
**Problem:** `.unwrap_or("unknown").to_string()` allocates a heap `String` even for the default case on every facet result. In a collection with 2M+ points this runs for every domain aggregation.

Current:
```rust
.unwrap_or("unknown").to_string()
```

**Fix:**
```rust
.map_or_else(|| "unknown".to_string(), str::to_string)
```

Or preferably, keep `"unknown"` as a named constant:
```rust
const UNKNOWN_DOMAIN: &str = "unknown";
// ...
.unwrap_or(UNKNOWN_DOMAIN).to_string()
```

---

### FIX-10 — Fix `next.unwrap()` after `is_none()` check in scroll loop

**Chapter:** 1 — Coding Styles
**File:** `crates/vector/ops/qdrant/client.rs` — `scroll_pages_raw`
**Problem:** The pattern checks `if next.is_none() { break; }` then immediately calls `next.unwrap()`. This is logically sound but non-idiomatic and misleads future readers into thinking the `unwrap` could actually panic.

Current:
```rust
if next.is_none() {
    break;
}
// ... several lines later ...
let offset = next.unwrap();
```

**Fix:**
```rust
let Some(offset) = next else { break };
```

---

## 🟢 Low Priority

### FIX-11 — `ssrf_blacklist_patterns()` — return static slice, not `Vec<String>`

**Chapter:** 1 — Coding Styles / Ch 3 — Performance
**File:** `crates/core/http.rs`
**Problem:** `ssrf_blacklist_patterns()` allocates and returns a `Vec<String>` on every call. This is constant data; it should be a static slice to avoid repeated allocation.

Current:
```rust
fn ssrf_blacklist_patterns() -> Vec<String> {
    vec!["169.254.".to_string(), "fd00:".to_string(), ...]
}
```

**Fix:**
```rust
fn ssrf_blacklist_patterns() -> &'static [&'static str] {
    &[
        "169.254.",   // AWS/Azure metadata
        "fd00:",      // IPv6 ULA
        // ...
    ]
}
```

Update callers accordingly (`.contains()` on `&[&str]` works identically).

---

### FIX-12 — `clone()` on JSON points in `qdrant_retrieve_by_url`

**Chapter:** 1 — Coding Styles
**File:** `crates/vector/ops/qdrant/client.rs` — `qdrant_retrieve_by_url`
**Problem:** `serde_json::from_value::<QdrantPoint>(p.clone())` clones the entire JSON object before deserialization. The clone is unnecessary because `from_value` consumes the value.

Current:
```rust
for p in &hits {
    if let Ok(point) = serde_json::from_value::<QdrantPoint>(p.clone()) {
```

**Fix:** Collect with ownership then iterate:
```rust
for p in hits {   // move out of vec, no clone needed
    if let Ok(point) = serde_json::from_value::<QdrantPoint>(p) {
```

---

### FIX-13 — Remove `test_` prefix from test function names in http.rs

**Chapter:** 5 — Automated Testing
**File:** `crates/core/http.rs`
**Problem:** Several test functions use a `test_` prefix. Ch5 recommends test names read as plain sentences — the `#[test]` attribute already identifies them as tests.

Examples to rename:
```rust
// before
fn test_validate_url_allows_public_https()
fn test_validate_url_rejects_private_ipv4()
fn test_normalize_url_adds_https_scheme_to_bare_host()

// after
fn validate_url_allows_public_https()
fn validate_url_rejects_private_ipv4()
fn normalize_url_adds_https_scheme_to_bare_host()
```

Search pattern: `grep -n "fn test_" crates/core/http.rs`

---

### FIX-14 — Split `test_config_default_sensible_values` into per-field tests

**Chapter:** 5 — Automated Testing
**File:** `crates/core/config/types.rs`
**Problem:** A single test asserts dozens of Config default values. When it fails the output is `assertion failed at config/types.rs:N` without telling you which field is wrong.

**Fix:** One test per logical group of defaults:
```rust
#[test]
fn config_default_render_mode_is_auto_switch() {
    assert_eq!(Config::default().render_mode, RenderMode::AutoSwitch);
}

#[test]
fn config_default_collection_is_cortex() {
    assert_eq!(Config::default().collection, "cortex");
}
// etc.
```

---

### FIX-15 — `mark_job_failed` should return `Result` instead of swallowing

**Chapter:** 4 — Error Handling
**File:** `crates/jobs/common.rs`
**Problem:** `mark_job_failed()` returns `()` and only logs the error. Callers cannot distinguish "successfully recorded failure" from "failed to record failure" — a secondary failure (DB down, pool exhausted) is silently dropped.

Current:
```rust
pub async fn mark_job_failed(pool: &PgPool, id: Uuid, error_text: &str) {
    if let Err(e) = sqlx::query(...).execute(pool).await {
        log_warn(&format!("mark_job_failed: {e}"));
    }
}
```

**Fix:**
```rust
pub async fn mark_job_failed(pool: &PgPool, id: Uuid, error_text: &str) -> anyhow::Result<()> {
    sqlx::query(...)
        .execute(pool)
        .await
        .with_context(|| format!("mark_job_failed for job {id}"))?;
    Ok(())
}
```

Callers that want the current log-and-continue behavior can use `.unwrap_or_else(|e| log_warn(...))`.

---

### FIX-16 — Add `# Errors` / `# Panics` doc sections to public functions

**Chapter:** 8 — Comments vs Documentation
**Files:** `crates/core/http.rs`, `crates/vector/ops/qdrant/client.rs`
**Problem:** Public functions returning `Result` or that can panic lack `# Errors` / `# Panics` doc sections, which rustdoc renders as a dedicated section in generated docs.

Example for `validate_url()`:
```rust
/// Validates a URL for safety and well-formedness.
///
/// Rejects private IP ranges, loopback addresses, and known metadata endpoints
/// as an SSRF mitigation. Note this is a best-effort check — DNS rebinding can
/// bypass it at request time.
///
/// # Errors
///
/// Returns `Err` if the URL is malformed, uses a non-HTTP(S) scheme, or resolves
/// to a blocked address range.
pub fn validate_url(url: &str) -> anyhow::Result<Url> { ... }
```

---

### FIX-17 — `parent_text` clone in reddit recursive comment traversal

**Chapter:** 1 — Coding Styles
**File:** `crates/ingest/reddit.rs`
**Problem:** `parent_text.clone()` is called on each recursive descent to pass `Option<String>` into child calls. This clones the parent comment text string for every child node in the tree.

**Fix:** Change the recursive parameter to `Option<&str>` with a lifetime:
```rust
// before
fn build_comment_tree(comment: Value, depth: usize, parent_text: Option<String>, ...)

// after
fn build_comment_tree<'a>(comment: &'a Value, depth: usize, parent_text: Option<&'a str>, ...)
```

This eliminates the clone entirely and avoids heap allocation on each recursive call.

---

### FIX-18 — Add `rstest` for parameterized URL validation tests

**Chapter:** 5 — Automated Testing
**File:** `crates/core/http.rs`
**Problem:** `validate_url_*` tests repeat the same structure 15+ times. This inflates test code and makes it easy to miss an edge case.

**Fix:** Add `rstest` to `[dev-dependencies]` and parameterize:
```toml
# Cargo.toml
[dev-dependencies]
rstest = "0.23"
```

```rust
use rstest::rstest;

#[rstest]
#[case("https://google.com/path", true)]
#[case("https://docs.rs/tokio", true)]
#[case("http://127.0.0.1/anything", false)]
#[case("http://169.254.169.254/latest/meta-data", false)]
#[case("http://[::1]/", false)]
#[case("ftp://example.com", false)]
fn validate_url_allows_public_rejects_private(#[case] url: &str, #[case] should_pass: bool) {
    assert_eq!(validate_url(url).is_ok(), should_pass, "url: {url}");
}
```

---

### FIX-19 — `all_variants_are_distinct` — use iterator, not nested loops

**Chapter:** 5 — Automated Testing
**File:** `crates/jobs/status.rs`
**Problem:** The test uses nested `for` loops with `assert_eq!`/`assert_ne!` inside, which is multiple assertions per test and harder to read than an iterator comparison.

Current:
```rust
for (i, a) in statuses.iter().enumerate() {
    for (j, b) in statuses.iter().enumerate() {
        if i == j { assert_eq!(a, b); } else { assert_ne!(a, b); }
    }
}
```

**Fix:** This is already well-proven by `as_str_returns_expected_values` (distinct strings → distinct variants). Consider removing `all_variants_are_distinct` entirely as redundant, or collapsing it to:
```rust
#[test]
fn job_status_variants_have_unique_string_representations() {
    let strings: std::collections::HashSet<_> = [
        JobStatus::Pending, JobStatus::Running, JobStatus::Completed,
        JobStatus::Failed, JobStatus::Canceled,
    ].iter().map(JobStatus::as_str).collect();
    assert_eq!(strings.len(), 5);
}
```

---

## Summary Table

| ID | File | Chapter | Priority | Effort |
|----|------|---------|----------|--------|
| FIX-01 | `Cargo.toml` | Ch2 Linting | 🔴 High | 30 min |
| FIX-02 | `vector/ops/qdrant/client.rs` | Ch4 Errors | 🟡 Medium | 1 h |
| FIX-03 | `core/http.rs` | Ch4 Errors | 🟡 Medium | 15 min |
| FIX-04 | `ingest/sessions/gemini.rs` | Ch4 Errors | 🟡 Medium | 1 h |
| FIX-05 | `streaming.rs`, `evaluate.rs`, `collector.rs`, `crawl.rs` | Ch2 Linting | 🟡 Medium | 30 min |
| FIX-06 | `ingest/reddit.rs` | Ch1 Idioms / Ch3 Perf | 🟡 Medium | 1 h |
| FIX-07 | `commands/streaming.rs`, `commands/evaluate.rs` | Ch2 / Ch6 Generics | 🟡 Medium | 1 h |
| FIX-08 | `http.rs`, `qdrant/client.rs`, `jobs/common.rs` | Ch8 Docs | 🟡 Medium | 45 min |
| FIX-09 | `vector/ops/qdrant/client.rs` | Ch1 / Ch3 Perf | 🟡 Medium | 15 min |
| FIX-10 | `vector/ops/qdrant/client.rs` | Ch1 Idioms | 🟡 Medium | 10 min |
| FIX-11 | `core/http.rs` | Ch1 / Ch3 Perf | 🟢 Low | 20 min |
| FIX-12 | `vector/ops/qdrant/client.rs` | Ch1 Idioms | 🟢 Low | 10 min |
| FIX-13 | `core/http.rs` | Ch5 Testing | 🟢 Low | 15 min |
| FIX-14 | `core/config/types.rs` | Ch5 Testing | 🟢 Low | 30 min |
| FIX-15 | `jobs/common.rs` | Ch4 Errors | 🟢 Low | 30 min |
| FIX-16 | `core/http.rs`, `qdrant/client.rs` | Ch8 Docs | 🟢 Low | 30 min |
| FIX-17 | `ingest/reddit.rs` | Ch1 Idioms | 🟢 Low | 30 min |
| FIX-18 | `core/http.rs` | Ch5 Testing | 🟢 Low | 30 min |
| FIX-19 | `jobs/status.rs` | Ch5 Testing | 🟢 Low | 10 min |

**Total estimated effort: ~10 hours**

---

*Last updated: 2026-02-23 | Branch: `fix-crawl`*
