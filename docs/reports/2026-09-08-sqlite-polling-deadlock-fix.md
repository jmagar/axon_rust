# SQLite writer polling deadlock remediation

## Root cause

The shared SQLite writer gate exposed circular waits in the futures driving
source ingestion. Restarting released the gate but did not correct those waits.
Four regression tests reproduced the following failures before their fixes:

1. Source heartbeat handling awaited its database write inside a `select!`
   branch, stopping the source future that could release the writer.
2. Buffered web acquisition stopped polling sibling provider futures while
   reporting progress or delivering a page downstream. A paused sibling could
   own the writer or be the next admitted waiter.
3. Buffered embedding similarly stopped polling sibling calls while publishing
   another result or checkpointing its document state.
4. Provider lease renewal awaited its database write inside a `select!` branch,
   stopping the provider operation that could release the writer.

## Changes

- Poll source work and its heartbeat loop concurrently. Drop the heartbeat
  future before cancellation cleanup so it cannot retain the cleanup writer.
- Run bounded web provider calls in abort-on-drop tasks, retaining the streaming
  path's ordered results and the non-streaming path's unordered results.
- Run embedding provider calls in abort-on-drop tasks so publication cannot
  suspend them while waiting for their writer.
- Poll lease renewal and the provider operation concurrently. Drop both futures
  before awaiting failure release or completion bookkeeping.

Cancellation tests verify writer release; buffering remains bounded. These are
code fixes, not increased SQLite busy timeouts or repeated restart workarounds.

## Verification

- Jobs library: **252 passed**.
- Web adapters: **67 passed**.
- Vectorization pipeline: **11 passed**.
- Source runner: **10 passed**.
- Clippy for jobs, adapters, and services, all targets with warnings denied:
  passed.
- Locked release build: passed; `git diff --check`: passed.

## Deployment and live evidence

The full patch was deployed on 2026-09-08 at **03:32:03 UTC**, with launchd
running PID **91941**. Binary SHA-256:

`089ed86f9f067c5cba7d0bb4a65d4039a2d63c4ea21c4ec1980783a0ce8fa501`

The supported recovery API requeued the interrupted Swoosh attempt once.
Without another restart, the existing queue completed:

| Source | Documents | Chunks |
| --- | ---: | ---: |
| Qwen3-Embedding-0.6B model documentation | 5 | 73 |
| TEI documentation | 2 | 6 |
| jless | 3 | 46 |

Committed generation pointers were verified for these jobs. The jq manual job
then progressed from 1 document / 14 chunks to 5 / 167 and 6 / 320 by
03:36:28 UTC, across multiple heartbeat and lease-renewal intervals.
The timestamped Axon log contained **zero** SQLite writer-admission, database
locked, or pool-timeout warnings since deployment at that observation.

The jq job subsequently failed at 03:36:44 UTC. Its durable event 82 records
`vector.qdrant.status`: Qdrant returned HTTP **408** during upsert; producer
cancellation then settled. This was not another SQLite wedge: the same daemon
claimed the next queued job, `https://just.systems/man/en`, and reached
vectorization by 03:37:27 UTC. The Qdrant timeout remains a separate unresolved
provider failure; the jq crawl is not certified as ingested.

At 03:38:23 UTC, the just manual crawl had progressed to 11 documents and
164 prepared chunks. At 03:38:40 UTC, the same process still had zero SQLite
lock warnings since deployment. A read-only Qdrant health check returned
HTTP 200 in 1.84 seconds; readiness does not establish that the failed upsert
would now succeed.

Saving the final verification to Bead `axon_rust-ji14f` failed twice because
the issue tracker's Dolt endpoint was unreachable. The bead remains open; this
report preserves the evidence until its tracking state can be reconciled.

This verifies recovery of live ingestion, not completion or full certification
of the documentation inventory. Existing failed jobs and uncrawled scopes still
require their separate investigation and retrieval/synthesis certification.
No commit or push was performed for this remediation.

## Follow-up at 03:50 UTC

The HTTP jobs API showed the queue continuing through additional jobs. The
just manual job failed with `embedding.tei.transport`: TEI transport timeout,
followed by orderly producer cancellation. This is an additional unresolved
provider failure, not successful ingestion. A subsequent job completed with
3 documents / 60 chunks, and the next running job reached 40 documents / 110
chunks before continuing normalization.

Fourteen writer-admission warnings appeared around 03:43:14 UTC (approximately
one-second waits). Therefore the earlier zero-warning observation is only valid
for its stated time window. Subsequent durable progress at 03:49:35 and
03:50:01 UTC demonstrates that this contention did not permanently wedge the
queue. No restart or recovery was performed during this follow-up, and no
documentation inventory checkboxes were certified from these observations.

## Follow-up at 04:12 UTC

Kotlin reached 131 documents but failed at 04:10:18 UTC with
`vector.qdrant.transport`: upsert transport timeout. The next queued source,
Lefthook, was already upserting by 04:11:31. One additional approximately
one-second SQLite writer-admission warning occurred at 04:06:57; continued
progress does not indicate a permanent SQLite wedge.

Read-only inspection on tootie confirmed both provider containers remained up
for four days, with Qdrant reporting healthy. Qdrant access logs near the Kotlin
failure show successful HTTP 200 upserts taking **67.76, 65.96, and 50.92
seconds**, followed by count/delete requests during failed-generation cleanup.
Subsequent Lefthook upserts took approximately 0.7–3.5 seconds. This establishes
large provider latency spikes during the timeout window, but does not yet
identify whether upload, storage, or another resource caused those spikes.
No provider restart, configuration change, retry, or certification was performed.

## Provider-timeout root cause and patch, 04:38 UTC

Read-only controlled requests isolated slow network transport from Qdrant
processing. A 1 MiB JSON body sent to the count endpoint took 7.91–12.10 seconds
from the Mac through the configured LAN address, versus 1.68–3.31 milliseconds
from tootie itself. Qdrant reported processing below 2 milliseconds. The Mac's
LAN-address route uses Tailscale through the squirts subnet router. Testing
tootie's direct Tailscale address still took 10.89–18.49 seconds, so changing
the address was not a demonstrated remedy. Without configuration changes,
later Mac samples improved to 1.15–1.32 seconds and peer latency dropped from
approximately one second to 75 milliseconds. The evidence establishes a
variable network bottleneck, not sustained Qdrant compute saturation.

Two reproduced code defects made these transient conditions fatal prematurely:

- Native TEI retried errors obtaining response headers, but returned immediately
  on a timeout while reading the successful response body. Body I/O now lives
  inside the existing bounded attempt/retry boundary. Invalid JSON still fails
  immediately, and request/input permits are released before retry backoff.
- Qdrant PUT retries covered transport errors, 429, and 5xx, but not HTTP 408.
  Replayable PUTs now retry 408 using the unchanged attempt limit/backoff.

Both regression tests failed before their fixes and passed after. Additional
tests cover permanent 408 exhaustion and fail-fast malformed TEI JSON.
Validation: **79 embedding tests passed / 1 ignored**, **234 vector tests
passed / 2 ignored**, Clippy all-targets with warnings denied passed, and the
locked release build passed. No timeout limits, crawl settings, VPN settings,
provider addresses, or remote provider configuration were changed.

Deployed at **04:38:27 UTC**, PID **15478**, binary SHA-256:

`abacd7f83afcbf3c2e212bf2a160736e222698deae03d044e108db21aa23fbec`

One newly started interrupted crawl was recovered through the supported API.
jq, just, and Kotlin were requeued through same-config retries as attempt 2,
without overrides. Their retained counts do not constitute progress on the
new attempts. Maven ingestion advanced on the new daemon. Eight approximately
one-second writer-admission warnings occurred during startup/recovery, followed
by progress; this is not a zero-contention claim.

The original SQLite issue is closed on sustained live progress evidence.
Provider issue **axon_rust-yt8ya remains open** pending end-to-end verification
of the original failed sources. The thread follow-up now explicitly continues
root-cause remediation of recurrent provider failures instead of only reporting
them. The network itself has not been repaired or claimed stable; the code
fixes restore the intended retry behavior. No commit or push was performed.

## Long-crawl source lease expiry, 06:33 UTC

OpenTelemetry job `6daf6031-b8e4-44a8-baca-f1688cc84b7d` ran from
05:59:17 to 06:33:33 UTC and then failed with `source.index_failed`:
`source refresh lost lease before publish`. Its event projection reached
1,514 documents and 65,886 chunks, but generation `gen_1` remained failed,
in `writing` state, with no publication timestamp. These are not certified
ingestion counts. The reported total was 1,487 documents; the inconsistent
progress projection does not change the failed publication outcome.

The source executor acquires a 30-minute lease before materialization and
previously renewed it only immediately before publication. A 34-minute crawl
therefore outlived its lease without requiring a SQLite deadlock or provider
timeout. Tracked separately in **axon_rust-ethfl**.

A real-SQLite regression reproduced the defect with a one-second lease: after
1.5 seconds, a competing owner stole the active operation's lease. The initial
test failed for that exact reason. The scoped patch maintains source and
publication-finalizer leases in independently polled, abort-on-drop tasks,
renewing at one-third of the existing TTL. It does not extend the TTL or
reacquire lost leases. Existing publication validation and failed-generation
cleanup remain in place. Renewal errors are logged; final publication still
validates the source lease. Normal completion aborts and joins renewal before
release, while cancellation drops the renewal task.

The first source-suite run passed **260 tests**, including renewal, completion,
and cancellation regressions. A subsequent module extraction keeps modified
source files under 500 lines; the post-extraction source-suite run also passed
**260 tests**. Both test runs emitted Apple's existing large unwind-table linker
warning. Post-extraction Clippy passed with warnings denied, and the locked
release build passed in 6 minutes 33 seconds.

Deployed **07:45:13 UTC**, PID **42546**, SHA-256:

`b2faef082c4227f4408f384be5d6d6627eaef26971b39de8c26c89454ed680a2`

Before deployment, PostHog job `73f5eddf-4570-4566-8a17-1735fdcea478`
was still doing work with a source lease that expired at 07:08:33 UTC;
its last heartbeat was the original acquisition at 06:38:33 UTC. It could
not pass the existing final publication lease check. One controlled restart
activated the verified patch, and the supported recovery API requeued only
that interrupted job. OpenTelemetry was separately requeued as same-config
attempt 2, with no overrides. The new daemon then completed another job
degraded with 7 documents and 177 chunks, proving resumed execution but not
long-duration renewal correctness.

**The original OpenTelemetry failure remains unverified end to end.** Both
affected long crawls are queued. The issue remains open pending observed live
lease renewal and successful publication; no documentation checkbox was marked.

At 07:56 UTC, periodic renewal was observed in the live SQLite lease for job
`541fb1cf-eda4-4847-aec7-5f439374f0b6`: acquisition at 07:45:40.063033,
heartbeat at 07:55:40.077452, and expiry extended to 08:25:40.077452 UTC.
The job had reached 759 documents. No matching SQLite admission, database-lock,
pool-timeout, or source-renewal warnings appeared since deployment. This proves
the deployed renewal task executes; publication beyond the original TTL and
the original failed-source retries remain unverified.

## Host sleep and another provider timeout, 08:39 UTC

Qdrant documentation job `541fb1cf-eda4-4847-aec7-5f439374f0b6` failed at
08:20:33 UTC after reporting 1,133 documents and 16,904 chunks. The underlying
error was `vector.qdrant.transport` timeout during upsert, not source lease
expiry. The secondary `prepared work receiver closed` error followed consumer
failure. Provider logs inspected for 08:17–08:21 showed no completed request
over ten seconds; successful requests in the inspected tail took approximately
0.03–0.19 seconds. This does not establish the cause of the client timeout or
prove that all provider requests were fast.

Read-only macOS power logs reveal repeated host suspension: `Dark Wake Thermal
Emergency` sleep at 07:57:49 UTC for 904 seconds, maintenance wake at 08:12:53,
then another sleep at 08:23:17 for 912 seconds. This explains the missed
wall-clock heartbeat interval and delayed automation execution. The provider
timeout occurred during an awake interval; sleep alone is not a proven cause
of that specific failure. No thermal protection, power, or VPN settings were
changed. Sustained unattended ingestion requires resolving the host's repeated
suspension or obtaining authorization for a deployment move. The queue resumed
with React; all five previously tracked retries remain queued.

At the next check (executed at approximately 10:22 UTC after additional host
sleep), 1Password job `77e270eb-4d17-4086-a33a-96b7dc469591` had committed
`gen_1` at 09:38:02 UTC, beyond its initial 09:34:26 lease deadline. Its renewed
lease had expired at 09:44:26, allowing this 33-minute publication to succeed.
The job finished degraded; this is live evidence for periodic renewal, not full
documentation certification or verification of the original failed sources.

Discord job `9dff1940-d3ec-4c63-94d8-b0f0ad2d27c9` was advancing but its lease
still had acquisition/heartbeat 09:39:08 and expiry 10:09:08 UTC. Power logs show
sleep from 09:42:50 to 09:59:30 and again from 10:00:15 to 10:18:01. Sustained
host suspension can therefore still outlive the renewed-lease design. No lease
was forcibly reacquired, no thermal protection bypassed, and no restart made.

## Local disk exhaustion, 13:17 UTC

Four additional jobs failed around 13:11 UTC: Notion, Radix UI, ShellCheck,
and TypeScript. Radix UI's durable source error explicitly reports
`No space left on device (os error 28)`; the other three underlying messages
are redacted, so they cannot all be attributed to disk exhaustion from the
available error evidence alone. `df` reports the local data volume at 100%
capacity with 4.5 GiB available at inspection. Scoped `du` measurements:
Axon `target/` 103 GiB, `.axon/output` 367 MiB, `.axon/artifacts` 56 MiB,
and `.axon/logs` 36 MiB. Build artifacts are not automatically stale; none
were deleted, and no dirty work or ingestion data was removed.

The next job continued upserting, with 97 jobs queued. This new storage
blocker requires approved, verified cleanup or additional capacity before
reliable bulk ingestion can be assumed. The five tracked retries remain queued.

## Performance investigation, 16:07–16:21 UTC

The live queue was advancing rather than deadlocked: 541 completed, 105
completed-degraded, 219 failed, 32 queued, and one running. The running Android
documentation job `6aebfa50-7c04-442e-8efe-5c0a7392be52` advanced from 1,707
documents / 32,598 chunks at 16:07 to 1,951 documents / 39,708 chunks at 16:20.
Its current `gen_2` contains 24,102 source items; the older `gen_1` contains
23,306. Those generations must not be summed into the current crawl size.
The local volume now has approximately 140 GiB free; this investigation did
not delete anything or establish who freed the space.

The observed performance costs are distinct:

- 24 streaming acquisition waves covered 384 items in 779.482 seconds of
  cumulative wave wall time, with mean slot occupancy 54.25%. Each steady-state
  wave contains 16 items and finishes before the producer starts the next.
  This is evidence of acquisition-tail underutilization, not a quantified
  speedup from changing concurrency.
- A provider sample beginning 15:53:26 included 144 embedding operations
  totaling 73.303 seconds of provider time and 144 vector upserts totaling
  150.990 seconds. These sums can overlap and are not an additive critical-path
  attribution. Qdrant uses eight-point requests and one write slot; these
  deployment settings were not changed.
- The generation scheduler flushed at two **envelopes**, even when those
  envelopes were tiny single-document deliveries. This prematurely defeated
  the configured chunk-pool capacity.

The minimal scheduler correction retains the two-pool charged-chunk threshold
and makes the envelope-count fallback equivalent to two pools of one-unit
envelopes. Chunk/byte semaphores, the oldest-item deadline, FIFO, cancellation,
and publication accounting remain in place. No acquisition scope, caching
policy, timeout, provider configuration, or concurrency was changed.

TDD evidence: the focused policy test initially produced `[2, 2, 2, 2]` instead
of `[8]`. A real scheduler test using the production prepared-work channel,
vectorizer and ledger boundary now embeds all eight documents together and
retains all eight vector points. Temporarily restoring the old production
condition made that integration test fail with the same four two-item batches;
restoring the fix passed. Deadline and cancellation tests also pass.

Verification: `cargo test --offline -p axon-services --lib --locked source::`
passed **264 tests**, zero failures. `cargo clippy --offline -p axon-services
--lib --tests --locked -- -D warnings` also passed. The linker emitted the existing large
`__eh_frame`/compact-unwind-table warning. This is a reduction in unnecessary
provider operations on the controlled fixture, **not** a fourfold measured
end-to-end crawl speedup. Deployment and a live before/after comparison remain
pending; the running crawl was not restarted. Tracking: `axon_rust-rhtxr`.

The optimized release build (`cargo build --offline --release --bin axon
--locked`) passed in 7m07s. The new artifact is ready, but the existing process
continues executing the previous build until an approved restart. No commit
or push was performed.

## Android attempt failed, 17:31 UTC

Android job `6aebfa50-7c04-442e-8efe-5c0a7392be52` failed at 17:31:29 after
3,202 documents / 74,738 chunks. The durable inner error is
`embedding.tei.transport`: TEI transport timeout; producer cancellation is
secondary. Generation `gen_2` is failed/writing with no publication timestamp,
so the attempt is not certified ingestion. There were no matching SQLite
writer/lock/pool/lease warnings between 17:24:25 and the 17:34:58 check.

TEI remained up for five days and its health endpoint returned HTTP 200 in
0.143 seconds at follow-up. The inspected provider log tail after the failure
contains successful requests taking approximately 51–134 ms; this does not
identify the failed request or prove the cause of its timeout. Recent power
logs show no sleep spanning this failure. Further client/provider correlation
is required; the timeout root cause is not resolved.

The queue continued without intervention: three more jobs completed, another
was running, and 28 remained queued. The five tracked retries were still queued.
No retry, restart, deployment, or certification was performed during this check.
