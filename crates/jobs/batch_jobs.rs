use crate::crates::core::config::Config;
use crate::crates::core::content::{to_markdown, url_to_filename};
use crate::crates::core::health::redis_healthy;
use crate::crates::core::http::{build_client, fetch_html};
use crate::crates::core::logging::{log_done, log_info, log_warn};
use crate::crates::jobs::common::{
    enqueue_job, make_pool, mark_job_failed, open_amqp_channel, purge_queue_safe,
    reclaim_stale_running_jobs, JobTable,
};
use crate::crates::jobs::status::JobStatus;
use crate::crates::vector::ops::embed_path_native;
use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::error::Error;
use std::path::PathBuf;
use uuid::Uuid;

mod queue_injection;
pub(crate) use queue_injection::apply_queue_injection_with_pool;
pub use queue_injection::{
    apply_queue_injection, evaluate_queue_injection, ExtractionObservability, InjectionCandidate,
    QueueInjectionDecision, QueueInjectionEvaluation, QueueInjectionRule, RuleSelectionStats,
};

static SCHEMA_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

const TABLE: JobTable = JobTable::Batch;
const WORKER_CONCURRENCY: usize = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BatchJobConfig {
    embed: bool,
    collection: String,
    output_dir: String,
    extraction_prompt: Option<String>,
}

#[derive(Debug, FromRow, Serialize)]
pub struct BatchJob {
    pub id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_text: Option<String>,
    pub urls_json: serde_json::Value,
    pub result_json: Option<serde_json::Value>,
}

async fn ensure_schema(pool: &PgPool) -> Result<(), Box<dyn Error>> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS axon_batch_jobs (
            id UUID PRIMARY KEY,
            status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed', 'canceled')),
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            started_at TIMESTAMPTZ,
            finished_at TIMESTAMPTZ,
            error_text TEXT,
            urls_json JSONB NOT NULL,
            result_json JSONB,
            config_json JSONB NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Composite partial index for claim_next_pending: WHERE status='pending' ORDER BY created_at
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_axon_batch_jobs_pending ON axon_batch_jobs(created_at ASC) WHERE status = 'pending'"
    )
    .execute(pool)
    .await?;

    // Add CHECK constraint to existing tables (idempotent via IF NOT EXISTS pattern).
    sqlx::query(
        r#"DO $$ BEGIN
            ALTER TABLE axon_batch_jobs ADD CONSTRAINT axon_batch_jobs_status_check
                CHECK (status IN ('pending', 'running', 'completed', 'failed', 'canceled'));
        EXCEPTION WHEN duplicate_object THEN NULL;
        END $$"#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn start_batch_job(cfg: &Config, urls: &[String]) -> Result<Uuid, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }

    let urls_json = serde_json::to_value(urls)?;
    let cfg_json = serde_json::to_value(BatchJobConfig {
        embed: cfg.embed,
        collection: cfg.collection.clone(),
        output_dir: cfg.output_dir.to_string_lossy().to_string(),
        extraction_prompt: cfg.query.clone(),
    })?;
    if let Some(existing_id) = sqlx::query_scalar::<_, Uuid>(&format!(
        r#"
        SELECT id
        FROM axon_batch_jobs
        WHERE status IN ('{pending}','{running}')
          AND urls_json = $1
          AND config_json = $2
        ORDER BY created_at DESC
        LIMIT 1
        "#,
        pending = JobStatus::Pending.as_str(),
        running = JobStatus::Running.as_str(),
    ))
    .bind(urls_json.clone())
    .bind(cfg_json.clone())
    .fetch_optional(&pool)
    .await?
    {
        log_info(&format!(
            "batch dedupe hit: reusing active job {} for {} urls",
            existing_id,
            urls.len()
        ));
        return Ok(existing_id);
    }
    let id = Uuid::new_v4();

    sqlx::query(&format!(
        r#"INSERT INTO axon_batch_jobs (id, status, urls_json, config_json) VALUES ($1, '{pending}', $2, $3)"#,
        pending = JobStatus::Pending.as_str(),
    ))
    .bind(id)
    .bind(urls_json)
    .bind(cfg_json)
    .execute(&pool)
    .await?;

    if let Err(err) = enqueue_job(cfg, &cfg.batch_queue, id).await {
        log_warn(&format!(
            "batch enqueue failed for {id}; polling fallback will pick up: {err}"
        ));
    }

    Ok(id)
}

pub async fn get_batch_job(cfg: &Config, id: Uuid) -> Result<Option<BatchJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    Ok(sqlx::query_as::<_, BatchJob>(
        r#"SELECT id,status,created_at,updated_at,started_at,finished_at,error_text,urls_json,result_json FROM axon_batch_jobs WHERE id=$1"#,
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?)
}

pub async fn list_batch_jobs(cfg: &Config, limit: i64) -> Result<Vec<BatchJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    Ok(sqlx::query_as::<_, BatchJob>(
        r#"SELECT id,status,created_at,updated_at,started_at,finished_at,error_text,urls_json,result_json FROM axon_batch_jobs ORDER BY created_at DESC LIMIT $1"#,
    )
    .bind(limit)
    .fetch_all(&pool)
    .await?)
}

pub async fn cancel_batch_job(cfg: &Config, id: Uuid) -> Result<bool, Box<dyn Error>> {
    maintenance::cancel_batch_job(cfg, id).await
}

pub async fn cleanup_batch_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    maintenance::cleanup_batch_jobs(cfg).await
}

pub async fn clear_batch_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    maintenance::clear_batch_jobs(cfg).await
}

mod maintenance;
mod worker;

pub async fn run_batch_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    worker::run_batch_worker(cfg).await
}

pub async fn recover_stale_batch_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    maintenance::recover_stale_batch_jobs(cfg).await
}

pub async fn batch_doctor(cfg: &Config) -> Result<serde_json::Value, Box<dyn Error>> {
    maintenance::batch_doctor(cfg).await
}
#[cfg(test)]
mod tests;
