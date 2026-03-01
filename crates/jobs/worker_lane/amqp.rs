use crate::crates::core::config::Config;
use crate::crates::core::logging::{log_info, log_warn};
use crate::crates::jobs::common::open_amqp_connection_and_channel;
use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use lapin::options::{BasicConsumeOptions, BasicQosOptions};
use lapin::types::FieldTable;
use sqlx::PgPool;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use super::delivery::claim_delivery;
use super::{ProcessFn, STALE_SWEEP_INTERVAL_SECS, WorkerConfig, sweep_stale_jobs};

/// Open an AMQP connection, set QoS, declare a consumer, and log startup.
/// Returns `(Connection, Channel, Consumer)` ready to receive deliveries.
async fn setup_amqp_consumer(
    cfg: &Config,
    wc: &WorkerConfig,
    lane: usize,
) -> Result<(lapin::Connection, lapin::Channel, lapin::Consumer), Box<dyn std::error::Error>> {
    let (conn, ch) = open_amqp_connection_and_channel(cfg, &wc.queue_name).await?;

    // Tell the broker to only push one unacked message at a time per consumer,
    // preventing a single lane from buffering more work than it can process.
    ch.basic_qos(1, BasicQosOptions::default()).await?;

    let tag = format!("{}-{lane}", wc.consumer_tag_prefix);
    let consumer = ch
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

    Ok((conn, ch, consumer))
}

/// Generic AMQP consumer lane. Listens for job IDs on the queue, claims them,
/// and dispatches to `process_fn` concurrently using `FuturesUnordered` with a
/// semaphore for backpressure. Runs stale sweeps on idle timeout.
pub(crate) async fn run_amqp_lane(
    cfg: &Config,
    pool: PgPool,
    wc: &WorkerConfig,
    lane: usize,
    process_fn: &ProcessFn,
    semaphore: Arc<tokio::sync::Semaphore>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, ch, mut consumer) = setup_amqp_consumer(cfg, wc, lane).await?;

    // ProcessFn returns !Send futures; the lane runs on a single task so Send
    // is not required.
    let mut inflight: FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>> = FuturesUnordered::new();

    // Sweep interval used in the full-capacity backpressure path so that
    // watchdog sweeps keep firing even when all semaphore permits are held.
    let mut sweep_interval = tokio::time::interval(Duration::from_secs(STALE_SWEEP_INTERVAL_SECS));
    sweep_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    sweep_interval.tick().await; // consume the immediate first tick

    loop {
        // If all permits are consumed, block until at least one in-flight job
        // completes OR the sweep interval fires.  Without the select! here,
        // sweeps stop firing for the entire duration of any saturated burst,
        // which can span hours for long-running jobs.
        if semaphore.available_permits() == 0 && !inflight.is_empty() {
            tokio::select! {
                _ = inflight.next() => {}
                _ = sweep_interval.tick() => {
                    sweep_stale_jobs(cfg, &pool, wc, "amqp", lane).await;
                }
            }
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

        if let Some(job_fut) =
            claim_delivery(delivery, cfg, &pool, wc, lane, process_fn, &semaphore).await?
        {
            inflight.push(job_fut);
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
