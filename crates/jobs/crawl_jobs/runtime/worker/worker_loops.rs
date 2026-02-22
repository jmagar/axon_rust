use crate::crates::core::config::Config;
use crate::crates::core::logging::{log_info, log_warn};
use crate::crates::jobs::common::{
    claim_next_pending, claim_pending_by_id, make_pool, mark_job_failed,
    open_amqp_connection_and_channel, reclaim_stale_running_jobs as generic_reclaim,
    WatchdogSweepStats,
};
use futures_util::StreamExt;
use lapin::options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions};
use lapin::types::FieldTable;
use spider::tokio;
use sqlx::PgPool;
use std::error::Error;
use std::time::{Duration, Instant};
use uuid::Uuid;

use super::super::{
    ensure_schema, CrawlWatchdogSweepStats, STALE_SWEEP_INTERVAL_SECS, TABLE, WORKER_CONCURRENCY,
};
use super::worker_process::process_job;

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
    // Use join! so a lane failure does not abruptly cancel the sibling lane
    // mid-job (which would leave jobs stuck in 'running' until the watchdog reclaims them).
    let (r1, r2) = tokio::join!(
        run_worker_polling_lane(cfg, pool, 1),
        run_worker_polling_lane(cfg, pool, 2)
    );
    r1?;
    r2?;
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
            match reclaim_stale_running_jobs(
                pool,
                lane,
                cfg.watchdog_stale_timeout_secs,
                cfg.watchdog_confirm_secs,
            )
            .await
            {
                Ok(stats) => {
                    if stats.stale_candidates > 0 || stats.reclaimed_jobs > 0 {
                        log_info(&format!(
                            "watchdog crawl sweep lane={} candidates={} marked={} reclaimed={}",
                            lane,
                            stats.stale_candidates,
                            stats.marked_candidates,
                            stats.reclaimed_jobs
                        ));
                    }
                }
                Err(err) => log_warn(&format!("watchdog sweep failed (lane={}): {}", lane, err)),
            }
            last_sweep = Instant::now();
        }
        if let Some(job_id) = claim_next_pending(pool, TABLE).await? {
            if let Err(err) = process_job(cfg, pool, job_id).await {
                let error_text = err.to_string();
                mark_job_failed(pool, TABLE, job_id, &error_text).await;
                log_warn(&format!("worker failed crawl job {job_id}: {error_text}"));
            }
        } else {
            tokio::time::sleep(Duration::from_millis(800)).await;
        }
    }
}

async fn handle_crawl_delivery(
    cfg: &Config,
    pool: &PgPool,
    delivery: lapin::message::Delivery,
    lane: usize,
) {
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
        return;
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
            if let Err(err) = process_job(cfg, pool, job_id).await {
                let error_text = err.to_string();
                mark_job_failed(pool, TABLE, job_id, &error_text).await;
                log_warn(&format!("worker failed crawl job {job_id}: {error_text}"));
            }
        }
        Ok(false) => {
            if let Err(err) = delivery.ack(BasicAckOptions::default()).await {
                log_warn(&format!(
                    "failed to ack already-claimed crawl delivery (lane={lane}): {err}"
                ));
            }
        }
        Err(err) => {
            log_warn(&format!(
                "failed to claim crawl job {job_id} (lane={lane}); nacking for retry: {err}"
            ));
            // Brief pause before requeuing so a persistent DB failure (e.g. PG
            // restart) doesn't spin-loop at full speed consuming CPU and flooding logs.
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
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
        }
    }
}

async fn run_amqp_worker_lane(
    cfg: &Config,
    pool: &PgPool,
    lane: usize,
) -> Result<(), Box<dyn Error>> {
    // Hold `_conn` in scope for the entire consumer loop. open_amqp_connection_and_channel
    // returns the Connection; dropping it would close the backing TCP connection
    // and kill the consumer stream. Keeping _conn alive prevents that.
    let (_conn, ch) = open_amqp_connection_and_channel(cfg, &cfg.crawl_queue).await?;
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
        let msg = tokio::select! {
            msg = consumer.next() => match msg {
                Some(msg) => msg,
                None => break,
            },
            _ = heartbeat_interval.tick() => {
                log_info(&format!("crawl worker heartbeat lane={} alive", lane));
                continue;
            },
            _ = sweep_interval.tick() => {
                match reclaim_stale_running_jobs(
                    pool,
                    lane,
                    cfg.watchdog_stale_timeout_secs,
                    cfg.watchdog_confirm_secs,
                )
                .await
                {
                    Ok(stats) => {
                        if stats.stale_candidates > 0 || stats.reclaimed_jobs > 0 {
                            log_info(&format!(
                                "watchdog crawl sweep lane={} candidates={} marked={} reclaimed={}",
                                lane,
                                stats.stale_candidates,
                                stats.marked_candidates,
                                stats.reclaimed_jobs
                            ));
                        }
                    }
                    Err(err) => {
                        log_warn(&format!("watchdog sweep failed (lane={}): {}", lane, err))
                    }
                }
                continue;
            }
        };
        let delivery = match msg {
            Ok(d) => d,
            Err(err) => {
                log_warn(&format!("consumer error (lane={lane}): {err}"));
                continue;
            }
        };

        handle_crawl_delivery(cfg, pool, delivery, lane).await;
    }

    Err(format!("crawl worker consumer stream ended unexpectedly (lane={lane})").into())
}

pub(crate) async fn run_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
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
    if WORKER_CONCURRENCY <= 1 {
        return run_amqp_worker_lane(cfg, &pool, 1).await;
    }
    // Use join! so a lane failure does not abruptly cancel the sibling lane
    // mid-job (which would leave jobs stuck in 'running' until the watchdog reclaims them).
    let (r1, r2) = tokio::join!(
        run_amqp_worker_lane(cfg, &pool, 1),
        run_amqp_worker_lane(cfg, &pool, 2)
    );
    r1?;
    r2?;
    Ok(())
}
