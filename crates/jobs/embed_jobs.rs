use crate::crates::core::config::Config;
use crate::crates::core::health::redis_healthy;
use crate::crates::core::logging::{log_done, log_info, log_warn};
use crate::crates::jobs::common::{
    enqueue_job, make_pool, mark_job_failed, open_amqp_connection_and_channel, purge_queue_safe,
    reclaim_stale_running_jobs, JobTable,
};
use crate::crates::jobs::status::JobStatus;
use crate::crates::vector::ops::{embed_path_native_with_progress, EmbedProgress};
use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::error::Error;
use tokio;
use uuid::Uuid;

static SCHEMA_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

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
            status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed', 'canceled')),
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
        "CREATE INDEX IF NOT EXISTS idx_axon_embed_jobs_pending ON axon_embed_jobs(created_at ASC) WHERE status = 'pending'"
    )
    .execute(pool)
    .await?;

    // Add CHECK constraint to existing tables (idempotent via IF NOT EXISTS pattern).
    sqlx::query(
        r#"DO $$ BEGIN
            ALTER TABLE axon_embed_jobs ADD CONSTRAINT axon_embed_jobs_status_check
                CHECK (status IN ('pending', 'running', 'completed', 'failed', 'canceled'));
        EXCEPTION WHEN duplicate_object THEN NULL;
        END $$"#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Start an embed job, creating a new pool for this call (CLI / one-shot use).
pub async fn start_embed_job(cfg: &Config, input: &str) -> Result<Uuid, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    start_embed_job_with_pool(&pool, cfg, input).await
}

/// Start an embed job using a pre-existing pool. Used by workers that already
/// hold a long-lived pool to avoid per-call TCP connection churn.
pub(crate) async fn start_embed_job_with_pool(
    pool: &PgPool,
    cfg: &Config,
    input: &str,
) -> Result<Uuid, Box<dyn Error>> {
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(pool).await?;
        let _ = SCHEMA_INIT.set(());
    }

    let cfg_json = serde_json::to_value(EmbedJobConfig {
        collection: cfg.collection.clone(),
    })?;
    if let Some(existing_id) = sqlx::query_scalar::<_, Uuid>(&format!(
        r#"
        SELECT id
        FROM axon_embed_jobs
        WHERE status IN ('{pending}','{running}')
          AND input_text = $1
          AND config_json = $2
        ORDER BY created_at DESC
        LIMIT 1
        "#,
        pending = JobStatus::Pending.as_str(),
        running = JobStatus::Running.as_str(),
    ))
    .bind(input)
    .bind(cfg_json.clone())
    .fetch_optional(pool)
    .await?
    {
        log_info(&format!(
            "embed dedupe hit: reusing active job {} for input={}",
            existing_id, input
        ));
        return Ok(existing_id);
    }
    let id = Uuid::new_v4();
    sqlx::query(&format!(
        r#"INSERT INTO axon_embed_jobs (id, status, input_text, config_json) VALUES ($1, '{pending}', $2, $3)"#,
        pending = JobStatus::Pending.as_str(),
    ))
    .bind(id)
    .bind(input)
    .bind(cfg_json)
    .execute(pool)
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
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    Ok(sqlx::query_as::<_, EmbedJob>(
        r#"SELECT id,status,created_at,updated_at,started_at,finished_at,error_text,input_text,result_json FROM axon_embed_jobs WHERE id=$1"#,
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?)
}

pub async fn list_embed_jobs(cfg: &Config, limit: i64) -> Result<Vec<EmbedJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    Ok(sqlx::query_as::<_, EmbedJob>(
        r#"SELECT id,status,created_at,updated_at,started_at,finished_at,error_text,input_text,result_json FROM axon_embed_jobs ORDER BY created_at DESC LIMIT $1"#,
    )
    .bind(limit)
    .fetch_all(&pool)
    .await?)
}

pub async fn cancel_embed_job(cfg: &Config, id: Uuid) -> Result<bool, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    let rows = sqlx::query(&format!(
        "UPDATE axon_embed_jobs SET status='{canceled}',updated_at=NOW(),finished_at=NOW() WHERE id=$1 AND status IN ('{pending}','{running}')",
        canceled = JobStatus::Canceled.as_str(),
        pending = JobStatus::Pending.as_str(),
        running = JobStatus::Running.as_str(),
    ))
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
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    let mut total = 0u64;
    loop {
        let deleted = sqlx::query(&format!(
            "DELETE FROM axon_embed_jobs WHERE id IN (
                SELECT id FROM axon_embed_jobs
                WHERE status IN ('{failed}','{canceled}')
                LIMIT 1000
            )",
            failed = JobStatus::Failed.as_str(),
            canceled = JobStatus::Canceled.as_str(),
        ))
        .execute(&pool)
        .await?
        .rows_affected();
        total += deleted;
        if deleted == 0 {
            break;
        }
    }
    Ok(total)
}

pub async fn clear_embed_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    let rows = sqlx::query("DELETE FROM axon_embed_jobs")
        .execute(&pool)
        .await?
        .rows_affected();
    let _ = purge_queue_safe(cfg, &cfg.embed_queue).await;
    Ok(rows)
}

/// Check if the embed job has been canceled via Redis. Returns `true` if a cancel
/// key is present and the job has been marked canceled in the DB, `false` otherwise.
/// On Redis failure, logs a warning and returns `false` (non-cancellation path).
async fn check_embed_canceled(
    cfg: &Config,
    pool: &PgPool,
    id: Uuid,
) -> Result<bool, Box<dyn Error>> {
    let cancel_key = format!("axon:embed:cancel:{id}");
    let cancel_value: Option<String> = match redis::Client::open(cfg.redis_url.clone()) {
        Err(e) => {
            log_warn(&format!("embed cancel redis client failed for {id}: {e}"));
            None
        }
        Ok(redis_client) => match redis_client.get_multiplexed_async_connection().await {
            Err(e) => {
                log_warn(&format!("embed cancel redis connect failed for {id}: {e}"));
                None
            }
            Ok(mut conn) => match conn.get::<_, Option<String>>(&cancel_key).await {
                Ok(v) => v,
                Err(e) => {
                    log_warn(&format!("embed cancel check failed for {id}: {e}"));
                    None
                }
            },
        },
    };
    if cancel_value.is_none() {
        return Ok(false);
    }
    sqlx::query(&format!(
        "UPDATE axon_embed_jobs SET status='{canceled}',updated_at=NOW(),finished_at=NOW() WHERE id=$1",
        canceled = JobStatus::Canceled.as_str(),
    ))
    .bind(id)
    .execute(pool)
    .await?;
    Ok(true)
}

/// Run the embed operation and return the result JSON. Spawns a progress task
/// to stream intermediate updates to the DB while the embed runs.
async fn run_embed_core(
    cfg: &Config,
    pool: &PgPool,
    id: Uuid,
    input_text: String,
    collection: String,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel::<EmbedProgress>(256);
    let progress_pool = pool.clone();
    let progress_task = tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            let progress_json = serde_json::json!({
                "phase": "embedding",
                "docs_total": progress.docs_total,
                "docs_completed": progress.docs_completed,
                "chunks_embedded": progress.chunks_embedded,
            });
            let _ = sqlx::query(&format!(
                "UPDATE axon_embed_jobs SET updated_at=NOW(), result_json=$2 WHERE id=$1 AND status='{running}'",
                running = JobStatus::Running.as_str(),
            ))
            .bind(id)
            .bind(progress_json)
            .execute(&progress_pool)
            .await;
        }
    });
    let mut embed_cfg = cfg.clone();
    embed_cfg.collection = collection.clone();
    let summary =
        embed_path_native_with_progress(&embed_cfg, &input_text, Some(progress_tx)).await?;
    if let Err(err) = progress_task.await {
        log_warn(&format!(
            "embed progress_task panicked for job {id}: {err:?}"
        ));
    }
    Ok(serde_json::json!({
        "input": input_text,
        "collection": collection,
        "docs_embedded": summary.docs_embedded,
        "chunks_embedded": summary.chunks_embedded,
        "source": "rust"
    }))
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
        log_info(&format!(
            "embed worker started job {id} input={input_preview}"
        ));
        if check_embed_canceled(cfg, pool, id).await? {
            return Ok(None);
        }
        let job_cfg: EmbedJobConfig = serde_json::from_value(cfg_json)?;
        let result = run_embed_core(cfg, pool, id, input_text, job_cfg.collection).await?;
        Ok(Some(result))
    }
    .await;
    // Convert Box<dyn Error> to String before the match so no !Send type
    // is held across any await inside the match arms (tokio::spawn Send bound).
    let run_result = run_result.map_err(|e| e.to_string());

    match run_result {
        Ok(Some(result_json)) => {
            sqlx::query(&format!(
                "UPDATE axon_embed_jobs SET status='{completed}',updated_at=NOW(),finished_at=NOW(),result_json=$2,error_text=NULL WHERE id=$1 AND status='{running}'",
                completed = JobStatus::Completed.as_str(),
                running = JobStatus::Running.as_str(),
            ))
            .bind(id)
            .bind(result_json)
            .execute(pool)
            .await?;
            log_done(&format!("worker completed embed job {id}"));
        }
        Ok(None) => {}
        Err(error_text) => {
            let _ = sqlx::query(&format!(
                "UPDATE axon_embed_jobs SET status='{failed}',updated_at=NOW(),finished_at=NOW(),error_text=$2 WHERE id=$1 AND status='{running}'",
                failed = JobStatus::Failed.as_str(),
                running = JobStatus::Running.as_str(),
            ))
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
    use crate::crates::jobs::worker_lane::{
        run_job_worker, validate_worker_env_vars, ProcessFn, WorkerConfig,
    };

    // Validate required environment variables before attempting any connections.
    // Exits with a clear error message if any are missing.
    validate_worker_env_vars();

    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }

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
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
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
