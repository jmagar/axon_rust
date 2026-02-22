use crate::crates::core::config::Config;
use crate::crates::core::health::redis_healthy;
use crate::crates::core::logging::{log_info, log_warn};
use crate::crates::jobs::common::{
    batch_enqueue_jobs, enqueue_job, make_pool, open_amqp_channel, purge_queue_safe,
};
use redis::AsyncCommands;
use std::error::Error;
use uuid::Uuid;

use super::{ensure_schema, reclaim_stale_running_jobs, to_job_config, CrawlJob};

pub async fn doctor(cfg: &Config) -> Result<serde_json::Value, Box<dyn Error>> {
    let pg_ok = match make_pool(cfg).await {
        Ok(p) => ensure_schema(&p).await.is_ok(),
        Err(_) => false,
    };

    let amqp_result = open_amqp_channel(cfg, &cfg.crawl_queue).await;
    let amqp_ok = amqp_result.is_ok();
    let amqp_error = amqp_result.err().map(|e| e.to_string());

    let redis_ok = redis_healthy(&cfg.redis_url).await;

    Ok(serde_json::json!({
        "postgres_ok": pg_ok,
        "amqp_ok": amqp_ok,
        "amqp_error": amqp_error,
        "redis_ok": redis_ok,
        "all_ok": pg_ok && amqp_ok && redis_ok
    }))
}

pub async fn start_crawl_job(cfg: &Config, start_url: &str) -> Result<Uuid, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;

    let cfg_json = serde_json::to_value(to_job_config(cfg))?;
    if let Some(existing_id) = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id
        FROM axon_crawl_jobs
        WHERE status IN ('pending','running')
          AND url = $1
          AND config_json = $2
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(start_url)
    .bind(cfg_json.clone())
    .fetch_optional(&pool)
    .await?
    {
        log_info(&format!(
            "crawl dedupe hit: reusing active job {} for {}",
            existing_id, start_url
        ));
        return Ok(existing_id);
    }
    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO axon_crawl_jobs (id, url, status, config_json)
        VALUES ($1, $2, 'pending', $3)
        "#,
    )
    .bind(id)
    .bind(start_url)
    .bind(cfg_json)
    .execute(&pool)
    .await?;

    if let Err(err) = enqueue_job(cfg, &cfg.crawl_queue, id).await {
        log_warn(&format!(
            "amqp enqueue failed for {id}; worker fallback polling will pick it up: {err}"
        ));
    }
    Ok(id)
}

/// Insert and AMQP-enqueue multiple crawl jobs using a single Postgres pool and
/// a single AMQP connection (one TCP handshake for N publishes).
///
/// Returns a `Vec<(url, job_id)>` in the same order as `start_urls`.
/// Duplicate-active jobs reuse the existing ID without a new AMQP publish.
pub async fn start_crawl_jobs_batch(
    cfg: &Config,
    start_urls: &[&str],
) -> Result<Vec<(String, Uuid)>, Box<dyn Error>> {
    if start_urls.is_empty() {
        return Ok(Vec::new());
    }

    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;

    let cfg_json = serde_json::to_value(to_job_config(cfg))?;
    let url_strings: Vec<String> = start_urls.iter().map(|u| u.to_string()).collect();

    // 1. Find existing active jobs for all URLs in a single query.
    let existing_rows = sqlx::query_as::<_, (String, Uuid)>(
        r#"
        SELECT DISTINCT ON (url) url, id
        FROM axon_crawl_jobs
        WHERE status IN ('pending','running')
          AND url = ANY($1)
          AND config_json = $2
        ORDER BY url, created_at DESC
        "#,
    )
    .bind(&url_strings)
    .bind(cfg_json.clone())
    .fetch_all(&pool)
    .await?;

    let existing_map: std::collections::HashMap<String, Uuid> = existing_rows.into_iter().collect();

    // 2. Collect URLs that need new jobs (not already active).
    let new_urls: Vec<String> = url_strings
        .iter()
        .filter(|u| !existing_map.contains_key(*u))
        .cloned()
        .collect();

    // 3. Bulk INSERT new jobs using unnest, skipping any that acquired an active
    //    status between step 1 and now (race guard).
    let mut new_map: std::collections::HashMap<String, Uuid> = std::collections::HashMap::new();
    if !new_urls.is_empty() {
        let inserted_rows = sqlx::query_as::<_, (Uuid, String)>(
            r#"
            WITH new_urls AS (
                SELECT u FROM unnest($1::text[]) AS u
                WHERE NOT EXISTS (
                    SELECT 1 FROM axon_crawl_jobs
                    WHERE url = u AND status IN ('pending','running')
                )
            )
            INSERT INTO axon_crawl_jobs (id, url, status, config_json, created_at, updated_at)
            SELECT gen_random_uuid(), u, 'pending', $2::jsonb, now(), now()
            FROM new_urls
            RETURNING id, url
            "#,
        )
        .bind(&new_urls)
        .bind(cfg_json)
        .fetch_all(&pool)
        .await?;

        for (id, url) in &inserted_rows {
            new_map.insert(url.clone(), *id);
        }
    }

    // Log dedupe hits.
    for (url, id) in &existing_map {
        log_info(&format!(
            "crawl dedupe hit: reusing active job {} for {}",
            id, url
        ));
    }

    // 4. Build results in original input order.
    let mut results: Vec<(String, Uuid)> = Vec::with_capacity(start_urls.len());
    for url in &url_strings {
        if let Some(&id) = existing_map.get(url) {
            results.push((url.clone(), id));
        } else if let Some(&id) = new_map.get(url) {
            results.push((url.clone(), id));
        }
        // URLs that were filtered by the race guard in the CTE are silently
        // dropped — they are now active via another concurrent insert.
    }

    // 5. Enqueue only newly inserted jobs.
    let new_ids: Vec<Uuid> = new_map.values().copied().collect();
    if !new_ids.is_empty() {
        if let Err(err) = batch_enqueue_jobs(cfg, &cfg.crawl_queue, &new_ids).await {
            log_warn(&format!(
                "amqp batch enqueue failed; worker fallback polling will pick up {} jobs: {err}",
                new_ids.len()
            ));
        }
    }

    Ok(results)
}

pub async fn get_job(cfg: &Config, id: Uuid) -> Result<Option<CrawlJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let row = sqlx::query_as::<_, CrawlJob>(
        r#"
        SELECT id, url, status, created_at, updated_at, started_at, finished_at, error_text
            , result_json
        FROM axon_crawl_jobs
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?;
    Ok(row)
}

pub async fn list_jobs(cfg: &Config, limit: i64) -> Result<Vec<CrawlJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query_as::<_, CrawlJob>(
        r#"
        SELECT id, url, status, created_at, updated_at, started_at, finished_at, error_text
            , result_json
        FROM axon_crawl_jobs
        ORDER BY created_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(&pool)
    .await?;
    Ok(rows)
}

pub async fn cancel_job(cfg: &Config, id: Uuid) -> Result<bool, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;

    let rows = sqlx::query(
        "UPDATE axon_crawl_jobs SET status='canceled', updated_at=NOW(), finished_at=NOW() WHERE id=$1 AND status IN ('pending','running')",
    )
    .bind(id)
    .execute(&pool)
    .await?
    .rows_affected();

    let redis_client = redis::Client::open(cfg.redis_url.clone())?;
    match redis_client.get_multiplexed_async_connection().await {
        Ok(mut conn) => {
            let key = format!("axon:crawl:cancel:{id}");
            if let Err(err) = conn.set_ex::<_, _, ()>(key, "1", 24 * 60 * 60).await {
                log_warn(&format!("crawl cancel signal failed for job {id}: {err}"));
            }
        }
        Err(err) => {
            log_warn(&format!(
                "crawl cancel signal skipped for job {id}: redis connect failed: {err}"
            ));
        }
    }

    Ok(rows > 0)
}

pub async fn cleanup_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let mut total = 0u64;
    loop {
        let deleted = sqlx::query(
            "DELETE FROM axon_crawl_jobs WHERE id IN (
                SELECT id FROM axon_crawl_jobs
                WHERE status IN ('failed','canceled')
                   OR (status='pending' AND created_at < NOW() - INTERVAL '1 day')
                LIMIT 1000
            )",
        )
        .execute(&pool)
        .await?
        .rows_affected();
        total += deleted;
        if deleted == 0 {
            break;
        }
    }

    // Also prune completed jobs older than 30 days to prevent unbounded table growth.
    let completed_rows = sqlx::query(
        "DELETE FROM axon_crawl_jobs WHERE status = 'completed' AND finished_at < NOW() - INTERVAL '30 days'"
    )
    .execute(&pool)
    .await?
    .rows_affected();
    total += completed_rows;

    Ok(total)
}

pub async fn clear_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query("DELETE FROM axon_crawl_jobs")
        .execute(&pool)
        .await?
        .rows_affected();

    if let Err(err) = purge_queue_safe(cfg, &cfg.crawl_queue).await {
        log_warn(&format!("crawl clear: queue purge failed: {err}"));
    }

    Ok(rows)
}

pub async fn recover_stale_crawl_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let stats = reclaim_stale_running_jobs(
        &pool,
        0,
        cfg.watchdog_stale_timeout_secs,
        cfg.watchdog_confirm_secs,
    )
    .await?;
    Ok(stats.reclaimed_jobs)
}
