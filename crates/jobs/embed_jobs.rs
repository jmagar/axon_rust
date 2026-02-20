use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::health::redis_healthy;
use crate::axon_cli::crates::core::logging::{log_done, log_info, log_warn};
use crate::axon_cli::crates::jobs::common::{
    enqueue_job, make_pool, mark_job_failed, open_amqp_channel, open_amqp_connection_and_channel,
    reclaim_stale_running_jobs, JobTable,
};
use crate::axon_cli::crates::vector::ops::{embed_path_native_with_progress, EmbedProgress};
use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use spider::tokio;
use sqlx::{FromRow, PgPool};
use std::error::Error;
use uuid::Uuid;

const TABLE: JobTable = JobTable::Embed;
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

    // Composite partial index for claim_next_pending: WHERE status='pending' ORDER BY created_at
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_axon_embed_jobs_pending ON axon_embed_jobs(status, created_at ASC) WHERE status = 'pending'"
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
        let input_preview: String = input_text.chars().take(80).collect();
        log_info(&format!("embed worker started job {id} input={input_preview}"));

        let redis_client = redis::Client::open(cfg.redis_url.clone())?;
        let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;
        let cancel_key = format!("axon:embed:cancel:{id}");
        let cancel_before: Option<String> = redis_conn
            .get(&cancel_key)
            .await
            .map_err(|e| format!("failed to check embed cancellation key {cancel_key}: {e}"))?;
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
    // Convert Box<dyn Error> to String before the match so no !Send type
    // is held across any await inside the match arms (tokio::spawn Send bound).
    let run_result = run_result.map_err(|e| e.to_string());

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
        Err(error_text) => {
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

async fn process_claimed_embed_job(cfg: Config, pool: PgPool, id: Uuid) {
    let fail_msg = match process_embed_job(&cfg, &pool, id).await {
        Ok(()) => None,
        Err(err) => Some(err.to_string()),
    };
    if let Some(error_text) = fail_msg {
        mark_job_failed(&pool, TABLE, id, &error_text).await;
        log_warn(&format!("worker failed embed job {id}: {error_text}"));
    }
}

pub async fn run_embed_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    use crate::axon_cli::crates::jobs::worker_lane::{run_job_worker, ProcessFn, WorkerConfig};

    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;

    let wc = WorkerConfig {
        table: TABLE,
        queue_name: cfg.embed_queue.clone(),
        job_kind: "embed",
        consumer_tag_prefix: "axon-rust-embed-worker",
        lane_count: WORKER_CONCURRENCY,
    };

    let process_fn: ProcessFn =
        std::sync::Arc::new(|cfg, pool, id| Box::pin(process_claimed_embed_job(cfg, pool, id)));

    run_job_worker(cfg, pool, &wc, process_fn).await
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
    let amqp_ok = match open_amqp_connection_and_channel(cfg, &cfg.embed_queue).await {
        Ok((conn, ch)) => {
            let _ = ch.close(0, "probe").await;
            let _ = conn.close(200, "probe").await;
            true
        }
        Err(_) => false,
    };
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
mod tests;
