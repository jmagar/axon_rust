use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::health::redis_healthy;
use crate::axon_cli::crates::core::logging::{log_done, log_info, log_warn};
use crate::axon_cli::crates::jobs::common::{
    claim_next_pending, claim_pending_by_id, enqueue_job, make_pool, mark_job_failed,
    open_amqp_channel, reclaim_stale_running_jobs, JobTable,
};
use crate::axon_cli::crates::vector::ops::{embed_path_native_with_progress, EmbedProgress};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use lapin::options::{BasicAckOptions, BasicConsumeOptions};
use lapin::types::FieldTable;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use spider::tokio;
use sqlx::{FromRow, PgPool};
use std::error::Error;
use std::time::{Duration, Instant};
use uuid::Uuid;

const TABLE: JobTable = JobTable::Embed;
const STALE_SWEEP_INTERVAL_SECS: u64 = 30;
const WORKER_CONCURRENCY: usize = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EmbedJobConfig {
    collection: String,
}

#[derive(Debug, FromRow, Serialize)]
pub struct EmbedJob {
    pub id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_text: Option<String>,
    pub input_text: String,
    pub result_json: Option<serde_json::Value>,
}

async fn ensure_schema(pool: &PgPool) -> Result<(), Box<dyn Error>> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS axon_embed_jobs (
            id UUID PRIMARY KEY,
            status TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            started_at TIMESTAMPTZ,
            finished_at TIMESTAMPTZ,
            error_text TEXT,
            input_text TEXT NOT NULL,
            result_json JSONB,
            config_json JSONB NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn start_embed_job(cfg: &Config, input: &str) -> Result<Uuid, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;

    let cfg_json = serde_json::to_value(EmbedJobConfig {
        collection: cfg.collection.clone(),
    })?;
    if let Some(existing_id) = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id
        FROM axon_embed_jobs
        WHERE status IN ('pending','running')
          AND input_text = $1
          AND config_json = $2
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(input)
    .bind(cfg_json.clone())
    .fetch_optional(&pool)
    .await?
    {
        log_info(&format!(
            "embed dedupe hit: reusing active job {} for input={}",
            existing_id, input
        ));
        return Ok(existing_id);
    }
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO axon_embed_jobs (id, status, input_text, config_json) VALUES ($1, 'pending', $2, $3)"#,
    )
    .bind(id)
    .bind(input)
    .bind(cfg_json)
    .execute(&pool)
    .await?;

    if let Err(err) = enqueue_job(cfg, &cfg.embed_queue, id).await {
        log_warn(&format!(
            "embed enqueue failed for {id}; polling fallback will pick up: {err}"
        ));
    }

    Ok(id)
}

pub async fn get_embed_job(cfg: &Config, id: Uuid) -> Result<Option<EmbedJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    Ok(sqlx::query_as::<_, EmbedJob>(
        r#"SELECT id,status,created_at,updated_at,started_at,finished_at,error_text,input_text,result_json FROM axon_embed_jobs WHERE id=$1"#,
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?)
}

pub async fn list_embed_jobs(cfg: &Config, limit: i64) -> Result<Vec<EmbedJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    Ok(sqlx::query_as::<_, EmbedJob>(
        r#"SELECT id,status,created_at,updated_at,started_at,finished_at,error_text,input_text,result_json FROM axon_embed_jobs ORDER BY created_at DESC LIMIT $1"#,
    )
    .bind(limit)
    .fetch_all(&pool)
    .await?)
}

pub async fn cancel_embed_job(cfg: &Config, id: Uuid) -> Result<bool, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query("UPDATE axon_embed_jobs SET status='canceled',updated_at=NOW(),finished_at=NOW() WHERE id=$1 AND status IN ('pending','running')")
        .bind(id)
        .execute(&pool)
        .await?
        .rows_affected();

    let redis_client = redis::Client::open(cfg.redis_url.clone())?;
    let mut conn = redis_client.get_multiplexed_async_connection().await?;
    let key = format!("axon:embed:cancel:{id}");
    let _: () = conn.set_ex(key, "1", 86400).await?;
    Ok(rows > 0)
}

pub async fn cleanup_embed_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    Ok(
        sqlx::query("DELETE FROM axon_embed_jobs WHERE status IN ('failed','canceled')")
            .execute(&pool)
            .await?
            .rows_affected(),
    )
}

pub async fn clear_embed_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query("DELETE FROM axon_embed_jobs")
        .execute(&pool)
        .await?
        .rows_affected();
    if let Ok(ch) = open_amqp_channel(cfg, &cfg.embed_queue).await {
        let _ = ch
            .queue_purge(
                &cfg.embed_queue,
                lapin::options::QueuePurgeOptions::default(),
            )
            .await;
    }
    Ok(rows)
}

async fn process_embed_job(cfg: &Config, pool: &PgPool, id: Uuid) -> Result<(), Box<dyn Error>> {
    let run_result = async {
        let row = sqlx::query_as::<_, (String, serde_json::Value)>(
            "SELECT input_text, config_json FROM axon_embed_jobs WHERE id=$1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        let Some((input_text, cfg_json)) = row else {
            return Ok::<Option<serde_json::Value>, Box<dyn Error>>(None);
        };

        let redis_client = redis::Client::open(cfg.redis_url.clone())?;
        let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;
        let cancel_key = format!("axon:embed:cancel:{id}");
        let cancel_before: Option<String> = redis_conn.get(&cancel_key).await.ok();
        if cancel_before.is_some() {
            sqlx::query("UPDATE axon_embed_jobs SET status='canceled',updated_at=NOW(),finished_at=NOW() WHERE id=$1")
                .bind(id)
                .execute(pool)
                .await?;
            return Ok(None);
        }

        let job_cfg: EmbedJobConfig = serde_json::from_value(cfg_json)?;
        let mut embed_cfg = cfg.clone();
        embed_cfg.collection = job_cfg.collection.clone();
        let (progress_tx, mut progress_rx) =
            tokio::sync::mpsc::unbounded_channel::<EmbedProgress>();
        let progress_pool = pool.clone();
        let progress_job_id = id;
        let progress_task = tokio::spawn(async move {
            while let Some(progress) = progress_rx.recv().await {
                let progress_json = serde_json::json!({
                    "phase": "embedding",
                    "docs_total": progress.docs_total,
                    "docs_completed": progress.docs_completed,
                    "chunks_embedded": progress.chunks_embedded,
                });
                let _ = sqlx::query(
                    "UPDATE axon_embed_jobs SET updated_at=NOW(), result_json=$2 WHERE id=$1 AND status='running'",
                )
                .bind(progress_job_id)
                .bind(progress_json)
                .execute(&progress_pool)
                .await;
            }
        });

        let summary =
            embed_path_native_with_progress(&embed_cfg, &input_text, Some(progress_tx)).await?;
        if let Err(err) = progress_task.await {
            log_warn(&format!(
                "embed progress_task panicked for job {id}: {err:?}"
            ));
        }

        Ok(Some(serde_json::json!({
            "input": input_text,
            "collection": job_cfg.collection,
            "docs_embedded": summary.docs_embedded,
            "chunks_embedded": summary.chunks_embedded,
            "source": "rust"
        })))
    }
    .await;

    match run_result {
        Ok(Some(result_json)) => {
            sqlx::query(
                "UPDATE axon_embed_jobs SET status='completed',updated_at=NOW(),finished_at=NOW(),result_json=$2,error_text=NULL WHERE id=$1 AND status='running'",
            )
            .bind(id)
            .bind(result_json)
            .execute(pool)
            .await?;
            log_done(&format!("worker completed embed job {id}"));
        }
        Ok(None) => {}
        Err(err) => {
            let error_text = err.to_string();
            let _ = sqlx::query(
                "UPDATE axon_embed_jobs SET status='failed',updated_at=NOW(),finished_at=NOW(),error_text=$2 WHERE id=$1 AND status='running'",
            )
            .bind(id)
            .bind(error_text.clone())
            .execute(pool)
            .await;
            log_warn(&format!("worker failed embed job {id}: {error_text}"));
        }
    }

    Ok(())
}

pub async fn run_embed_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    match reclaim_stale_running_jobs(
        &pool,
        TABLE,
        "embed",
        cfg.watchdog_stale_timeout_secs,
        cfg.watchdog_confirm_secs,
        "startup",
    )
    .await
    {
        Ok(stats) => {
            if stats.stale_candidates > 0 || stats.reclaimed_jobs > 0 {
                log_info(&format!(
                    "watchdog embed startup sweep candidates={} marked={} reclaimed={}",
                    stats.stale_candidates, stats.marked_candidates, stats.reclaimed_jobs
                ));
            }
        }
        Err(err) => log_warn(&format!("watchdog embed startup sweep failed: {err}")),
    }

    let run_amqp_lane = |lane: usize| {
        let pool = pool.clone();
        async move {
            let ch = open_amqp_channel(cfg, &cfg.embed_queue).await?;
            let tag = format!("axon-rust-embed-worker-{lane}");
            let mut consumer = ch
                .basic_consume(
                    &cfg.embed_queue,
                    &tag,
                    BasicConsumeOptions::default(),
                    FieldTable::default(),
                )
                .await?;
            log_info(&format!(
                "embed worker lane={} listening on queue={} concurrency={}",
                lane, cfg.embed_queue, WORKER_CONCURRENCY
            ));
            loop {
                let msg = match tokio::time::timeout(
                    Duration::from_secs(STALE_SWEEP_INTERVAL_SECS),
                    consumer.next(),
                )
                .await
                {
                    Ok(Some(msg)) => msg,
                    Ok(None) => break,
                    Err(_) => {
                        if let Ok(stats) = reclaim_stale_running_jobs(
                            &pool,
                            TABLE,
                            "embed",
                            cfg.watchdog_stale_timeout_secs,
                            cfg.watchdog_confirm_secs,
                            "amqp",
                        )
                        .await
                        {
                            if stats.stale_candidates > 0 || stats.reclaimed_jobs > 0 {
                                log_info(&format!(
                                "watchdog embed sweep lane={} candidates={} marked={} reclaimed={}",
                                lane,
                                stats.stale_candidates,
                                stats.marked_candidates,
                                stats.reclaimed_jobs
                            ));
                            }
                        }
                        continue;
                    }
                };
                let delivery = match msg {
                    Ok(d) => d,
                    Err(_) => continue,
                };
                let parsed = std::str::from_utf8(&delivery.data)
                    .ok()
                    .and_then(|s| Uuid::parse_str(s.trim()).ok());
                if let Some(job_id) = parsed {
                    if claim_pending_by_id(&pool, TABLE, job_id)
                        .await
                        .unwrap_or(false)
                    {
                        if let Err(err) = process_embed_job(cfg, &pool, job_id).await {
                            let error_text = err.to_string();
                            mark_job_failed(&pool, TABLE, job_id, &error_text).await;
                            log_warn(&format!("worker failed embed job {job_id}: {error_text}"));
                        }
                    }
                }
                delivery.ack(BasicAckOptions::default()).await?;
            }
            Result::<(), Box<dyn Error>>::Ok(())
        }
    };

    let run_polling_lane = |lane: usize| {
        let pool = pool.clone();
        async move {
            log_info(&format!(
                "embed worker polling lane={} active queue={}",
                lane, cfg.embed_queue
            ));
            let mut last_sweep = Instant::now();
            loop {
                if last_sweep.elapsed() >= Duration::from_secs(STALE_SWEEP_INTERVAL_SECS) {
                    if let Ok(stats) = reclaim_stale_running_jobs(
                        &pool,
                        TABLE,
                        "embed",
                        cfg.watchdog_stale_timeout_secs,
                        cfg.watchdog_confirm_secs,
                        "polling",
                    )
                    .await
                    {
                        if stats.stale_candidates > 0 || stats.reclaimed_jobs > 0 {
                            log_info(&format!(
                                "watchdog embed sweep lane={} candidates={} marked={} reclaimed={}",
                                lane,
                                stats.stale_candidates,
                                stats.marked_candidates,
                                stats.reclaimed_jobs
                            ));
                        }
                    }
                    last_sweep = Instant::now();
                }
                if let Some(id) = claim_next_pending(&pool, TABLE).await? {
                    if let Err(err) = process_embed_job(cfg, &pool, id).await {
                        let error_text = err.to_string();
                        mark_job_failed(&pool, TABLE, id, &error_text).await;
                        log_warn(&format!("worker failed embed job {id}: {error_text}"));
                    }
                } else {
                    tokio::time::sleep(Duration::from_millis(800)).await;
                }
            }
            #[allow(unreachable_code)]
            Result::<(), Box<dyn Error>>::Ok(())
        }
    };

    if open_amqp_channel(cfg, &cfg.embed_queue).await.is_ok() {
        tokio::try_join!(run_amqp_lane(1), run_amqp_lane(2))?;
        return Ok(());
    }

    log_warn("amqp unavailable; running embed worker in postgres polling mode");
    tokio::try_join!(run_polling_lane(1), run_polling_lane(2))?;
    Ok(())
}

pub async fn recover_stale_embed_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let stats = reclaim_stale_running_jobs(
        &pool,
        TABLE,
        "embed",
        cfg.watchdog_stale_timeout_secs,
        cfg.watchdog_confirm_secs,
        "manual",
    )
    .await?;
    Ok(stats.reclaimed_jobs)
}

pub async fn embed_doctor(cfg: &Config) -> Result<serde_json::Value, Box<dyn Error>> {
    let pg_ok = make_pool(cfg).await.is_ok();
    let amqp_ok = open_amqp_channel(cfg, &cfg.embed_queue).await.is_ok();
    let redis_ok = redis_healthy(&cfg.redis_url).await;
    Ok(serde_json::json!({
        "postgres_ok": pg_ok,
        "amqp_ok": amqp_ok,
        "redis_ok": redis_ok,
        "tei_configured": !cfg.tei_url.is_empty(),
        "qdrant_configured": !cfg.qdrant_url.is_empty(),
        "all_ok": pg_ok && amqp_ok && redis_ok
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::axon_cli::crates::jobs::common::test_config;
    use chrono::{Duration, Utc};
    use std::env;
    use tokio::time::{sleep, timeout, Duration as TokioDuration};

    fn pg_url() -> Option<String> {
        env::var("AXON_TEST_PG_URL")
            .ok()
            .or_else(|| env::var("AXON_PG_URL").ok())
            .filter(|v| !v.trim().is_empty())
    }

    #[tokio::test]
    async fn embed_start_job_dedupes_active_pending_job() -> Result<(), Box<dyn Error>> {
        let Some(pg_url) = pg_url() else {
            return Ok(());
        };
        let cfg = test_config(&pg_url);
        let input = format!("embed-dedupe-{}", Uuid::new_v4());

        let first_id = start_embed_job(&cfg, &input).await?;
        let second_id = start_embed_job(&cfg, &input).await?;
        assert_eq!(first_id, second_id);

        let pool = make_pool(&cfg).await?;
        let _ = sqlx::query("DELETE FROM axon_embed_jobs WHERE id = $1")
            .bind(first_id)
            .execute(&pool)
            .await;
        Ok(())
    }

    #[tokio::test]
    async fn embed_recover_reclaims_confirmed_stale_running_job() -> Result<(), Box<dyn Error>> {
        let Some(pg_url) = pg_url() else {
            return Ok(());
        };
        let cfg = test_config(&pg_url);
        let input = format!("embed-recover-{}", Uuid::new_v4());
        let id = start_embed_job(&cfg, &input).await?;
        let pool = make_pool(&cfg).await?;

        sqlx::query(
            "UPDATE axon_embed_jobs SET status='running', updated_at=NOW() - INTERVAL '20 minutes' WHERE id=$1",
        )
        .bind(id)
        .execute(&pool)
        .await?;

        let observed_updated_at: DateTime<Utc> =
            sqlx::query_scalar("SELECT updated_at FROM axon_embed_jobs WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await?;
        let watchdog = serde_json::json!({
            "_watchdog": {
                "observed_updated_at": observed_updated_at.to_rfc3339(),
                "first_seen_stale_at": (Utc::now() - Duration::minutes(10)).to_rfc3339()
            }
        });
        sqlx::query("UPDATE axon_embed_jobs SET result_json=$2 WHERE id=$1")
            .bind(id)
            .bind(watchdog)
            .execute(&pool)
            .await?;

        let reclaimed = recover_stale_embed_jobs(&cfg).await?;
        assert!(reclaimed >= 1);

        let status: String = sqlx::query_scalar("SELECT status FROM axon_embed_jobs WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await?;
        assert_eq!(status, "failed");

        let _ = sqlx::query("DELETE FROM axon_embed_jobs WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await;
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn embed_worker_e2e_processes_pending_job_to_terminal_status(
    ) -> Result<(), Box<dyn Error>> {
        let Some(pg_url) = pg_url() else {
            return Ok(());
        };
        let cfg = test_config(&pg_url);
        let input = format!("embed-worker-e2e-{}", Uuid::new_v4());
        let id = start_embed_job(&cfg, &input).await?;

        let worker_cfg = cfg.clone();
        let worker = tokio::task::spawn_local(async move {
            let _ = run_embed_worker(&worker_cfg).await;
        });

        let pool = make_pool(&cfg).await?;
        let wait = timeout(TokioDuration::from_secs(8), async {
            loop {
                let status: Option<String> =
                    sqlx::query_scalar("SELECT status FROM axon_embed_jobs WHERE id=$1")
                        .bind(id)
                        .fetch_optional(&pool)
                        .await
                        .ok()
                        .flatten();
                if matches!(status.as_deref(), Some("completed" | "failed" | "canceled")) {
                    break;
                }
                sleep(TokioDuration::from_millis(100)).await;
            }
        })
        .await;
        worker.abort();
        let _ = worker.await;
        assert!(
            wait.is_ok(),
            "embed worker did not reach terminal state in time"
        );

        let status: String = sqlx::query_scalar("SELECT status FROM axon_embed_jobs WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await?;
        assert!(matches!(
            status.as_str(),
            "completed" | "failed" | "canceled"
        ));

        let _ = sqlx::query("DELETE FROM axon_embed_jobs WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await;
        Ok(())
    }
}
