use crate::crates::core::config::Config;
use crate::crates::core::logging::{log_info, log_warn};
use crate::crates::jobs::common::{enqueue_job, make_pool, purge_queue_safe};
use crate::crates::jobs::status::JobStatus;
use sqlx::PgPool;
use std::error::Error;
use uuid::Uuid;

use super::schema::ensure_schema;
use super::types::{IngestJob, IngestJobConfig, IngestSource, source_type_label, target_label};

pub async fn start_ingest_job(cfg: &Config, source: IngestSource) -> Result<Uuid, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;

    let job_config = IngestJobConfig {
        source: source.clone(),
        collection: cfg.collection.clone(),
    };
    let cfg_json = serde_json::to_value(&job_config)?;
    let source_type = source_type_label(&source);
    let target = target_label(&source);

    let id = Uuid::new_v4();
    sqlx::query(&format!(
        "INSERT INTO axon_ingest_jobs (id, status, source_type, target, config_json) \
         VALUES ($1, '{pending}', $2, $3, $4)",
        pending = JobStatus::Pending.as_str(),
    ))
    .bind(id)
    .bind(source_type)
    .bind(&target)
    .bind(cfg_json)
    .execute(&pool)
    .await?;

    if let Err(err) = enqueue_job(cfg, &cfg.ingest_queue, id).await {
        log_warn(&format!(
            "ingest enqueue failed for {id}; polling fallback will pick up: {err}"
        ));
    }

    log_info(&format!(
        "ingest job queued: id={id} source={source_type} target={target}"
    ));
    Ok(id)
}

pub async fn get_ingest_job(cfg: &Config, id: Uuid) -> Result<Option<IngestJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    Ok(sqlx::query_as::<_, IngestJob>(
        "SELECT id,status,source_type,target,created_at,updated_at,started_at,finished_at,\
         error_text,result_json,config_json FROM axon_ingest_jobs WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?)
}

pub async fn list_ingest_jobs(cfg: &Config, limit: i64) -> Result<Vec<IngestJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    Ok(sqlx::query_as::<_, IngestJob>(
        "SELECT id,status,source_type,target,created_at,updated_at,started_at,finished_at,\
         error_text,result_json,config_json FROM axon_ingest_jobs ORDER BY created_at DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(&pool)
    .await?)
}

pub async fn cancel_ingest_job(cfg: &Config, id: Uuid) -> Result<bool, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query(&format!(
        "UPDATE axon_ingest_jobs SET status='{canceled}',updated_at=NOW(),finished_at=NOW() \
         WHERE id=$1 AND status IN ('{pending}','{running}')",
        canceled = JobStatus::Canceled.as_str(),
        pending = JobStatus::Pending.as_str(),
        running = JobStatus::Running.as_str(),
    ))
    .bind(id)
    .execute(&pool)
    .await?
    .rows_affected();
    Ok(rows > 0)
}

pub async fn cleanup_ingest_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    Ok(sqlx::query(&format!(
        "DELETE FROM axon_ingest_jobs WHERE status IN ('{failed}','{canceled}') \
         OR (status = '{completed}' AND finished_at < NOW() - INTERVAL '30 days')",
        failed = JobStatus::Failed.as_str(),
        canceled = JobStatus::Canceled.as_str(),
        completed = JobStatus::Completed.as_str(),
    ))
    .execute(&pool)
    .await?
    .rows_affected())
}

pub async fn clear_ingest_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query("DELETE FROM axon_ingest_jobs")
        .execute(&pool)
        .await?
        .rows_affected();
    let _ = purge_queue_safe(cfg, &cfg.ingest_queue).await;
    Ok(rows)
}

pub(crate) async fn mark_completed(pool: &PgPool, id: Uuid, chunks: usize) {
    use crate::crates::core::logging::log_warn;

    match sqlx::query(&format!(
        "UPDATE axon_ingest_jobs SET status='{completed}',updated_at=NOW(),\
         finished_at=NOW(),result_json=$2 WHERE id=$1 AND status='{running}'",
        completed = JobStatus::Completed.as_str(),
        running = JobStatus::Running.as_str(),
    ))
    .bind(id)
    .bind(serde_json::json!({"chunks_embedded": chunks}))
    .execute(pool)
    .await
    {
        Ok(done) => {
            if done.rows_affected() == 0 {
                log_warn(&format!(
                    "command=ingest_worker completion_update_skipped job_id={id} reason=not_running_state"
                ));
            }
        }
        Err(e) => {
            log_warn(&format!(
                "command=ingest_worker mark_completed_failed job_id={id} err={e}"
            ));
        }
    }
}
