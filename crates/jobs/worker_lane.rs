use crate::crates::core::config::Config;
use crate::crates::core::logging::{log_info, log_warn};
use crate::crates::jobs::common::{
    claim_next_pending, claim_pending_by_id, open_amqp_connection_and_channel,
    reclaim_stale_running_jobs, JobTable,
};
use futures_util::StreamExt;
use lapin::options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions};
use lapin::types::FieldTable;
use spider::tokio;
use sqlx::PgPool;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

const STALE_SWEEP_INTERVAL_SECS: u64 = 30;

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
/// and dispatches to `process_fn`. Runs stale sweeps on idle timeout.
async fn run_amqp_lane(
    cfg: &Config,
    pool: PgPool,
    wc: &WorkerConfig,
    lane: usize,
    process_fn: &ProcessFn,
) -> Result<(), Box<dyn std::error::Error>> {
    let (_conn, ch) = open_amqp_connection_and_channel(cfg, &wc.queue_name).await?;
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

    loop {
        let timed = tokio::time::timeout(
            Duration::from_secs(STALE_SWEEP_INTERVAL_SECS),
            consumer.next(),
        )
        .await;
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

        match claim_pending_by_id(&pool, wc.table, job_id).await {
            Ok(true) => {
                delivery.ack(BasicAckOptions::default()).await?;
                process_fn(cfg.clone(), pool.clone(), job_id).await;
            }
            Ok(false) => {
                // Another lane claimed this ID first; ack and skip.
                delivery.ack(BasicAckOptions::default()).await?;
            }
            Err(e) => {
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

    Err(format!(
        "{} worker lane={lane} AMQP consumer stream ended unexpectedly",
        wc.job_kind
    )
    .into())
}

/// Generic polling lane. Claims pending jobs via SQL polling with an 800ms idle
/// sleep. Runs stale sweeps on the configured interval.
async fn run_polling_lane(
    cfg: &Config,
    pool: PgPool,
    wc: &WorkerConfig,
    lane: usize,
    process_fn: &ProcessFn,
) -> Result<(), Box<dyn std::error::Error>> {
    log_info(&format!(
        "{} worker polling lane={lane} active queue={}",
        wc.job_kind, wc.queue_name
    ));
    let mut last_sweep = Instant::now();
    loop {
        if last_sweep.elapsed() >= Duration::from_secs(STALE_SWEEP_INTERVAL_SECS) {
            sweep_stale_jobs(cfg, &pool, wc, "polling", lane).await;
            last_sweep = Instant::now();
        }
        match claim_next_pending(&pool, wc.table).await {
            Ok(Some(id)) => {
                process_fn(cfg.clone(), pool.clone(), id).await;
            }
            Ok(None) => {
                tokio::time::sleep(Duration::from_millis(800)).await;
            }
            Err(err) => {
                log_warn(&format!(
                    "{} worker polling lane={lane} DB error; retrying in 5s: {err}",
                    wc.job_kind
                ));
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

/// Generic top-level worker: startup sweep, probe AMQP, then run `lane_count` lanes
/// (AMQP or polling fallback) using `futures_util::future::join_all` for dynamic concurrency.
///
/// Callers must call `make_pool` and `ensure_schema` before invoking this.
pub(crate) async fn run_job_worker(
    cfg: &Config,
    pool: PgPool,
    wc: &WorkerConfig,
    process_fn: ProcessFn,
) -> Result<(), Box<dyn std::error::Error>> {
    sweep_stale_jobs(cfg, &pool, wc, "startup", 0).await;

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
                .map(|lane| run_amqp_lane(cfg, pool.clone(), wc, lane, &process_fn))
                .collect();
            let results = futures_util::future::join_all(futs).await;
            let mut all_ok = true;
            for result in results {
                if let Err(err) = result {
                    log_warn(&format!(
                        "{} worker lane terminated unexpectedly: {err}",
                        wc.job_kind
                    ));
                    all_ok = false;
                }
            }
            if all_ok {
                return Ok(());
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
        .map(|lane| run_polling_lane(cfg, pool.clone(), wc, lane, &process_fn))
        .collect();
    let results = futures_util::future::join_all(futs).await;
    for result in results {
        result?;
    }
    Ok(())
}
