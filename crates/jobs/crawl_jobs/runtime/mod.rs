use crate::crates::core::config::{Config, RenderMode};
use crate::crates::core::health::redis_healthy;
use crate::crates::core::logging::{log_info, log_warn};
use crate::crates::jobs::batch_jobs::InjectionCandidate;
use crate::crates::jobs::common::{
    batch_enqueue_jobs, enqueue_job, make_pool, open_amqp_channel, purge_queue_safe, JobTable,
};
use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use spider::tokio;
use sqlx::{FromRow, PgPool};
use std::collections::HashSet;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use uuid::Uuid;

static SCHEMA_INIT: OnceLock<()> = OnceLock::new();

const TABLE: JobTable = JobTable::Crawl;
const MID_CRAWL_INJECTION_TRIGGER_PAGES: u32 = 25;
const MID_CRAWL_INJECTION_MIN_CANDIDATES: usize = 3;
const WORKER_CONCURRENCY: usize = 2;
const STALE_SWEEP_INTERVAL_SECS: u64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CrawlJobConfig {
    max_pages: u32,
    max_depth: usize,
    include_subdomains: bool,
    exclude_path_prefix: Vec<String>,
    respect_robots: bool,
    min_markdown_chars: usize,
    drop_thin_markdown: bool,
    discover_sitemaps: bool,
    embed: bool,
    render_mode: RenderMode,
    collection: String,
    output_dir: String,
    crawl_concurrency_limit: Option<usize>,
    backfill_concurrency_limit: Option<usize>,
    delay_ms: u64,
    request_timeout_ms: Option<u64>,
    fetch_retries: usize,
    retry_backoff_ms: u64,
    shared_queue: bool,
    extraction_prompt: Option<String>,
    #[serde(default = "default_cache_enabled")]
    cache: bool,
    #[serde(default)]
    cache_skip_browser: bool,
}

fn default_cache_enabled() -> bool {
    true
}

#[derive(Debug, FromRow, Serialize)]
pub struct CrawlJob {
    pub id: Uuid,
    pub url: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_text: Option<String>,
    pub result_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Default)]
struct CrawlWatchdogSweepStats {
    stale_candidates: u64,
    marked_candidates: u64,
    reclaimed_jobs: u64,
}

fn to_job_config(cfg: &Config) -> CrawlJobConfig {
    CrawlJobConfig {
        max_pages: cfg.max_pages,
        max_depth: cfg.max_depth,
        include_subdomains: cfg.include_subdomains,
        exclude_path_prefix: cfg.exclude_path_prefix.clone(),
        respect_robots: cfg.respect_robots,
        min_markdown_chars: cfg.min_markdown_chars,
        drop_thin_markdown: cfg.drop_thin_markdown,
        discover_sitemaps: cfg.discover_sitemaps,
        embed: cfg.embed,
        render_mode: cfg.render_mode,
        collection: cfg.collection.clone(),
        output_dir: cfg.output_dir.to_string_lossy().to_string(),
        crawl_concurrency_limit: cfg.crawl_concurrency_limit,
        backfill_concurrency_limit: cfg.backfill_concurrency_limit,
        delay_ms: cfg.delay_ms,
        request_timeout_ms: cfg.request_timeout_ms,
        fetch_retries: cfg.fetch_retries,
        retry_backoff_ms: cfg.retry_backoff_ms,
        shared_queue: cfg.shared_queue,
        extraction_prompt: cfg.query.clone(),
        cache: cfg.cache,
        cache_skip_browser: cfg.cache_skip_browser,
    }
}

#[derive(Debug, Serialize)]
struct CrawlAuditDiff {
    start_url: String,
    previous_count: usize,
    current_count: usize,
    added_count: usize,
    removed_count: usize,
    unchanged_count: usize,
    cache_hit: bool,
    cache_source: Option<String>,
}

fn resolve_initial_mode(render_mode: RenderMode, cache_skip_browser: bool) -> RenderMode {
    if cache_skip_browser {
        return RenderMode::Http;
    }
    match render_mode {
        RenderMode::AutoSwitch => RenderMode::Http,
        mode => mode,
    }
}

async fn read_manifest_urls(path: &Path) -> Result<HashSet<String>, Box<dyn Error>> {
    if !tokio::fs::try_exists(path).await? {
        return Ok(HashSet::new());
    }
    let content = tokio::fs::read_to_string(path).await?;
    let mut out = HashSet::new();
    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(url) = json.get("url").and_then(|value| value.as_str()) else {
            continue;
        };
        out.insert(url.to_string());
    }
    Ok(out)
}

async fn read_manifest_candidates(path: &Path) -> std::io::Result<Vec<InjectionCandidate>> {
    if !tokio::fs::try_exists(path).await? {
        return Ok(Vec::new());
    }
    let content = tokio::fs::read_to_string(path).await?;
    let mut out = Vec::new();
    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(url) = json.get("url").and_then(|value| value.as_str()) else {
            continue;
        };
        let Some(markdown_chars) = json.get("markdown_chars").and_then(|value| value.as_u64())
        else {
            continue;
        };
        out.push(InjectionCandidate {
            url: url.to_string(),
            markdown_chars: markdown_chars as usize,
        });
    }
    Ok(out)
}

async fn write_audit_diff(
    output_dir: &Path,
    start_url: &str,
    previous: &HashSet<String>,
    current: &HashSet<String>,
    cache_hit: bool,
    cache_source: Option<String>,
) -> Result<(PathBuf, CrawlAuditDiff), Box<dyn Error>> {
    let unchanged_count = previous.intersection(current).count();
    let added_count = current.difference(previous).count();
    let removed_count = previous.difference(current).count();
    let report = CrawlAuditDiff {
        start_url: start_url.to_string(),
        previous_count: previous.len(),
        current_count: current.len(),
        added_count,
        removed_count,
        unchanged_count,
        cache_hit,
        cache_source,
    };

    let audit_dir = output_dir.join("audit");
    tokio::fs::create_dir_all(&audit_dir).await?;
    let report_path = audit_dir.join("diff-report.json");
    let payload = serde_json::to_string_pretty(&report)?;
    tokio::fs::write(&report_path, payload).await?;
    Ok((report_path, report))
}

mod robots;
mod worker;
use worker::reclaim_stale_running_jobs;

async fn latest_completed_result_for_url(
    pool: &PgPool,
    url: &str,
    current_job_id: Uuid,
) -> Result<Option<(Uuid, serde_json::Value)>, Box<dyn Error>> {
    let row = sqlx::query_as::<_, (Uuid, serde_json::Value)>(
        r#"
        SELECT id, result_json
        FROM axon_crawl_jobs
        WHERE url = $1
          AND id <> $2
          AND status = 'completed'
          AND result_json IS NOT NULL
        ORDER BY finished_at DESC NULLS LAST
        LIMIT 1
        "#,
    )
    .bind(url)
    .bind(current_job_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

async fn ensure_schema(pool: &PgPool) -> Result<(), Box<dyn Error>> {
    if SCHEMA_INIT.get().is_some() {
        return Ok(());
    }

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS axon_crawl_jobs (
            id UUID PRIMARY KEY,
            url TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('pending','running','completed','failed','canceled')),
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            started_at TIMESTAMPTZ,
            finished_at TIMESTAMPTZ,
            error_text TEXT,
            result_json JSONB,
            config_json JSONB NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_axon_crawl_jobs_status ON axon_crawl_jobs(status)")
        .execute(pool)
        .await?;

    // Composite partial index for claim_next_pending: WHERE status='pending' ORDER BY created_at
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_axon_crawl_jobs_pending ON axon_crawl_jobs(created_at ASC) WHERE status = 'pending'"
    )
    .execute(pool)
    .await?;

    // Add CHECK constraint to existing tables that were created before this constraint was added.
    // This is a no-op if the constraint already exists (catches the 42710 duplicate_object error).
    let add_check = sqlx::query(
        r#"
        ALTER TABLE axon_crawl_jobs
        ADD CONSTRAINT axon_crawl_jobs_status_check
        CHECK (status IN ('pending','running','completed','failed','canceled'))
        "#,
    )
    .execute(pool)
    .await;
    match add_check {
        Ok(_) => {}
        Err(sqlx::Error::Database(ref db_err)) if db_err.code().as_deref() == Some("42710") => {
            // Constraint already exists — expected for tables created with inline CHECK.
        }
        Err(err) => return Err(err.into()),
    }

    let _ = SCHEMA_INIT.set(());
    Ok(())
}

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

pub async fn run_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    worker::run_worker(cfg).await
}

#[cfg(test)]
mod tests;
