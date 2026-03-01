use super::{RefreshSchedule, SCHEDULE_CLAIM_LEASE_SECS, ensure_schema_once};
use crate::crates::core::config::Config;
use crate::crates::core::logging::log_warn;
use crate::crates::jobs::common::{enqueue_job, make_pool};
use crate::crates::jobs::status::JobStatus;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::error::Error;
use uuid::Uuid;

use super::RefreshJobConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshScheduleCreate {
    pub name: String,
    pub seed_url: Option<String>,
    pub urls: Option<Vec<String>>,
    pub every_seconds: i64,
    pub enabled: bool,
    pub next_run_at: DateTime<Utc>,
}

pub async fn start_refresh_job(cfg: &Config, urls: &[String]) -> Result<Uuid, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema_once(&pool).await?;
    start_refresh_job_with_pool(&pool, cfg, urls, true).await
}

pub(crate) async fn start_refresh_job_with_pool(
    pool: &PgPool,
    cfg: &Config,
    urls: &[String],
    enqueue: bool,
) -> Result<Uuid, Box<dyn Error>> {
    let id = Uuid::new_v4();
    let urls_json = serde_json::to_value(urls)?;
    let cfg_json = serde_json::to_value(RefreshJobConfig {
        urls: urls.to_vec(),
        embed: cfg.embed,
        output_dir: cfg.output_dir.to_string_lossy().to_string(),
    })?;

    sqlx::query(&format!(
        "INSERT INTO axon_refresh_jobs (id, status, urls_json, config_json) VALUES ($1, '{pending}', $2, $3)",
        pending = JobStatus::Pending.as_str(),
    ))
    .bind(id)
    .bind(urls_json)
    .bind(cfg_json)
    .execute(pool)
    .await?;

    if enqueue {
        if let Err(err) = enqueue_job(cfg, &cfg.refresh_queue, id).await {
            log_warn(&format!(
                "refresh enqueue failed for {id}; polling fallback will pick up: {err}"
            ));
        }
    }

    Ok(id)
}

pub async fn create_refresh_schedule(
    cfg: &Config,
    schedule: &RefreshScheduleCreate,
) -> Result<RefreshSchedule, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema_once(&pool).await?;
    create_refresh_schedule_with_pool(&pool, schedule).await
}

pub(crate) async fn create_refresh_schedule_with_pool(
    pool: &PgPool,
    schedule: &RefreshScheduleCreate,
) -> Result<RefreshSchedule, Box<dyn Error>> {
    let urls_json = schedule
        .urls
        .as_ref()
        .map(serde_json::to_value)
        .transpose()?;
    let id = Uuid::new_v4();

    Ok(sqlx::query_as::<_, RefreshSchedule>(
        r#"
        INSERT INTO axon_refresh_schedules (
            id, name, seed_url, urls_json, every_seconds, enabled, next_run_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING
            id, name, seed_url, urls_json, every_seconds, enabled,
            next_run_at, last_run_at, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(&schedule.name)
    .bind(schedule.seed_url.as_deref())
    .bind(urls_json)
    .bind(schedule.every_seconds)
    .bind(schedule.enabled)
    .bind(schedule.next_run_at)
    .fetch_one(pool)
    .await?)
}

pub async fn list_refresh_schedules(
    cfg: &Config,
    limit: i64,
) -> Result<Vec<RefreshSchedule>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema_once(&pool).await?;
    list_refresh_schedules_with_pool(&pool, limit).await
}

pub(crate) async fn list_refresh_schedules_with_pool(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<RefreshSchedule>, Box<dyn Error>> {
    Ok(sqlx::query_as::<_, RefreshSchedule>(
        r#"
        SELECT
            id, name, seed_url, urls_json, every_seconds, enabled,
            next_run_at, last_run_at, created_at, updated_at
        FROM axon_refresh_schedules
        ORDER BY next_run_at ASC, created_at ASC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?)
}

pub async fn delete_refresh_schedule(cfg: &Config, name: &str) -> Result<bool, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema_once(&pool).await?;
    delete_refresh_schedule_with_pool(&pool, name).await
}

pub(crate) async fn delete_refresh_schedule_with_pool(
    pool: &PgPool,
    name: &str,
) -> Result<bool, Box<dyn Error>> {
    let rows = sqlx::query("DELETE FROM axon_refresh_schedules WHERE name = $1")
        .bind(name)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(rows > 0)
}

pub async fn set_refresh_schedule_enabled(
    cfg: &Config,
    name: &str,
    enabled: bool,
) -> Result<bool, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema_once(&pool).await?;
    set_refresh_schedule_enabled_with_pool(&pool, name, enabled).await
}

pub(crate) async fn set_refresh_schedule_enabled_with_pool(
    pool: &PgPool,
    name: &str,
    enabled: bool,
) -> Result<bool, Box<dyn Error>> {
    let rows = sqlx::query(
        "UPDATE axon_refresh_schedules SET enabled = $2, updated_at = NOW() WHERE name = $1",
    )
    .bind(name)
    .bind(enabled)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(rows > 0)
}

pub async fn claim_due_refresh_schedules(
    cfg: &Config,
    limit: i64,
) -> Result<Vec<RefreshSchedule>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema_once(&pool).await?;
    claim_due_refresh_schedules_with_pool(&pool, limit).await
}

pub(crate) async fn claim_due_refresh_schedules_with_pool(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<RefreshSchedule>, Box<dyn Error>> {
    let mut tx = pool.begin().await?;
    let claimed = sqlx::query_as::<_, RefreshSchedule>(
        r#"
        WITH due AS (
            SELECT id
            FROM axon_refresh_schedules
            WHERE enabled = TRUE AND next_run_at <= NOW()
            ORDER BY next_run_at ASC
            FOR UPDATE SKIP LOCKED
            LIMIT $1
        ),
        claimed AS (
            UPDATE axon_refresh_schedules s
            SET
                next_run_at = NOW() + make_interval(secs => $2::double precision),
                updated_at = NOW()
            FROM due
            WHERE s.id = due.id
            RETURNING
                s.id, s.name, s.seed_url, s.urls_json, s.every_seconds, s.enabled,
                s.next_run_at, s.last_run_at, s.created_at, s.updated_at
        )
        SELECT
            id, name, seed_url, urls_json, every_seconds, enabled,
            next_run_at, last_run_at, created_at, updated_at
        FROM claimed
        ORDER BY next_run_at ASC, created_at ASC
        "#,
    )
    .bind(limit)
    .bind(SCHEDULE_CLAIM_LEASE_SECS)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(claimed)
}

pub async fn mark_refresh_schedule_ran(
    cfg: &Config,
    id: Uuid,
    next_run_at: DateTime<Utc>,
) -> Result<bool, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema_once(&pool).await?;
    mark_refresh_schedule_ran_with_pool(&pool, id, next_run_at).await
}

pub(crate) async fn mark_refresh_schedule_ran_with_pool(
    pool: &PgPool,
    id: Uuid,
    next_run_at: DateTime<Utc>,
) -> Result<bool, Box<dyn Error>> {
    let rows = sqlx::query(
        "UPDATE axon_refresh_schedules SET last_run_at = NOW(), next_run_at = $2, updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .bind(next_run_at)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(rows > 0)
}
