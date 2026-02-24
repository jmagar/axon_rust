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

### ✅ FIX-05 — Replace `#[allow(clippy::...)]` with `#[expect(clippy::...)]` (DONE)

**Chapter:** 2 — Clippy and Linting
**Files:** See table below
**Resolution:** The `module_inception` lint on `crates/crawl.rs` was unnecessary (the lint doesn't fire since module `crawl` contains `engine`, not `crawl`) — attribute removed entirely. The `too_many_arguments` suppressions in `streaming.rs`, `evaluate.rs`, and `collector.rs` were already resolved by the FIX-07 parameter struct refactoring.

---

### ✅ FIX-06 — Fix `clone()` in reddit comment tree traversal (DONE)

**Chapter:** 1 — Coding Styles / Ch 3 — Performance
**File:** `crates/ingest/reddit.rs`
**Resolution:** Refactored `CommentWithContext` to hold `body: String` instead of `data: serde_json::Value`, eliminating cloning of entire JSON subtrees. Changed `fetch_comments_recursive` → `collect_comments_recursive` (sync, takes `&mut Vec<CommentWithContext>`, borrows `parent_text: Option<&str>`). Combined with FIX-17.

---

### ✅ FIX-07 — Extract parameter structs for 10-arg functions (DONE — pre-existing)

**Chapter:** 2 — Clippy / Ch 6 — Generics
**Files:** `crates/vector/ops/commands/streaming.rs`, `crates/vector/ops/commands/evaluate.rs`
**Resolution:** Already implemented. `JudgeContext` struct exists at `streaming.rs:23-34`. The `too_many_arguments` suppressions were already removed.

---

### ✅ FIX-08 — Add `//!` module-level documentation (DONE)

**Chapter:** 8 — Comments vs Documentation
**Resolution:** Added `//!` module headers to `crates/core/http.rs`, `crates/vector/ops/qdrant/client.rs`, and `crates/jobs/common.rs` with key invariants and usage patterns documented.

---

### ✅ FIX-09 — Fix `unwrap_or(..).to_string()` allocation in facet hot path (DONE)

**Chapter:** 1 — Coding Styles / Ch 3 — Performance
**File:** `crates/vector/ops/qdrant/client.rs` — `qdrant_domain_facets`
**Resolution:** Changed `.unwrap_or("unknown").to_string()` to `.map_or_else(|| "unknown".to_string(), str::to_string)` to avoid redundant allocation in the default case.

---

### ✅ FIX-10 — Fix `next.unwrap()` after `is_none()` check in scroll loop (DONE)

**Chapter:** 1 — Coding Styles
**File:** `crates/vector/ops/qdrant/client.rs` — `scroll_pages_raw`
**Resolution:** Replaced `is_none()`/`unwrap()` pattern with `let Some(next) = ... .filter(|v| !v.is_null()) else { break };`.

---

## 🟢 Low Priority

### ✅ FIX-11 — `ssrf_blacklist_patterns()` — return static slice, not `Vec<String>` (DONE)

**Chapter:** 1 — Coding Styles / Ch 3 — Performance
**File:** `crates/core/http.rs`
**Resolution:** Changed return type from `Vec<String>` to `&'static [&'static str]`. Updated all callers (`engine.rs`, `scrape.rs`, `content.rs`, `http.rs` tests) from `.into_iter()` to `.iter().copied()`.

---

### ✅ FIX-12 — `clone()` on JSON points in `qdrant_retrieve_by_url` (DONE — documented)

**Chapter:** 1 — Coding Styles
**File:** `crates/vector/ops/qdrant/client.rs` — `qdrant_retrieve_by_url`
**Resolution:** The clone is unavoidable without changing `scroll_pages_raw`'s callback signature (callback receives `&[Value]`, can't take ownership). Added explanatory comment documenting why the clone is necessary.

---

### ✅ FIX-13 — Remove `test_` prefix from test function names (DONE)

**Chapter:** 5 — Automated Testing
**File:** `crates/core/http.rs`, `crates/ingest/reddit.rs`
**Resolution:** Removed `test_` prefix from ~30 test functions in `http.rs` and 4 test functions in `reddit.rs`.

---

### ✅ FIX-14 — Split `test_config_default_sensible_values` into per-field tests (DONE)

**Chapter:** 5 — Automated Testing
**File:** `crates/core/config/types.rs`
**Resolution:** Split into 6 focused tests: `config_default_crawl_settings`, `config_default_vector_settings`, `config_default_ask_settings`, `config_default_queue_settings`, `config_default_worker_settings`, `config_default_output_flags`. Also renamed other tests to remove `test_` prefix.

---

### ✅ FIX-15 — `mark_job_failed` should return `Result` (DONE)

**Chapter:** 4 — Error Handling
**File:** `crates/jobs/common.rs`
**Resolution:** Changed `mark_job_failed` to return `Result<()>` with `.with_context(...)`. Updated all 9 callers across `ingest.rs`, `embed/worker.rs`, `extract/worker.rs`, `crawl/runtime/worker/worker_loops.rs`, and `common/tests.rs` — fire-and-forget callers use `let _ = mark_job_failed(...).await;`, test callers use `.await?`.

---

### ✅ FIX-16 — Add `# Errors` doc section to `validate_url()` (DONE)

**Chapter:** 8 — Comments vs Documentation
**File:** `crates/core/http.rs`
**Resolution:** Added `# Errors` section documenting the conditions under which `validate_url()` returns `Err`.

---

### ✅ FIX-17 — `parent_text` clone in reddit recursive comment traversal (DONE)

**Chapter:** 1 — Coding Styles
**File:** `crates/ingest/reddit.rs`
**Resolution:** Combined with FIX-06. Changed `parent_text` parameter from `Option<String>` to `Option<&str>`, eliminating the clone on each recursive descent.

---

### ⏳ FIX-18 — Add `rstest` for parameterized URL validation tests (DEFERRED)

**Chapter:** 5 — Automated Testing
**File:** `crates/core/http.rs`
**Problem:** `validate_url_*` tests repeat the same structure 15+ times. This inflates test code and makes it easy to miss an edge case.

**Status:** Deferred — requires adding `rstest` as a dev-dependency. Low priority since the existing tests cover all cases correctly; the improvement is purely ergonomic.

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

### ✅ FIX-19 — `all_variants_are_distinct` — use HashSet, not nested loops (DONE)

**Chapter:** 5 — Automated Testing
**File:** `crates/jobs/status.rs`
**Resolution:** Replaced nested `for` loop with `HashSet`-based uniqueness check: collect all `as_str()` values into a `HashSet` and assert length equals 5.

---

## Summary Table

| ID | File | Chapter | Priority | Status |
|----|------|---------|----------|--------|
| FIX-01 | `Cargo.toml` | Ch2 Linting | 🔴 High | ✅ Done |
| FIX-02 | `vector/ops/qdrant/client.rs` | Ch4 Errors | 🟡 Medium | ✅ Done |
| FIX-03 | `core/http.rs` | Ch4 Errors | 🟡 Medium | ✅ Done |
| FIX-04 | `ingest/sessions/gemini.rs` | Ch4 Errors | 🟡 Medium | ✅ Done |
| FIX-05 | `streaming.rs`, `evaluate.rs`, `collector.rs`, `crawl.rs` | Ch2 Linting | 🟡 Medium | ✅ Done |
| FIX-06 | `ingest/reddit.rs` | Ch1 Idioms / Ch3 Perf | 🟡 Medium | ✅ Done |
| FIX-07 | `commands/streaming.rs`, `commands/evaluate.rs` | Ch2 / Ch6 Generics | 🟡 Medium | ✅ Done (pre-existing) |
| FIX-08 | `http.rs`, `qdrant/client.rs`, `jobs/common.rs` | Ch8 Docs | 🟡 Medium | ✅ Done |
| FIX-09 | `vector/ops/qdrant/client.rs` | Ch1 / Ch3 Perf | 🟡 Medium | ✅ Done |
| FIX-10 | `vector/ops/qdrant/client.rs` | Ch1 Idioms | 🟡 Medium | ✅ Done |
| FIX-11 | `core/http.rs` | Ch1 / Ch3 Perf | 🟢 Low | ✅ Done |
| FIX-12 | `vector/ops/qdrant/client.rs` | Ch1 Idioms | 🟢 Low | ✅ Documented |
| FIX-13 | `core/http.rs` | Ch5 Testing | 🟢 Low | ✅ Done |
| FIX-14 | `core/config/types.rs` | Ch5 Testing | 🟢 Low | ✅ Done |
| FIX-15 | `jobs/common.rs` | Ch4 Errors | 🟢 Low | ✅ Done |
| FIX-16 | `core/http.rs`, `qdrant/client.rs` | Ch8 Docs | 🟢 Low | ✅ Done |
| FIX-17 | `ingest/reddit.rs` | Ch1 Idioms | 🟢 Low | ✅ Done |
| FIX-18 | `core/http.rs` | Ch5 Testing | 🟢 Low | ⏳ Deferred |
| FIX-19 | `jobs/status.rs` | Ch5 Testing | 🟢 Low | ✅ Done |

**18/19 fixes complete. 1 deferred (FIX-18: `rstest` dependency).**

---

*Last updated: 2026-02-23 | Branch: `fix-crawl`*
