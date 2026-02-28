use crate::crates::core::config::Config;
use crate::crates::core::logging::{log_info, log_warn};
use crate::crates::jobs::common::{
    WatchdogSweepStats, claim_next_pending, claim_pending_by_id, make_pool, mark_job_failed,
    open_amqp_connection_and_channel, reclaim_stale_running_jobs as generic_reclaim,
};
use crate::crates::jobs::worker_lane::validate_worker_env_vars;
use futures_util::StreamExt;
use lapin::options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions};
use lapin::types::FieldTable;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio;
use uuid::Uuid;

use super::super::{
    CrawlWatchdogSweepStats, STALE_SWEEP_INTERVAL_SECS, TABLE, WORKER_CONCURRENCY, ensure_schema,
};
use super::process::process_job;

/// Thin wrapper around the generic watchdog that delegates all logic to
/// `common::reclaim_stale_running_jobs` while mapping the result to the
/// crawl-specific stats type.
pub(crate) async fn reclaim_stale_running_jobs(
    pool: &PgPool,
    lane: usize,
    idle_timeout_secs: i64,
    confirm_secs: i64,
) -> Result<CrawlWatchdogSweepStats, Box<dyn Error>> {
    let marker = format!("lane={lane}");
    let stats: WatchdogSweepStats = generic_reclaim(
        pool,
        TABLE,
        "crawl",
        idle_timeout_secs,
        confirm_secs,
        &marker,
    )
    .await?;
    Ok(CrawlWatchdogSweepStats {
        stale_candidates: stats.stale_candidates,
        marked_candidates: stats.marked_candidates,
        reclaimed_jobs: stats.reclaimed_jobs,
    })
}

async fn run_worker_polling_loop(cfg: &Config, pool: &PgPool) -> Result<(), Box<dyn Error>> {
    log_warn("amqp unavailable; running crawl worker in postgres polling mode");
    if WORKER_CONCURRENCY <= 1 {
        return run_worker_polling_lane(cfg, pool, 1).await;
    }
    // Use join_all so a lane failure does not abruptly cancel sibling lanes
    // mid-job (which would leave jobs stuck in 'running' until the watchdog reclaims them).
    let results = futures_util::future::join_all(
        (1..=WORKER_CONCURRENCY).map(|lane| run_worker_polling_lane(cfg, pool, lane)),
    )
    .await;
    for r in results {
        r?;
    }
    Ok(())
}

async fn run_worker_polling_lane(
    cfg: &Config,
    pool: &PgPool,
    lane: usize,
) -> Result<(), Box<dyn Error>> {
    log_info(&format!(
        "crawl worker polling lane={} active queue={}",
        lane, cfg.crawl_queue
    ));
    let mut last_sweep = Instant::now();
    let mut last_heartbeat = Instant::now();
    loop {
        if last_heartbeat.elapsed() >= Duration::from_secs(60) {
            log_info(&format!("crawl worker heartbeat lane={} alive", lane));
            last_heartbeat = Instant::now();
        }
        if last_sweep.elapsed() >= Duration::from_secs(STALE_SWEEP_INTERVAL_SECS) {
            run_watchdog_sweep(
                pool,
                lane,
                cfg.watchdog_stale_timeout_secs,
                cfg.watchdog_confirm_secs,
            )
            .await;
            last_sweep = Instant::now();
        }
        if let Some(job_id) = claim_next_pending(pool, TABLE).await? {
            if let Err(err) = process_job(cfg, pool, job_id).await {
                let error_text = err.to_string();
                if let Err(mark_err) = mark_job_failed(pool, TABLE, job_id, &error_text).await {
                    log_warn(&format!(
                        "mark_job_failed error for crawl job {job_id}: {mark_err}"
                    ));
                }
                log_warn(&format!("worker failed crawl job {job_id}: {error_text}"));
            }
        } else {
            tokio::time::sleep(Duration::from_millis(800)).await;
        }
    }
}

/// Runs a single watchdog sweep and logs results. Shared by polling and AMQP lanes.
async fn run_watchdog_sweep(pool: &PgPool, lane: usize, stale_secs: i64, confirm_secs: i64) {
    match reclaim_stale_running_jobs(pool, lane, stale_secs, confirm_secs).await {
        Ok(stats) => {
            if stats.stale_candidates > 0 || stats.reclaimed_jobs > 0 {
                log_info(&format!(
                    "watchdog crawl sweep lane={} candidates={} marked={} reclaimed={}",
                    lane, stats.stale_candidates, stats.marked_candidates, stats.reclaimed_jobs
                ));
            }
        }
        Err(err) => log_warn(&format!("watchdog sweep failed (lane={}): {}", lane, err)),
    }
}

/// Parses, claims, and acks a single AMQP delivery.
///
/// Returns `Some(job_id)` when the delivery was successfully claimed and the
/// ack was sent, or `None` if the delivery was malformed, already claimed, or
/// encountered a DB error (in which case the delivery is nacked).
async fn claim_delivery(
    pool: &PgPool,
    delivery: lapin::message::Delivery,
    lane: usize,
) -> Option<Uuid> {
    let parsed = std::str::from_utf8(&delivery.data)
        .ok()
        .and_then(|s| Uuid::parse_str(s.trim()).ok());
    let Some(job_id) = parsed else {
        log_warn(&format!(
            "malformed crawl delivery payload (lane={lane}, len={}) - acking and skipping",
            delivery.data.len()
        ));
        if let Err(err) = delivery.ack(BasicAckOptions::default()).await {
            log_warn(&format!(
                "failed to ack malformed crawl delivery (lane={lane}): {err}"
            ));
        }
        return None;
    };

    match claim_pending_by_id(pool, TABLE, job_id).await {
        Ok(true) => {
            // Ack before processing: crawls can run for hours, and RabbitMQ's
            // consumer_timeout (default 30 min) will forcibly close the channel if
            // the ack comes too late. The DB is the source of truth for job state;
            // the watchdog reclaims any job that crashes without completing.
            if let Err(err) = delivery.ack(BasicAckOptions::default()).await {
                // Ack failure does not prevent processing: the job is already
                // claimed in the DB (status='running'). Skipping would leave
                // it stuck until the watchdog reclaims it.
                log_warn(&format!(
                    "failed to ack crawl delivery (lane={lane}), processing anyway: {err}"
                ));
            }
            Some(job_id)
        }
        Ok(false) => {
            if let Err(err) = delivery.ack(BasicAckOptions::default()).await {
                log_warn(&format!(
                    "failed to ack already-claimed crawl delivery (lane={lane}): {err}"
                ));
            }
            None
        }
        Err(err) => {
            log_warn(&format!(
                "failed to claim crawl job {job_id} (lane={lane}); nacking for retry: {err}"
            ));
            // Brief pause before requeuing so a persistent DB failure (e.g. PG
            // restart) doesn't spin-loop at full speed consuming CPU and flooding logs.
            tokio::time::sleep(Duration::from_secs(5)).await;
            if let Err(nack_err) = delivery
                .nack(BasicNackOptions {
                    requeue: true,
                    ..Default::default()
                })
                .await
            {
                log_warn(&format!(
                    "failed to nack crawl delivery for job {job_id} (lane={lane}): {nack_err}"
                ));
            }
            None
        }
    }
}

/// Mutable interval handles shared across a single lane's consumer loop.
struct LaneTimers<'a> {
    heartbeat: &'a mut tokio::time::Interval,
    sweep: &'a mut tokio::time::Interval,
}

/// Runs `process_job` for the given `job_id` while keeping heartbeat and watchdog
/// sweep intervals alive. Returns once the job completes (success or failure).
///
/// `process_job` returns `Box<dyn Error>` which is `!Send`, so we cannot use
/// `tokio::spawn`. Instead we pin the future and poll it inside a `select!` loop
/// alongside the interval ticks. This keeps ticks responsive during long crawls
/// while preserving the 1-job-per-lane guarantee.
async fn run_job_with_ticks(
    cfg: &Config,
    pool: &PgPool,
    job_id: Uuid,
    lane: usize,
    timers: &mut LaneTimers<'_>,
) {
    let job_fut = process_job(cfg, pool, job_id);
    tokio::pin!(job_fut);
    let result = loop {
        tokio::select! {
            result = &mut job_fut => break result,
            _ = timers.heartbeat.tick() => {
                log_info(&format!("crawl worker heartbeat lane={} alive", lane));
            },
            _ = timers.sweep.tick() => {
                run_watchdog_sweep(pool, lane, cfg.watchdog_stale_timeout_secs, cfg.watchdog_confirm_secs).await;
            },
        }
    };
    if let Err(err) = result {
        let error_text = err.to_string();
        if let Err(mark_err) = mark_job_failed(pool, TABLE, job_id, &error_text).await {
            log_warn(&format!(
                "mark_job_failed error for crawl job {job_id}: {mark_err}"
            ));
        }
        log_warn(&format!("worker failed crawl job {job_id}: {error_text}"));
    }
}

/// Runs one AMQP consumer loop for a single lane. Returns an error when the
/// consumer stream ends or the channel is closed. Callers wrap this in a
/// reconnect loop via `run_amqp_lane_with_reconnect`.
async fn run_amqp_worker_lane(
    cfg: Arc<Config>,
    pool: PgPool,
    lane: usize,
) -> Result<(), Box<dyn Error>> {
    // Hold `_conn` in scope for the entire consumer loop. open_amqp_connection_and_channel
    // returns the Connection; dropping it would close the backing TCP connection
    // and kill the consumer stream. Keeping _conn alive prevents that.
    let (_conn, ch) = open_amqp_connection_and_channel(&cfg, &cfg.crawl_queue).await?;
    let consumer_tag = format!("axon-rust-crawl-worker-{lane}");
    let mut consumer = ch
        .basic_consume(
            &cfg.crawl_queue,
            &consumer_tag,
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    log_info(&format!(
        "crawl worker lane={} listening on queue={} concurrency={}",
        lane, cfg.crawl_queue, WORKER_CONCURRENCY
    ));

    let mut sweep_interval = tokio::time::interval(Duration::from_secs(STALE_SWEEP_INTERVAL_SECS));
    sweep_interval.tick().await; // consume the immediate first tick
    let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(60));
    heartbeat_interval.tick().await; // consume the immediate first tick

    loop {
        // Poll for the next AMQP message while keeping heartbeat and sweep ticks alive.
        let msg = {
            let timers = LaneTimers {
                heartbeat: &mut heartbeat_interval,
                sweep: &mut sweep_interval,
            };
            tokio::select! {
                msg = consumer.next() => match msg {
                    Some(msg) => msg,
                    None => break,
                },
                _ = timers.heartbeat.tick() => {
                    log_info(&format!("crawl worker heartbeat lane={} alive", lane));
                    continue;
                },
                _ = timers.sweep.tick() => {
                    run_watchdog_sweep(&pool, lane, cfg.watchdog_stale_timeout_secs, cfg.watchdog_confirm_secs).await;
                    continue;
                }
            }
        };
        let delivery = match msg {
            Ok(d) => d,
            Err(err) => {
                log_warn(&format!("consumer error (lane={lane}): {err}"));
                continue;
            }
        };
        // Claim the delivery; skip if malformed, already taken, or DB error.
        let Some(job_id) = claim_delivery(&pool, delivery, lane).await else {
            continue;
        };
        // Run the job while keeping heartbeat and sweep ticks responsive.
        // process_job is !Send (Box<dyn Error>), so we pin it here rather than spawning.
        // This preserves the 1-job-per-lane guarantee.
        let mut timers = LaneTimers {
            heartbeat: &mut heartbeat_interval,
            sweep: &mut sweep_interval,
        };
        run_job_with_ticks(&cfg, &pool, job_id, lane, &mut timers).await;
    }

    Err(format!("crawl worker consumer stream ended unexpectedly (lane={lane})").into())
}

/// Initial reconnect backoff in seconds. Doubles on each attempt, capped at 60s.
const RECONNECT_BACKOFF_INITIAL_SECS: u64 = 2;
const RECONNECT_BACKOFF_MAX_SECS: u64 = 60;

/// Wraps `run_amqp_worker_lane` in an infinite reconnect loop with exponential
/// backoff. When the channel dies (AMQP consumer_timeout, broker restart, etc.),
/// the current job completes normally (it holds no AMQP channel reference), then
/// the lane reconnects and resumes. This function never returns; callers use
/// `tokio::join!` to run multiple lanes concurrently.
async fn run_amqp_lane_with_reconnect(cfg: Arc<Config>, pool: PgPool, lane: usize) {
    let mut backoff_secs = RECONNECT_BACKOFF_INITIAL_SECS;
    loop {
        match run_amqp_worker_lane(Arc::clone(&cfg), pool.clone(), lane).await {
            Ok(()) => {
                // run_amqp_worker_lane only returns Ok if the consumer stream
                // ended cleanly, which shouldn't happen in normal operation.
                log_warn(&format!(
                    "crawl worker lane={lane} AMQP loop exited cleanly; reconnecting"
                ));
            }
            Err(err) => {
                log_warn(&format!(
                    "crawl worker lane={lane} AMQP error: {err}; reconnecting in {backoff_secs}s"
                ));
            }
        }
        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
        backoff_secs = (backoff_secs * 2).min(RECONNECT_BACKOFF_MAX_SECS);
        log_info(&format!(
            "crawl worker lane={lane} attempting AMQP reconnect"
        ));
    }
}

pub(crate) async fn run_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    // Validate required environment variables before attempting any connections.
    if let Err(msg) = validate_worker_env_vars() {
        return Err(msg.into());
    }

    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    match reclaim_stale_running_jobs(
        &pool,
        0,
        cfg.watchdog_stale_timeout_secs,
        cfg.watchdog_confirm_secs,
    )
    .await
    {
        Ok(stats) => {
            if stats.stale_candidates > 0 || stats.reclaimed_jobs > 0 {
                log_info(&format!(
                    "watchdog crawl startup sweep candidates={} marked={} reclaimed={}",
                    stats.stale_candidates, stats.marked_candidates, stats.reclaimed_jobs
                ));
            }
        }
        Err(err) => log_warn(&format!("watchdog crawl startup sweep failed: {err}")),
    }

    // Probe AMQP connectivity with a short-lived connection+channel pair.
    // Close both explicitly so RabbitMQ doesn't accumulate orphaned channels.
    // Each lane opens its own long-lived connection for its consumer loop.
    let amqp_available = match open_amqp_connection_and_channel(cfg, &cfg.crawl_queue).await {
        Ok((conn, ch)) => {
            let _ = ch.close(0, "probe").await;
            let _ = conn.close(200, "probe").await;
            true
        }
        Err(_) => false,
    };
    if !amqp_available {
        return run_worker_polling_loop(cfg, &pool).await;
    }
    let cfg_arc = Arc::new(cfg.clone());
    if WORKER_CONCURRENCY <= 1 {
        run_amqp_lane_with_reconnect(cfg_arc, pool, 1).await;
        return Ok(());
    }

    // Run all lanes concurrently. Each lane has its own reconnect loop, so a
    // channel death in one lane does not affect the others — each reconnects
    // independently. Lane independence is achieved by the reconnect loop, not
    // by separate OS threads: if one lane's channel dies it reconnects while
    // the others continue uninterrupted.
    //
    // Note: run_amqp_lane_with_reconnect is !Send (process_job uses Box<dyn Error>),
    // so tokio::spawn cannot be used here. join_all runs all lanes on the same
    // task, which is correct — each lane has its own reconnect guard.
    futures_util::future::join_all(
        (1..=WORKER_CONCURRENCY)
            .map(|lane| run_amqp_lane_with_reconnect(Arc::clone(&cfg_arc), pool.clone(), lane)),
    )
    .await;
    Ok(())
}
