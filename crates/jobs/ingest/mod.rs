//! Ingest job schema uses advisory-lock DDL via `common::schema::begin_schema_migration_tx`.
//! See `common/schema.rs` for the canonical pattern.

mod ops;
mod process;
mod schema;
pub mod types;

#[cfg(test)]
mod tests;

use crate::crates::core::config::Config;
use crate::crates::jobs::common::{JobTable, make_pool, reclaim_stale_running_jobs};
use crate::crates::jobs::worker_lane::{ProcessFn, WorkerConfig, run_job_worker};
use std::error::Error;
use std::sync::Arc;

// Re-export all public types and functions to preserve the existing API.
pub use self::ops::{
    cancel_ingest_job, cleanup_ingest_jobs, clear_ingest_jobs, get_ingest_job, list_ingest_jobs,
    start_ingest_job,
};
pub use self::types::{IngestJob, IngestJobConfig, IngestSource};

use self::process::process_ingest_job;
use self::schema::ensure_schema;

const TABLE: JobTable = JobTable::Ingest;

pub async fn ingest_doctor(cfg: &Config) -> Result<serde_json::Value, Box<dyn Error>> {
    use crate::crates::core::health::redis_healthy;
    use crate::crates::jobs::common::open_amqp_channel;

    let (pg_ok, amqp_result, redis_ok) = tokio::join!(
        async { make_pool(cfg).await.is_ok() },
        open_amqp_channel(cfg, &cfg.ingest_queue),
        redis_healthy(&cfg.redis_url),
    );
    let amqp_ok = match amqp_result {
        Ok(ch) => {
            let _ = ch.close(0, "probe").await;
            true
        }
        Err(_) => false,
    };
    Ok(serde_json::json!({
        "postgres_ok": pg_ok,
        "amqp_ok": amqp_ok,
        "redis_ok": redis_ok,
        "queue": cfg.ingest_queue,
        "all_ok": pg_ok && amqp_ok && redis_ok
    }))
}

pub async fn run_ingest_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;

    let wc = WorkerConfig {
        table: TABLE,
        queue_name: cfg.ingest_queue.clone(),
        job_kind: "ingest",
        consumer_tag_prefix: "ingest-worker",
        lane_count: std::env::var("AXON_INGEST_LANES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2),
    };

    let process_fn: ProcessFn =
        Arc::new(|cfg, pool, id| Box::pin(process_ingest_job(cfg, pool, id)));

    run_job_worker(cfg, pool, &wc, process_fn).await
}

pub async fn recover_stale_ingest_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let stats = reclaim_stale_running_jobs(
        &pool,
        TABLE,
        "ingest",
        cfg.watchdog_stale_timeout_secs,
        cfg.watchdog_confirm_secs,
        "manual",
    )
    .await?;
    Ok(stats.reclaimed_jobs)
}
