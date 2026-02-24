use crate::crates::core::config::Config;
use crate::crates::core::logging::{log_info, log_warn};
use crate::crates::jobs::common::{
    JobTable, claim_next_pending, claim_pending_by_id, open_amqp_connection_and_channel,
    reclaim_stale_running_jobs,
};
use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use lapin::options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicQosOptions};
use lapin::types::FieldTable;
use sqlx::PgPool;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio;
use uuid::Uuid;

const STALE_SWEEP_INTERVAL_SECS: u64 = 30;

/// Polling backoff constants (milliseconds).
const POLL_BACKOFF_INIT_MS: u64 = 100;
const POLL_BACKOFF_MAX_MS: u64 = 6400;

/// A boxed async function that processes a single claimed job.
/// It must handle its own error logging/marking internally (returns `()`).
pub(crate) type ProcessFn =
    Arc<dyn Fn(Config, PgPool, Uuid) -> Pin<Box<dyn Future<Output = ()>>> + Send + Sync>;

/// Configuration for a generic worker.
pub(crate) struct WorkerConfig {
    pub table: JobTable,
    pub queue_name: String,
    pub job_kind: &'static str,
    pub consumer_tag_prefix: &'static str,
    pub lane_count: usize,
}

/// Validate that the critical infrastructure environment variables are present
/// before the worker attempts any network connections.
///
/// Required variables:
/// - Postgres: `AXON_PG_URL`
/// - Redis:    `AXON_REDIS_URL`
/// - AMQP:     `AXON_AMQP_URL`
///
/// Note: this checks for the presence of at least one variable per service, not
/// the validity of the URL. Connection errors are still reported at connect time.
pub(crate) fn validate_worker_env_vars() -> Result<(), String> {
    let mut missing: Vec<&'static str> = Vec::new();

    // Postgres: AXON_PG_URL
    let pg_ok = std::env::var("AXON_PG_URL").is_ok();
    if !pg_ok {
        missing.push("AXON_PG_URL");
    }

    // Redis: AXON_REDIS_URL
    let redis_ok = std::env::var("AXON_REDIS_URL").is_ok();
    if !redis_ok {
        missing.push("AXON_REDIS_URL");
    }

    // AMQP: AXON_AMQP_URL
    let amqp_ok = std::env::var("AXON_AMQP_URL").is_ok();
    if !amqp_ok {
        missing.push("AXON_AMQP_URL");
    }

    if !missing.is_empty() {
        return Err(format!(
            "worker startup error: the following required environment variables are not set:\n  {}\n\
             Set them in your environment or .env file before starting the worker.",
            missing.join("\n  ")
        ));
    }
    Ok(())
}

/// Run the stale-job sweep and log results.
async fn sweep_stale_jobs(
    cfg: &Config,
    pool: &PgPool,
    wc: &WorkerConfig,
    source: &str,
    lane: usize,
) {
    match reclaim_stale_running_jobs(
        pool,
        wc.table,
        wc.job_kind,
        cfg.watchdog_stale_timeout_secs,
        cfg.watchdog_confirm_secs,
        source,
    )
    .await
    {
        Ok(stats) => {
            if stats.stale_candidates > 0 || stats.reclaimed_jobs > 0 {
                log_info(&format!(
                    "watchdog {} sweep lane={} candidates={} marked={} reclaimed={}",
                    wc.job_kind,
                    lane,
                    stats.stale_candidates,
                    stats.marked_candidates,
                    stats.reclaimed_jobs
                ));
            }
        }
        Err(e) => {
            log_warn(&format!(
                "watchdog {} sweep failed (lane={lane}): {e}",
                wc.job_kind
            ));
        }
    }
}

/// Generic AMQP consumer lane. Listens for job IDs on the queue, claims them,
/// and dispatches to `process_fn` concurrently using `FuturesUnordered` with a
/// semaphore for backpressure. Runs stale sweeps on idle timeout.
async fn run_amqp_lane(
    cfg: &Config,
    pool: PgPool,
    wc: &WorkerConfig,
    lane: usize,
    process_fn: &ProcessFn,
    semaphore: Arc<tokio::sync::Semaphore>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, ch) = open_amqp_connection_and_channel(cfg, &wc.queue_name).await?;

    // Tell the broker to only push one unacked message at a time per consumer,
    // preventing a single lane from buffering more work than it can process.
    ch.basic_qos(1, BasicQosOptions::default()).await?;

    let tag = format!("{}-{lane}", wc.consumer_tag_prefix);
    let mut consumer = ch
        .basic_consume(
            &wc.queue_name,
            &tag,
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    log_info(&format!(
        "{} worker lane={lane} listening on queue={} concurrency={}",
        wc.job_kind, wc.queue_name, wc.lane_count
    ));

    let mut inflight = FuturesUnordered::new();

    loop {
        // If all permits are consumed, block until at least one in-flight job
        // completes. This guarantees forward progress and prevents claiming new
        // jobs that cannot run yet.
        if semaphore.available_permits() == 0 && !inflight.is_empty() {
            let _ = inflight.next().await;
            continue;
        }

        let timed = if inflight.is_empty() {
            tokio::time::timeout(
                Duration::from_secs(STALE_SWEEP_INTERVAL_SECS),
                consumer.next(),
            )
            .await
        } else {
            tokio::select! {
                maybe_done = inflight.next() => {
                    if maybe_done.is_some() {
                        continue;
                    }
                    // No in-flight jobs left; fall back to consumer poll.
                    tokio::time::timeout(Duration::from_secs(STALE_SWEEP_INTERVAL_SECS), consumer.next()).await
                }
                delivery = tokio::time::timeout(Duration::from_secs(STALE_SWEEP_INTERVAL_SECS), consumer.next()) => {
                    delivery
                }
            }
        };
        let delivery = match timed {
            Ok(Some(Ok(d))) => d,
            Ok(Some(Err(e))) => {
                log_warn(&format!(
                    "{} worker lane={lane} AMQP delivery error: {e}",
                    wc.job_kind
                ));
                continue;
            }
            Ok(None) => break,
            Err(_) => {
                sweep_stale_jobs(cfg, &pool, wc, "amqp", lane).await;
                continue;
            }
        };

        let parsed = std::str::from_utf8(&delivery.data)
            .ok()
            .and_then(|s| Uuid::parse_str(s.trim()).ok());
        let Some(job_id) = parsed else {
            log_warn(&format!(
                "{} worker lane={lane} malformed delivery payload (len={}), acking and skipping",
                wc.job_kind,
                delivery.data.len()
            ));
            delivery.ack(BasicAckOptions::default()).await?;
            continue;
        };

        // Reserve capacity first so we never claim a job without a runnable slot.
        let permit = semaphore.clone().acquire_owned().await?;
        match claim_pending_by_id(&pool, wc.table, job_id).await {
            Ok(true) => {
                delivery.ack(BasicAckOptions::default()).await?;
                let fut = process_fn(cfg.clone(), pool.clone(), job_id);
                inflight.push(async move {
                    fut.await;
                    drop(permit);
                });
            }
            Ok(false) => {
                drop(permit);
                // Another lane claimed this ID first; ack and skip.
                delivery.ack(BasicAckOptions::default()).await?;
            }
            Err(e) => {
                drop(permit);
                log_warn(&format!(
                    "{} worker lane={lane} DB error claiming job {job_id}; nacking for retry: {e}",
                    wc.job_kind
                ));
                if let Err(nack_err) = delivery
                    .nack(BasicNackOptions {
                        requeue: true,
                        ..Default::default()
                    })
                    .await
                {
                    log_warn(&format!(
                        "{} worker lane={lane} failed to nack delivery: {nack_err}",
                        wc.job_kind
                    ));
                }
            }
        }
    }

    // Drain any remaining in-flight jobs before exiting.
    while inflight.next().await.is_some() {}

    // Explicitly close channel and connection so RabbitMQ cleans up immediately
    // rather than waiting for the TCP timeout.
    let _ = ch.close(200, "lane exit").await;
    let _ = conn.close(200, "lane exit").await;

    Err(format!(
        "{} worker lane={lane} AMQP consumer stream ended unexpectedly",
        wc.job_kind
    )
    .into())
}

/// Generic polling lane. Claims pending jobs via SQL polling with exponential
/// backoff (100ms -> 6400ms on idle, reset on job found). Dispatches to
/// `process_fn` concurrently using `FuturesUnordered` with semaphore backpressure.
/// Runs stale sweeps on the configured interval.
async fn run_polling_lane(
    cfg: &Config,
    pool: PgPool,
    wc: &WorkerConfig,
    lane: usize,
    process_fn: &ProcessFn,
    semaphore: Arc<tokio::sync::Semaphore>,
) -> Result<(), Box<dyn std::error::Error>> {
    log_info(&format!(
        "{} worker polling lane={lane} active queue={}",
        wc.job_kind, wc.queue_name
    ));
    let mut last_sweep = Instant::now();
    let mut backoff_ms = POLL_BACKOFF_INIT_MS;
    let mut inflight = FuturesUnordered::new();

    loop {
        // If all permits are consumed, block until one in-flight job completes.
        // Without this gate, permit acquisition can starve in-flight polling.
        if semaphore.available_permits() == 0 && !inflight.is_empty() {
            let _ = inflight.next().await;
            continue;
        }

        if last_sweep.elapsed() >= Duration::from_secs(STALE_SWEEP_INTERVAL_SECS) {
            sweep_stale_jobs(cfg, &pool, wc, "polling", lane).await;
            last_sweep = Instant::now();
        }
        // Reserve capacity first so we never claim a job without a runnable slot.
        let permit = semaphore.clone().acquire_owned().await?;
        match claim_next_pending(&pool, wc.table).await {
            Ok(Some(id)) => {
                backoff_ms = POLL_BACKOFF_INIT_MS;
                let fut = process_fn(cfg.clone(), pool.clone(), id);
                inflight.push(async move {
                    fut.await;
                    drop(permit);
                });
            }
            Ok(None) => {
                drop(permit);
                let sleep = tokio::time::sleep(Duration::from_millis(backoff_ms));
                tokio::pin!(sleep);
                tokio::select! {
                    _ = &mut sleep => {}
                    done = inflight.next(), if !inflight.is_empty() => {
                        if done.is_some() {
                            continue;
                        }
                    }
                }
                backoff_ms = (backoff_ms * 2).min(POLL_BACKOFF_MAX_MS);
            }
            Err(err) => {
                drop(permit);
                log_warn(&format!(
                    "{} worker polling lane={lane} DB error; retrying in 5s: {err}",
                    wc.job_kind
                ));
                let sleep = tokio::time::sleep(Duration::from_secs(5));
                tokio::pin!(sleep);
                tokio::select! {
                    _ = &mut sleep => {}
                    done = inflight.next(), if !inflight.is_empty() => {
                        if done.is_some() {
                            continue;
                        }
                    }
                }
            }
        }
    }
}

/// Generic top-level worker: startup sweep, probe AMQP, then run `lane_count` lanes
/// (AMQP or polling fallback) using `futures_util::future::join_all` for dynamic concurrency.
///
/// A shared `Semaphore` limits total in-flight spawned tasks to `lane_count`.
///
/// Callers must call `make_pool` and `ensure_schema` before invoking this.
pub(crate) async fn run_job_worker(
    cfg: &Config,
    pool: PgPool,
    wc: &WorkerConfig,
    process_fn: ProcessFn,
) -> Result<(), Box<dyn std::error::Error>> {
    if wc.lane_count == 0 {
        return Err(format!("{} worker: lane_count must be >= 1", wc.job_kind).into());
    }

    sweep_stale_jobs(cfg, &pool, wc, "startup", 0).await;

    let semaphore = Arc::new(tokio::sync::Semaphore::new(wc.lane_count));

    // Probe AMQP connectivity with a short-lived connection+channel pair.
    let amqp_available = match open_amqp_connection_and_channel(cfg, &wc.queue_name).await {
        Ok((conn, ch)) => {
            let _ = ch.close(0, "probe").await;
            let _ = conn.close(200, "probe").await;
            true
        }
        Err(e) => {
            log_warn(&format!(
                "{} worker: AMQP probe failed ({}), falling back to polling: {e}",
                wc.job_kind, wc.queue_name
            ));
            false
        }
    };

    if amqp_available {
        loop {
            let futs: Vec<_> = (1..=wc.lane_count)
                .map(|lane| {
                    run_amqp_lane(cfg, pool.clone(), wc, lane, &process_fn, semaphore.clone())
                })
                .collect();
            let results = futures_util::future::join_all(futs).await;
            for result in results {
                if let Err(err) = result {
                    log_warn(&format!(
                        "{} worker lane terminated unexpectedly: {err}",
                        wc.job_kind
                    ));
                }
            }
            log_warn(&format!(
                "{} worker restarting AMQP lanes in 2s",
                wc.job_kind
            ));
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    log_warn(&format!(
        "amqp unavailable; running {} worker in postgres polling mode",
        wc.job_kind
    ));
    let futs: Vec<_> = (1..=wc.lane_count)
        .map(|lane| run_polling_lane(cfg, pool.clone(), wc, lane, &process_fn, semaphore.clone()))
        .collect();
    let results = futures_util::future::join_all(futs).await;
    for result in results {
        result?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tokio::sync::Mutex;

    /// Verify semaphore permits: N permits from a semaphore of size N all succeed
    /// immediately, but the (N+1)-th blocks until one is released.
    #[tokio::test]
    async fn semaphore_permits_up_to_capacity_then_blocks() {
        let sem = Arc::new(tokio::sync::Semaphore::new(2));

        // Two permits acquired immediately.
        let p1 = sem.clone().acquire_owned().await.unwrap();
        let p2 = sem.clone().acquire_owned().await.unwrap();
        assert_eq!(sem.available_permits(), 0);

        // Third acquire should not resolve within a short timeout.
        let sem2 = sem.clone();
        let blocked = tokio::time::timeout(Duration::from_millis(50), sem2.acquire_owned()).await;
        assert!(
            blocked.is_err(),
            "third permit should block when capacity=2"
        );

        // Release one → third now succeeds.
        drop(p1);
        let p3 = tokio::time::timeout(Duration::from_millis(50), sem.clone().acquire_owned())
            .await
            .expect("third permit should succeed after release")
            .unwrap();
        assert_eq!(sem.available_permits(), 0);

        drop(p2);
        drop(p3);
        assert_eq!(sem.available_permits(), 2);
    }

    /// Verify that FuturesUnordered + semaphore allows two "jobs" to execute
    /// concurrently: both start before either finishes.
    #[tokio::test]
    async fn futures_unordered_runs_jobs_concurrently() {
        // Each job records (start_instant, end_instant) into shared vec.
        let log: Arc<Mutex<Vec<(Instant, Instant)>>> = Arc::new(Mutex::new(Vec::new()));
        let sem = Arc::new(tokio::sync::Semaphore::new(2));

        let mut inflight = FuturesUnordered::new();

        for _ in 0..2 {
            let permit = sem.clone().acquire_owned().await.unwrap();
            let log = log.clone();
            inflight.push(async move {
                let start = Instant::now();
                // Simulate work — 50ms is enough to prove overlap.
                tokio::time::sleep(Duration::from_millis(50)).await;
                let end = Instant::now();
                log.lock().await.push((start, end));
                drop(permit);
            });
        }

        // Drive all futures to completion.
        while inflight.next().await.is_some() {}

        let entries = log.lock().await;
        assert_eq!(entries.len(), 2);

        // Concurrent execution means job[1] started before job[0] ended.
        // Since both sleep 50ms and we push them both before polling,
        // the earlier-starting job should still be running when the second starts.
        let (start0, end0) = entries[0];
        let (start1, _end1) = entries[1];
        // Whichever started second should have started before the other finished.
        let (earlier_end, later_start) = if start0 <= start1 {
            (end0, start1)
        } else {
            (_end1, start0)
        };
        assert!(
            later_start < earlier_end,
            "jobs should overlap: later_start={later_start:?} should be < earlier_end={earlier_end:?}"
        );
    }

    /// Verify that when the semaphore is full, new jobs block until a permit is
    /// released (backpressure behavior matching worker_lane dispatch).
    #[tokio::test]
    async fn semaphore_backpressure_blocks_third_dispatch() {
        let sem = Arc::new(tokio::sync::Semaphore::new(2));
        let counter = Arc::new(AtomicU64::new(0));
        let mut inflight = FuturesUnordered::new();

        // Dispatch 2 long-running jobs.
        for _ in 0..2 {
            let permit = sem.clone().acquire_owned().await.unwrap();
            let counter = counter.clone();
            inflight.push(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(200)).await;
                drop(permit);
            });
        }

        // Try to acquire a third permit — should block.
        let sem_for_third = sem.clone();
        let third_handle = tokio::spawn(async move {
            let _permit = sem_for_third.acquire_owned().await.unwrap();
        });

        // Give tasks a moment to start, then verify 3rd is still pending.
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !third_handle.is_finished(),
            "third dispatch should be blocked"
        );

        // Drain inflight → releases permits → third unblocks.
        while inflight.next().await.is_some() {}
        tokio::time::timeout(Duration::from_millis(50), third_handle)
            .await
            .expect("third should complete after permits released")
            .unwrap();
    }

    /// Verify the exponential backoff sequence: 100 → 200 → 400 → 800 → 1600
    /// → 3200 → 6400 → 6400 (capped), then reset to 100 on job found.
    #[test]
    fn polling_backoff_sequence_doubles_caps_and_resets() {
        let mut backoff_ms = POLL_BACKOFF_INIT_MS;
        let expected = [100, 200, 400, 800, 1600, 3200, 6400, 6400, 6400];

        for (i, &expected_ms) in expected.iter().enumerate() {
            assert_eq!(
                backoff_ms, expected_ms,
                "iteration {i}: expected {expected_ms}ms, got {backoff_ms}ms"
            );
            // Simulate idle: double and cap (same logic as run_polling_lane).
            backoff_ms = (backoff_ms * 2).min(POLL_BACKOFF_MAX_MS);
        }

        // Simulate job found: reset.
        backoff_ms = POLL_BACKOFF_INIT_MS;
        assert_eq!(backoff_ms, 100, "should reset to 100ms on job found");

        // Verify it resumes doubling after reset.
        backoff_ms = (backoff_ms * 2).min(POLL_BACKOFF_MAX_MS);
        assert_eq!(backoff_ms, 200, "should double to 200ms after reset");
    }

    /// Verify backoff boundary constants are correct.
    #[test]
    fn polling_backoff_constants_are_valid() {
        assert_eq!(POLL_BACKOFF_INIT_MS, 100);
        assert_eq!(POLL_BACKOFF_MAX_MS, 6400);
        // Cap should be a power-of-two multiple of init.
        const { assert!(POLL_BACKOFF_MAX_MS >= POLL_BACKOFF_INIT_MS) };
        assert_eq!(POLL_BACKOFF_MAX_MS, POLL_BACKOFF_INIT_MS * 64);
    }

    /// Verify validate_worker_env_vars passes when all required vars are present.
    #[allow(unsafe_code)]
    #[test]
    fn validate_env_vars_passes_when_all_set() {
        // Set all required vars.
        unsafe {
            std::env::set_var("AXON_PG_URL", "postgresql://localhost/test");
            std::env::set_var("AXON_REDIS_URL", "redis://localhost");
            std::env::set_var("AXON_AMQP_URL", "amqp://localhost");
        }

        let result = validate_worker_env_vars();
        assert!(
            result.is_ok(),
            "expected env validation success: {result:?}"
        );

        unsafe {
            std::env::remove_var("AXON_PG_URL");
            std::env::remove_var("AXON_REDIS_URL");
            std::env::remove_var("AXON_AMQP_URL");
        }
    }

    /// Verify canonical variables are required and missing vars fail recognition.
    #[allow(unsafe_code)]
    #[test]
    fn validate_env_vars_requires_canonical_names() {
        unsafe {
            std::env::remove_var("AXON_PG_URL");
            std::env::remove_var("AXON_REDIS_URL");
            std::env::remove_var("AXON_AMQP_URL");
        }

        let result = validate_worker_env_vars();
        assert!(result.is_err(), "expected env validation failure");
        let msg = result.err().unwrap_or_default();
        assert!(msg.contains("AXON_PG_URL"));
        assert!(msg.contains("AXON_REDIS_URL"));
        assert!(msg.contains("AXON_AMQP_URL"));

        unsafe {
            std::env::remove_var("AXON_PG_URL");
            std::env::remove_var("AXON_REDIS_URL");
            std::env::remove_var("AXON_AMQP_URL");
        }
    }
}
