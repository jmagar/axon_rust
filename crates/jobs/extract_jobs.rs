use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::content::{
    run_extract_with_engine, DeterministicExtractionEngine,
};
use crate::axon_cli::crates::core::health::redis_healthy;
use crate::axon_cli::crates::core::logging::{log_done, log_info, log_warn};
use crate::axon_cli::crates::jobs::common::{
    claim_next_pending, claim_pending_by_id, enqueue_job, make_pool, mark_job_failed,
    open_amqp_channel, reclaim_stale_running_jobs, JobTable,
};
use chrono::{DateTime, Utc};
use futures_util::stream::FuturesUnordered;
use futures_util::StreamExt;
use lapin::options::{BasicAckOptions, BasicConsumeOptions};
use lapin::types::FieldTable;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use spider::tokio;
use sqlx::{FromRow, PgPool};
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

const TABLE: JobTable = JobTable::Extract;
const STALE_SWEEP_INTERVAL_SECS: u64 = 30;
const WORKER_CONCURRENCY: usize = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtractJobConfig {
    prompt: Option<String>,
    max_pages: u32,
}

#[derive(Debug, FromRow, Serialize)]
pub struct ExtractJob {
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
        CREATE TABLE IF NOT EXISTS axon_extract_jobs (
            id UUID PRIMARY KEY,
            status TEXT NOT NULL,
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
    Ok(())
}

pub async fn start_extract_job(
    cfg: &Config,
    urls: &[String],
    prompt: Option<String>,
) -> Result<Uuid, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;

    let urls_json = serde_json::to_value(urls)?;
    let cfg_json = serde_json::to_value(ExtractJobConfig {
        prompt,
        max_pages: cfg.max_pages,
    })?;
    if let Some(existing_id) = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id
        FROM axon_extract_jobs
        WHERE status IN ('pending','running')
          AND urls_json = $1
          AND config_json = $2
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(urls_json.clone())
    .bind(cfg_json.clone())
    .fetch_optional(&pool)
    .await?
    {
        log_info(&format!(
            "extract dedupe hit: reusing active job {} for {} urls",
            existing_id,
            urls.len()
        ));
        return Ok(existing_id);
    }
    let id = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO axon_extract_jobs (id, status, urls_json, config_json) VALUES ($1, 'pending', $2, $3)"#,
    )
    .bind(id)
    .bind(urls_json)
    .bind(cfg_json)
    .execute(&pool)
    .await?;

    if let Err(err) = enqueue_job(cfg, &cfg.extract_queue, id).await {
        log_warn(&format!(
            "extract enqueue failed for {id}; polling fallback will pick up: {err}"
        ));
    }

    Ok(id)
}

pub async fn get_extract_job(cfg: &Config, id: Uuid) -> Result<Option<ExtractJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    Ok(sqlx::query_as::<_, ExtractJob>(
        r#"SELECT id,status,created_at,updated_at,started_at,finished_at,error_text,urls_json,result_json FROM axon_extract_jobs WHERE id=$1"#,
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?)
}

pub async fn list_extract_jobs(
    cfg: &Config,
    limit: i64,
) -> Result<Vec<ExtractJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    Ok(sqlx::query_as::<_, ExtractJob>(
        r#"SELECT id,status,created_at,updated_at,started_at,finished_at,error_text,urls_json,result_json FROM axon_extract_jobs ORDER BY created_at DESC LIMIT $1"#,
    )
    .bind(limit)
    .fetch_all(&pool)
    .await?)
}

pub async fn cancel_extract_job(cfg: &Config, id: Uuid) -> Result<bool, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query("UPDATE axon_extract_jobs SET status='canceled',updated_at=NOW(),finished_at=NOW() WHERE id=$1 AND status IN ('pending','running')")
        .bind(id)
        .execute(&pool)
        .await?
        .rows_affected();

    let redis_client = redis::Client::open(cfg.redis_url.clone())?;
    let mut conn = redis_client.get_multiplexed_async_connection().await?;
    let key = format!("axon:extract:cancel:{id}");
    let _: () = conn.set_ex(key, "1", 86400).await?;
    Ok(rows > 0)
}

pub async fn cleanup_extract_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    Ok(
        sqlx::query("DELETE FROM axon_extract_jobs WHERE status IN ('failed','canceled')")
            .execute(&pool)
            .await?
            .rows_affected(),
    )
}

pub async fn clear_extract_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query("DELETE FROM axon_extract_jobs")
        .execute(&pool)
        .await?
        .rows_affected();
    if let Ok(ch) = open_amqp_channel(cfg, &cfg.extract_queue).await {
        let _ = ch
            .queue_purge(
                &cfg.extract_queue,
                lapin::options::QueuePurgeOptions::default(),
            )
            .await;
    }
    Ok(rows)
}

async fn process_extract_job(cfg: &Config, pool: &PgPool, id: Uuid) -> Result<(), Box<dyn Error>> {
    let run_result = async {
        let row = sqlx::query_as::<_, (serde_json::Value, serde_json::Value)>(
            "SELECT urls_json, config_json FROM axon_extract_jobs WHERE id=$1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        let Some((urls_json, cfg_json)) = row else {
            return Ok::<Option<serde_json::Value>, Box<dyn Error>>(None);
        };

        let redis_client = redis::Client::open(cfg.redis_url.clone())?;
        let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;
        let cancel_key = format!("axon:extract:cancel:{id}");
        let cancel_before: Option<String> = redis_conn.get(&cancel_key).await.ok();
        if cancel_before.is_some() {
            sqlx::query("UPDATE axon_extract_jobs SET status='canceled',updated_at=NOW(),finished_at=NOW() WHERE id=$1")
                .bind(id)
                .execute(pool)
                .await?;
            return Ok(None);
        }

        let job_cfg: ExtractJobConfig = serde_json::from_value(cfg_json)?;
        let urls: Vec<String> = serde_json::from_value(urls_json)?;
        let prompt = job_cfg
            .prompt
            .ok_or("extract prompt is required; pass --query")?;
        let mut runs = Vec::new();
        let mut all_results = Vec::new();
        let mut pages_visited = 0usize;
        let mut pages_with_data = 0usize;
        let mut deterministic_pages = 0usize;
        let mut llm_fallback_pages = 0usize;
        let mut llm_requests = 0usize;
        let mut prompt_tokens = 0u64;
        let mut completion_tokens = 0u64;
        let mut total_tokens = 0u64;
        let mut estimated_cost_usd = 0.0f64;
        let mut parser_hits = serde_json::Map::new();
        let engine = Arc::new(DeterministicExtractionEngine::with_default_parsers());
        let max_pages = job_cfg.max_pages;
        let openai_base_url = cfg.openai_base_url.clone();
        let openai_api_key = cfg.openai_api_key.clone();
        let openai_model = cfg.openai_model.clone();
        let mut pending_runs = FuturesUnordered::new();

        for url in urls {
            let engine = Arc::clone(&engine);
            let prompt = prompt.clone();
            let openai_base_url = openai_base_url.clone();
            let openai_api_key = openai_api_key.clone();
            let openai_model = openai_model.clone();
            pending_runs.push(async move {
                let run = run_extract_with_engine(
                    &url,
                    &prompt,
                    max_pages,
                    &openai_base_url,
                    &openai_api_key,
                    &openai_model,
                    engine,
                )
                .await;
                (url, run)
            });
        }

        while let Some((url, run_result)) = pending_runs.next().await {
            match run_result {
                Ok(run) => {
                    pages_visited += run.pages_visited;
                    pages_with_data += run.pages_with_data;
                    deterministic_pages += run.metrics.deterministic_pages;
                    llm_fallback_pages += run.metrics.llm_fallback_pages;
                    llm_requests += run.metrics.llm_requests;
                    prompt_tokens += run.metrics.prompt_tokens;
                    completion_tokens += run.metrics.completion_tokens;
                    total_tokens += run.metrics.total_tokens;
                    estimated_cost_usd += run.metrics.estimated_cost_usd;
                    for (name, count) in run.parser_hits.clone() {
                        let current = parser_hits
                            .get(&name)
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        parser_hits.insert(name, serde_json::json!(current + count as u64));
                    }
                    all_results.extend(run.results.clone());
                    runs.push(serde_json::json!({
                        "url": run.start_url,
                        "pages_visited": run.pages_visited,
                        "pages_with_data": run.pages_with_data,
                        "deterministic_pages": run.metrics.deterministic_pages,
                        "llm_fallback_pages": run.metrics.llm_fallback_pages,
                        "llm_requests": run.metrics.llm_requests,
                        "prompt_tokens": run.metrics.prompt_tokens,
                        "completion_tokens": run.metrics.completion_tokens,
                        "total_tokens": run.metrics.total_tokens,
                        "estimated_cost_usd": run.metrics.estimated_cost_usd,
                        "parser_hits": run.parser_hits,
                        "total_items": run.results.len(),
                        "results": run.results
                    }));
                }
                Err(err) => {
                    runs.push(serde_json::json!({
                        "url": url,
                        "error": err.to_string(),
                        "pages_visited": 0,
                        "pages_with_data": 0,
                        "total_items": 0,
                        "results": []
                    }));
                }
            }
        }

        Ok(Some(serde_json::json!({
            "prompt": prompt,
            "model": cfg.openai_model,
            "pages_visited": pages_visited,
            "pages_with_data": pages_with_data,
            "deterministic_pages": deterministic_pages,
            "llm_fallback_pages": llm_fallback_pages,
            "llm_requests": llm_requests,
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": total_tokens,
            "estimated_cost_usd": estimated_cost_usd,
            "parser_hits": parser_hits,
            "total_items": all_results.len(),
            "runs": runs,
            "results": all_results
        })))
    }
    .await;

    match run_result {
        Ok(Some(result_json)) => {
            sqlx::query(
                "UPDATE axon_extract_jobs SET status='completed',updated_at=NOW(),finished_at=NOW(),result_json=$2,error_text=NULL WHERE id=$1 AND status='running'",
            )
            .bind(id)
            .bind(result_json)
            .execute(pool)
            .await?;
            log_done(&format!("worker completed extract job {id}"));
        }
        Ok(None) => {}
        Err(err) => {
            let error_text = err.to_string();
            let _ = sqlx::query(
                "UPDATE axon_extract_jobs SET status='failed',updated_at=NOW(),finished_at=NOW(),error_text=$2 WHERE id=$1 AND status='running'",
            )
            .bind(id)
            .bind(error_text.clone())
            .execute(pool)
            .await;
            log_warn(&format!("worker failed extract job {id}: {error_text}"));
        }
    }

    Ok(())
}

pub async fn run_extract_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    match reclaim_stale_running_jobs(
        &pool,
        TABLE,
        "extract",
        cfg.watchdog_stale_timeout_secs,
        cfg.watchdog_confirm_secs,
        "startup",
    )
    .await
    {
        Ok(stats) => {
            if stats.stale_candidates > 0 || stats.reclaimed_jobs > 0 {
                log_info(&format!(
                    "watchdog extract startup sweep candidates={} marked={} reclaimed={}",
                    stats.stale_candidates, stats.marked_candidates, stats.reclaimed_jobs
                ));
            }
        }
        Err(err) => log_warn(&format!("watchdog extract startup sweep failed: {err}")),
    }

    let run_amqp_lane = |lane: usize| {
        let pool = pool.clone();
        async move {
            let ch = open_amqp_channel(cfg, &cfg.extract_queue).await?;
            let tag = format!("axon-rust-extract-worker-{lane}");
            let mut consumer = ch
                .basic_consume(
                    &cfg.extract_queue,
                    &tag,
                    BasicConsumeOptions::default(),
                    FieldTable::default(),
                )
                .await?;
            log_info(&format!(
                "extract worker lane={} listening on queue={} concurrency={}",
                lane, cfg.extract_queue, WORKER_CONCURRENCY
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
                            "extract",
                            cfg.watchdog_stale_timeout_secs,
                            cfg.watchdog_confirm_secs,
                            "amqp",
                        )
                        .await
                        {
                            if stats.stale_candidates > 0 || stats.reclaimed_jobs > 0 {
                                log_info(&format!(
                                "watchdog extract sweep lane={} candidates={} marked={} reclaimed={}",
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
                        if let Err(err) = process_extract_job(cfg, &pool, job_id).await {
                            let error_text = err.to_string();
                            mark_job_failed(&pool, TABLE, job_id, &error_text).await;
                            log_warn(&format!("worker failed extract job {job_id}: {error_text}"));
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
                "extract worker polling lane={} active queue={}",
                lane, cfg.extract_queue
            ));
            let mut last_sweep = Instant::now();
            loop {
                if last_sweep.elapsed() >= Duration::from_secs(STALE_SWEEP_INTERVAL_SECS) {
                    if let Ok(stats) = reclaim_stale_running_jobs(
                        &pool,
                        TABLE,
                        "extract",
                        cfg.watchdog_stale_timeout_secs,
                        cfg.watchdog_confirm_secs,
                        "polling",
                    )
                    .await
                    {
                        if stats.stale_candidates > 0 || stats.reclaimed_jobs > 0 {
                            log_info(&format!(
                            "watchdog extract sweep lane={} candidates={} marked={} reclaimed={}",
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
                    if let Err(err) = process_extract_job(cfg, &pool, id).await {
                        let error_text = err.to_string();
                        mark_job_failed(&pool, TABLE, id, &error_text).await;
                        log_warn(&format!("worker failed extract job {id}: {error_text}"));
                    }
                } else {
                    tokio::time::sleep(Duration::from_millis(800)).await;
                }
            }
            #[allow(unreachable_code)]
            Result::<(), Box<dyn Error>>::Ok(())
        }
    };

    if open_amqp_channel(cfg, &cfg.extract_queue).await.is_ok() {
        tokio::try_join!(run_amqp_lane(1), run_amqp_lane(2))?;
        return Ok(());
    }

    log_warn("amqp unavailable; running extract worker in postgres polling mode");
    tokio::try_join!(run_polling_lane(1), run_polling_lane(2))?;
    Ok(())
}

pub async fn recover_stale_extract_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let stats = reclaim_stale_running_jobs(
        &pool,
        TABLE,
        "extract",
        cfg.watchdog_stale_timeout_secs,
        cfg.watchdog_confirm_secs,
        "manual",
    )
    .await?;
    Ok(stats.reclaimed_jobs)
}

pub async fn extract_doctor(cfg: &Config) -> Result<serde_json::Value, Box<dyn Error>> {
    let pg_ok = make_pool(cfg).await.is_ok();
    let amqp_ok = open_amqp_channel(cfg, &cfg.extract_queue).await.is_ok();
    let redis_ok = redis_healthy(&cfg.redis_url).await;
    Ok(serde_json::json!({
        "postgres_ok": pg_ok,
        "amqp_ok": amqp_ok,
        "redis_ok": redis_ok,
        "all_ok": pg_ok && amqp_ok && redis_ok
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::axon_cli::crates::jobs::common::test_config;
    use chrono::{DateTime, Duration, Utc};
    use std::env;
    use tokio::time::{sleep, timeout, Duration as TokioDuration};

    fn pg_url() -> Option<String> {
        env::var("AXON_TEST_PG_URL")
            .ok()
            .or_else(|| env::var("AXON_PG_URL").ok())
            .filter(|v| !v.trim().is_empty())
    }

    #[tokio::test]
    async fn extract_start_job_dedupes_active_pending_job() -> Result<(), Box<dyn Error>> {
        let Some(pg_url) = pg_url() else {
            return Ok(());
        };
        let cfg = test_config(&pg_url);
        let url = format!("https://example.com/extract/{}", Uuid::new_v4());
        let urls = vec![url];

        let first_id = start_extract_job(&cfg, &urls, Some("extract prompt".to_string())).await?;
        let second_id = start_extract_job(&cfg, &urls, Some("extract prompt".to_string())).await?;
        assert_eq!(first_id, second_id);

        let pool = make_pool(&cfg).await?;
        let _ = sqlx::query("DELETE FROM axon_extract_jobs WHERE id = $1")
            .bind(first_id)
            .execute(&pool)
            .await;
        Ok(())
    }

    #[tokio::test]
    async fn extract_recover_reclaims_confirmed_stale_running_job() -> Result<(), Box<dyn Error>> {
        let Some(pg_url) = pg_url() else {
            return Ok(());
        };
        let cfg = test_config(&pg_url);
        let url = format!("https://example.com/recover/{}", Uuid::new_v4());
        let urls = vec![url];
        let id = start_extract_job(&cfg, &urls, None).await?;
        let pool = make_pool(&cfg).await?;

        sqlx::query(
            "UPDATE axon_extract_jobs SET status='running', updated_at=NOW() - INTERVAL '20 minutes' WHERE id=$1",
        )
        .bind(id)
        .execute(&pool)
        .await?;

        let observed_updated_at: DateTime<Utc> =
            sqlx::query_scalar("SELECT updated_at FROM axon_extract_jobs WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await?;
        let watchdog = serde_json::json!({
            "_watchdog": {
                "observed_updated_at": observed_updated_at.to_rfc3339(),
                "first_seen_stale_at": (Utc::now() - Duration::minutes(10)).to_rfc3339()
            }
        });
        sqlx::query("UPDATE axon_extract_jobs SET result_json=$2 WHERE id=$1")
            .bind(id)
            .bind(watchdog)
            .execute(&pool)
            .await?;

        let reclaimed = recover_stale_extract_jobs(&cfg).await?;
        assert!(reclaimed >= 1);

        let status: String =
            sqlx::query_scalar("SELECT status FROM axon_extract_jobs WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await?;
        assert_eq!(status, "failed");

        let _ = sqlx::query("DELETE FROM axon_extract_jobs WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await;
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn extract_worker_e2e_processes_pending_job_to_terminal_status(
    ) -> Result<(), Box<dyn Error>> {
        let Some(pg_url) = pg_url() else {
            return Ok(());
        };
        let mut cfg = test_config(&pg_url);
        cfg.query = Some("extract worker e2e prompt".to_string());
        let url = format!("https://example.com/extract-worker/{}", Uuid::new_v4());
        let urls = vec![url];
        let id = start_extract_job(&cfg, &urls, cfg.query.clone()).await?;

        let worker_cfg = cfg.clone();
        let worker = tokio::task::spawn_local(async move {
            let _ = run_extract_worker(&worker_cfg).await;
        });

        let pool = make_pool(&cfg).await?;
        let wait = timeout(TokioDuration::from_secs(8), async {
            loop {
                let status: Option<String> =
                    sqlx::query_scalar("SELECT status FROM axon_extract_jobs WHERE id=$1")
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
            "extract worker did not reach terminal state in time"
        );

        let status: String =
            sqlx::query_scalar("SELECT status FROM axon_extract_jobs WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await?;
        assert!(matches!(
            status.as_str(),
            "completed" | "failed" | "canceled"
        ));

        let _ = sqlx::query("DELETE FROM axon_extract_jobs WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await;
        Ok(())
    }
}
