---
title: "Backend performance review remediation"
created: 2026-09-08
updated: 2026-09-08
---

# Backend performance review remediation

## Scope and outcome

All **22 canonical findings surfaced by the performance review** have code
changes and passing regression coverage. The combined affected-crate test run
passed **2,866 tests**, with **six existing ignored tests**. No reported finding
was waived or deferred. Additional defects found during validation were also
resolved, as described below.

This is a remediation report, **not an assertion that the original full review
completed**. Its frozen snapshot contained 2,739 checksummed files at commit
`04ec29055338212841afd21158ede9019cb7be25` plus captured dirty changes, with scope
SHA-256 `58438e206c87edfe0164af4f55c6dca9fd58342b5812e4e42475b6b6192befd2`.
Checksum coverage is not semantic review coverage. A usage-limited reviewer and
unfinished review assignments prevented an exhaustive review. The user then
authorized remediation of every surfaced issue, superseding that review run.

Phase one had 19 raw IDs, deduplicated to 18 canonical findings. Two performance
and two security findings from phase two brought the total to 22. All were P2.
`ARC-A02-001` remains reconciled as a duplicate of `ARC-A01-001`, not a discarded
finding. Raw reports remain unchanged under `.full-review/raw/`.

Tracking: Beads epic `axon_rust-mxlpo`, remediation children `.1` through `.7`,
and combined-validation follow-up `.8`.

## Finding dispositions

Every row below is implemented and regression-tested. Test names identify
sidecar regressions within the named crate; the combined gate compiled and ran
them together.

| ID | Resolution | Representative regression evidence |
| --- | --- | --- |
| QUA-A01-001 | Reuse document-wide source-position indexing; compute the HTML source range once. | `axon-document`: `markdown_source_positions_do_not_rescan_every_section_prefix`, `html_source_positions_are_computed_once_for_all_chunks`. |
| QUA-A01-002 | Copy only the selected oversized-section window, preserving metadata and UTF-8 boundaries. | `axon-document`: `ranged_window_allocates_only_the_window_not_the_original_section` and complete 122-test suite. |
| QUA-A01-003 | Reuse a provider HTTP client and connection pool; keep redirect history request-local and enforce guarded destinations. | `axon-adapters` real TCP connection-reuse and concurrent redirect-isolation regressions. |
| QUA-A01-004 | Return long Retry-After waits as provider cooling; reject unrepresentable retry dates; release permits before short backoff. | `axon-embedding` long Retry-After, overflow, and bounded retry regressions. |
| QUA-A02-001 | Cancel uncommittable work after definitive lease loss, then settle cleanup without canceling the parent job token. | `axon-services`: `definitive_lease_loss_cancels_work_and_allows_its_cleanup_to_finish`, plus generation-level lease-loss coverage. |
| QUA-A02-002 | Release the source lease even if writing the failure summary fails; preserve both errors. | `axon-services`: `failed_summary_write_still_releases_source_lease`. |
| QUA-A03-001 | Make pool waiter/spawn accounting cancellation-safe with owned guards. | `axon-llm`: `canceled_initialization_releases_spawning_accounting`, `canceled_checkout_releases_waiter_admission`. |
| QUA-A03-002 | Reap idle children without another checkout; weak ownership lets retired pool reapers exit. | `axon-llm`: `unused_empty_pool_is_evicted_after_idle_interval`, `idle_expiry_reclaims_child_without_another_checkout`. |
| QUA-A03-003 | End SSE consumption at the definitive DONE marker rather than HTTP EOF. | `axon-llm`: `definitive_done_returns_without_waiting_for_http_eof`; existing usage-tail coverage remains green. |
| ARC-A01-001 | Deliver completed web items without ordinal head-of-line blocking; explicitly track finality; overlap bounded acquisition waves with preparation. | `axon-adapters` slow-first/fast-second HTTP regression; `axon-services` controlled first-wave barrier and existing streaming/cancellation tests. Duplicate `ARC-A02-001` covered here. |
| ARC-A01-002 | Share request and weighted-input admission by normalized endpoint, not tuning profile. | `axon-embedding`: `logical_jobs_share_authoritative_http_request_admission`, both `mixed_profiles_share_the_endpoint_*` tests, endpoint normalization/lifetime tests. |
| ARC-A01-003 | Apply point-count and encoded-byte bounds to carry-forward Qdrant publication with lazy batch encoding. | `axon-vectors`: `carry_forward_splits_requests_by_encoded_bytes_not_only_point_count`, `carry_forward_rejects_one_oversized_point_before_sending`. |
| ARC-A02-002 | Enforce a lifetime generation side-effect budget even with disk spilling and fallback. | `axon-services`: `total_generation_side_effects_are_bounded_with_and_without_spill`, `ambiguous_spool_failure_replays_exactly_once_and_preserves_the_budget`. |
| ARC-A02-003 | Move spool serialization, append, and replay off async runtime workers while retaining cancellation ownership. | `axon-services`: `blocking_spool_append_does_not_stall_the_runtime`; spill/fallback tests. |
| ARC-A02-004 | Make fresh queued-reservation observations read-only; conditionally coalesce liveness writes. | `axon-jobs`: `fresh_queue_observation_does_not_need_the_sqlite_writer`, `queue_liveness_renewal_is_rate_limited_across_observers`, scheduler lifecycle tests. |
| ARC-A03-001 | Preserve interactive priority at execution admission with bounded bursts, FIFO ordering within lanes, and cancellation reclamation. | `axon-llm` priority/fairness tests and `canceling_queued_and_granted_waiters_reclaims_execution_capacity`. |
| ARC-A03-002 | Apply the completion deadline to reservation and execution waiting as well as backend work. | `axon-llm`: `nonstreaming_completion_deadline_includes_admission`, `streaming_completion_deadline_includes_admission`. |
| ARC-A03-003 | Resolve backend defaults before explicit configuration, preserving an explicit OpenAI concurrency value of four. | `axon-core`: `completion_concurrency_defaults_and_explicit_values_resolve_before_dispatch`, explicit-four and explicit-value tests. |
| PER-A02-001 | Project terminal warnings outside the writer in bounded pages; hash-deduplicate; fence concurrent event/attempt changes. | `axon-jobs`: `terminal_warning_preparation_releases_writer_and_fences_concurrent_events`, `terminal_warnings_page_and_deduplicate_across_attempts`. |
| PER-A02-002 | Bound evidence materialization by rows, charged bytes, and serialized bytes; use explicit topology-only fallback or typed detail errors. | `axon-graph`: `evidence_heavy_graph_reads_are_bounded_and_explicit`; all 95 graph tests. |
| SEC-A03-001 | Bound OpenAI success bodies/SSE and Codex protocol input before unbounded accumulation. | `axon-llm`: `oversized_success_body_is_rejected_before_json_deserialization`, `oversized_unterminated_sse_frame_is_rejected_by_byte_limit`, Codex protocol budget tests. |
| SEC-A03-002 | Retain a process-group cleanup guard throughout Codex initialization and active turns. | `axon-llm`: `canceled_codex_initialization_reaps_descendants`, `canceled_codex_turn_reaps_descendants`, timeout/process-group tests. |

## What the measurements prove

TDD reproduced behavioral failures before production changes. Examples include:

- Source-position instrumentation recorded 13,299,700 rescanned prefix bytes for
  a 13,390-character fixture before indexing was reused. The new bounded-scan
  regressions pass without a wall-clock threshold.
- A small window from a roughly 650 KiB section previously allocated 650,839
  bytes. The allocation-budget regression now rejects that full-body-copy pattern.
- Thirty-two fresh queued-reservation observations previously made 32 writes
  and blocked behind a held SQLite writer. They now require zero renewal writes;
  an aged reservation renews once under concurrent observation.
- Mixed TEI profiles previously admitted a third request beyond a shared limit
  of two. A real TCP response-release barrier now proves that the third request
  waits for endpoint capacity, independent of response timing.
- Carry-forward tests split three roughly 6 MiB points into bounded requests
  and reject a roughly 17 MiB individual point before sending it.
- Single-thread Tokio tests verify that a heartbeat progresses while spool
  append is deliberately blocked. Warning projection similarly allows a
  concurrent writer and includes its late event after fencing.

These are structural, concurrency, and allocation measurements—not a new live
crawl benchmark. No claim is made that the production crawl now takes a specific
number of seconds or that all performance headroom is exhausted.

## Additional validation defects resolved

1. Thin-refetch recovery could truncate a later manifest append. Recovery now
   requires the exact recorded length and ownership evidence before rollback.
   Mismatches preserve the manifest and recovery journal.
2. Local acquisition now validates item keys before immutable-spool lookup,
   preserving the containment error for escape attempts. The symlink-swap test
   now verifies the intended immutable discovery snapshot rather than expecting
   acquisition to reopen the swapped live source directory.
3. The directory-exchange maintenance test counts implementation points, not
   call sites; a compensating call no longer creates a false failure.
4. REST contract expectations now include existing agent/loadout request fields.
   The prune fixture explicitly mocks both collection schema and exact count,
   avoiding dependency on live Qdrant state.
5. Reset resume compared redacted saved locations with unredacted fresh rows.
   Physical-inventory checks now compare the same redacted representation.
   Configuration identity independently binds endpoints and filesystem roots,
   including the artifact root. Existing plans with the older configuration
   checksum require fresh review rather than bypassing configuration fencing.
6. Cleanup fault-injection tests now share retry-registry serialization, and a
   deliberately retained panic-recovery journal has its own temporary root.
   Unrelated runtime tests can no longer replay that fixture against another
   fake ledger.
7. TEI admission tests use explicit response release rather than delayed mocks.
   Codex success tests allow realistic subprocess startup under concurrent test
   load; dedicated one-second timeout regressions remain in place.
8. Repo-wide documentation checks exclude both immutable review-archive naming
   forms. Three focused link-checker tests pass, including a regression that
   first reproduced both archive false positives. The directory-style archive
   is also gitignored. No frozen snapshot was edited or deleted.

## Validation

Combined gate:

```sh
RUST_MIN_STACK=8388608 cargo test --offline --locked --no-fail-fast -q --lib \
  -p axon-services -p axon-adapters -p axon-jobs -p axon-graph \
  -p axon-llm -p axon-document -p axon-embedding -p axon-vectors
```

| Crate | Passed | Ignored |
| --- | ---: | ---: |
| axon-adapters | 847 | 3 |
| axon-document | 122 | 0 |
| axon-embedding | 84 | 1 |
| axon-graph | 95 | 0 |
| axon-jobs | 256 | 0 |
| axon-llm | 165 | 0 |
| axon-services | 1,061 | 0 |
| axon-vectors | 236 | 2 |
| **Total** | **2,866** | **6** |

The eight-crate `cargo clippy --offline --locked --all-targets ... -- -D warnings`
gate passed. Formatter verification passed. Selected changed production modules
passed monolith hard limits; warning-only functions remain below 120 lines.
The macOS services test binary emits the existing linker warning about a large
`__eh_frame` section; this did not prevent linking or execution.

Additional gates passed:

- Four `axon-core` completion-concurrency tests.
- Three `xtask` repo-wide documentation-link tests.
- `cargo clippy --offline --locked -p xtask --all-targets -- -D warnings`.
- `cargo xtask check-layering`.
- `cargo xtask generated-contracts refresh`, followed by a successful
  `cargo xtask generated-contracts check`: eight presentation artifacts current,
  generated docs/provenance current, 541 Markdown files without broken links,
  128 contract-scanned Markdown files without removed-surface references, and
  all 111 required documentation files present.
- `cargo fmt --all -- --check` and `git diff --check`.

## Operational implications and limits

See [Pipeline performance and resource boundaries](../guides/pipeline-performance-boundaries.md)
for exact limits and behavior changes. In particular:

- The generation fix chooses an explicit 256 MiB charged side-effect budget,
  not an unlimited streaming final archive and not an exact RSS cap.
- Oversized graph evidence produces an explicit summary/error boundary, not
  a new evidence-pagination API.
- The first live TEI endpoint owner establishes shared limits; raising a
  profile does not change an already-live endpoint budget.
- Larger supported workloads may now fail explicitly at resource limits rather
  than consuming unbounded memory. This is intentional and documented.

The initial remediation was committed and pushed as `07d1761b1` on PR #607.
No daemon restart, production deployment, ingestion-queue mutation, or live crawl
benchmark was performed. The follow-up review status below is separate from the
original 22-finding remediation and its test counts.

## PR #607 follow-up review

Lavra reviewed the 235-file diff from `9152ebe94` to `07d1761b1` across seven
perspectives: architecture, performance, security, simplicity, data migration,
deployment verification, and migration drift. This is a scoped review, not a
claim of line-by-line semantic coverage of every changed file.

| Tracking | Finding | Remediation evidence |
| --- | --- | --- |
| `axon_rust-axvtw.3` (P2) | Canceled grants and expired queue heads delayed eligible successors until recovery. | Mutation-side transactional settlement/dispatch; two real SQLite tests failed before the fix, then passed. All 55 scheduler tests passed. |
| `axon_rust-s4y64` (P2, pre-existing) | Successful TEI bodies were unbounded before JSON decoding. | Streaming byte caps, bounded batch splitting, explicit singleton failure; four response regressions and the 88-test embedding suite passed, with one existing ignored test. |
| `axon_rust-axvtw.4` (P3) | MCP runtime-plan assertions did not exercise actual context wiring. | Removed disconnected plan; actual transport/context identity tests pass and detect a deliberate duplicate-context mutation. |
| `axon_rust-axvtw.5` (P3) | Early writer unlock required unnecessary holder-generation bookkeeping. | Clear diagnostics before unlocking; removed IDs and optional guard. Real drop/reacquire regressions pass. |
| `axon_rust-n1k01` (P1, pre-existing) | Pre-push shell wrapper could mask an earlier failed check. | Fail-fast shell execution; 16 controlled failure cases cover both pre-commit and pre-push branches. |
| `axon_rust-axvtw.6` (P1) | Required repository gate targeted an unavailable runner label. | Same immutable fleet validator and Rust profile on hosted runners; all 75 workflow tests passed. Missing guide frontmatter corrected; unchanged validator passes. |
| `axon_rust-axvtw.7` (P2) | Operational guidance used removed tuning keys and a nonexistent `--local` flag. | Current configuration ownership and isolated-process instructions documented; workflow documentation regression passed. |
| `axon_rust-axvtw.8` (P2) | Manual 301/302 redirects rewrote methods other than POST. | Status-specific method/body handling; real-wire 30-case matrix, all 17 fetch tests, and independent affected-code re-review passed. |

Performance, data-migration, deployment, and drift passes found no additional
confirmed code defects. All 152 frozen provenance records and six JSON fixture
families matched. The outer target-runtime gate-topology concern was inspected
and discarded as unconfirmed: it predated this diff, and no bounded failure was
established. Deployment remains unqualified until exact-artifact CI, a recovery
baseline, and a deployed source/retrieval canary are verified.

An initial embedding-suite run observed an existing request-count test retry
(two requests instead of one); its isolated run and full-suite rerun passed.
This is recorded rather than presenting the first run as green. Another 105
targeted/repeated checks passed without recurrence; the original retry trigger
remains uncertain. No speculative production or test-policy change was made.

Fresh CI also exposed fixture/build issues tracked under `axon_rust-axvtw.2`.
The hermetic security bind probe reused a live worker's database; isolating its
owned database restored the full composed CLI/HTTP/MCP replay with zero residual
resources, without weakening the authentication assertion. Windows directory
identity now uses a retained stable handle: Windows-target test compilation
passes, and the platform workflow now executes its native regression tests.
Schema fixture closure, isolated database fixtures, scheduler readiness, worker
lock ownership, and stale assertions were corrected. All 84 schema tests, two
generated-contract tests, and the targeted fixture regressions passed locally.
The palette test now kills and reaps its direct child rather than a shell;
its exact nextest selector passes without a leak. Native Windows execution and
fresh Linux workspace qualification still require final-head CI.

The separate PR Review Toolkit completed all seven aspects: code, types, tests,
silent failures, simplification, comments, and documentation/configuration. Its
two confirmed findings are `.7` and `.8` above; both are implemented and tested.
No additional actionable finding remained in the affected-code re-review.
The final quick-push patch version is 7.3.3. Local Cargo checking and generated
contract refresh/check passed at that version. These focused follow-up results
do not relabel the earlier broad-suite counts as a fresh full-workspace run.

### CI qualification of `5b88f9291`

Linux nextest passed all 6,081 tests (eight skipped) with no leak reports.
Repository Contract, Compose, Clippy, and all three platform smoke jobs passed.
The Windows native identity tests also passed (run `34292353463`, completed
2026-09-09 at 00:11 UTC), qualifying the stable-handle fix on Windows itself.

Two failures prevented merge. The MCP structural checker still required the
old direct stdio call spelling after the shared-context callback refactor;
`axon_rust-axvtw.2.3` fixes that matcher while retaining all transport checks.
Its actual-source regression failed before the change, then all six checker
tests and an independent affected review passed.

The hermetic security domain failed with an `unknown` diagnostic. The parser
discarded qualified exception names; only hashes of the underlying output
were retained, so this evidence does not establish the security failure's
cause. The direct composed replay passed locally. A native macOS run also
encountered sandbox-related failures and is not a substitute for Linux CI
qualification. Safe exception/return-code/source-location instrumentation
(`axon_rust-axvtw.2.4`) now makes the next CI failure actionable without retaining
messages, arguments, local variables, credentials, or private absolute paths.
Its real subprocess-to-report regression failed before the fix and passed
afterward, including the exact CI report verifier. All 21 workflow tests passed;
adversarial cases reject unapproved fields, private paths and wrong field types.
Independent safety review found no remaining actionable issue in the diagnostic
change. This instrumentation does not claim to repair the underlying security
failure.
