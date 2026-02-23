use crate::crates::core::config::Config;
use crate::crates::jobs::crawl_jobs::runtime;
use std::error::Error;

/// Reclaim stale running crawl jobs using the two-pass confirmation protocol.
///
/// A crawl job is considered stale when its `updated_at` timestamp has not
/// advanced for longer than `cfg.watchdog_stale_timeout_secs`.
///
/// **Two-pass confirmation protocol:**
///
/// Pass 1 — mark: the first sweep sets a `_watchdog` marker in the job's
///   `result_json` column, recording the `observed_updated_at` timestamp and
///   the current wall-clock time (`first_seen_at`).
///
/// Pass 2 — confirm: a subsequent sweep (at least `cfg.watchdog_confirm_secs`
///   later) checks that `updated_at` is still unchanged and that
///   `first_seen_at` is at least `watchdog_confirm_secs` old. Only then is the
///   job marked `failed` with a watchdog reclaim message in `error_text`.
///
/// This two-pass design prevents false positives where a job's heartbeat races
/// with the sweep window — the confirm pass ensures the job has been genuinely
/// idle for the full confirmation window before being reclaimed.
///
/// Returns the total number of jobs moved from `running` → `failed`.
pub async fn recover_stale_crawl_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    runtime::recover_stale_crawl_jobs(cfg).await
}
