use crate::axon_cli::crates::core::config::{Config, RenderMode};
use crate::axon_cli::crates::core::health::redis_healthy;
use crate::axon_cli::crates::core::logging::{log_done, log_info, log_warn};
use crate::axon_cli::crates::crawl::engine::{
    append_sitemap_backfill, run_crawl_once, CrawlSummary, SitemapBackfillStats,
};
use crate::axon_cli::crates::jobs::common::{
    claim_next_pending, claim_pending_by_id, enqueue_job, make_pool, mark_job_failed,
    open_amqp_channel, JobTable,
};
use crate::axon_cli::crates::jobs::embed_jobs::start_embed_job;
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use lapin::options::{BasicAckOptions, BasicConsumeOptions};
use lapin::types::FieldTable;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use spider::tokio;
use sqlx::{FromRow, PgPool};
use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

const TABLE: JobTable = JobTable::Crawl;

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
    sitemap_concurrency_limit: Option<usize>,
    backfill_concurrency_limit: Option<usize>,
    max_sitemaps: usize,
    delay_ms: u64,
    request_timeout_ms: Option<u64>,
    fetch_retries: usize,
    retry_backoff_ms: u64,
    shared_queue: bool,
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
        sitemap_concurrency_limit: cfg.sitemap_concurrency_limit,
        backfill_concurrency_limit: cfg.backfill_concurrency_limit,
        max_sitemaps: cfg.max_sitemaps,
        delay_ms: cfg.delay_ms,
        request_timeout_ms: cfg.request_timeout_ms,
        fetch_retries: cfg.fetch_retries,
        retry_backoff_ms: cfg.retry_backoff_ms,
        shared_queue: cfg.shared_queue,
    }
}

async fn ensure_schema(pool: &PgPool) -> Result<(), Box<dyn Error>> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS axon_crawl_jobs (
            id UUID PRIMARY KEY,
            url TEXT NOT NULL,
            status TEXT NOT NULL,
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

    let id = Uuid::new_v4();
    let cfg_json = serde_json::to_value(to_job_config(cfg))?;

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
    let mut conn = redis_client.get_multiplexed_async_connection().await?;
    let key = format!("axon:crawl:cancel:{id}");
    let _: () = conn.set_ex(key, "1", 24 * 60 * 60).await?;

    Ok(rows > 0)
}

pub async fn cleanup_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query(
        "DELETE FROM axon_crawl_jobs WHERE status IN ('failed','canceled') OR (status='pending' AND created_at < NOW() - INTERVAL '1 day')",
    )
    .execute(&pool)
    .await?
    .rows_affected();
    Ok(rows)
}

pub async fn clear_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query("DELETE FROM axon_crawl_jobs")
        .execute(&pool)
        .await?
        .rows_affected();

    if let Ok(ch) = open_amqp_channel(cfg, &cfg.crawl_queue).await {
        let _ = ch
            .queue_purge(
                &cfg.crawl_queue,
                lapin::options::QueuePurgeOptions::default(),
            )
            .await;
    }

    Ok(rows)
}

async fn process_job(cfg: &Config, pool: &PgPool, id: Uuid) -> Result<(), Box<dyn Error>> {
    let row = sqlx::query_as::<_, (String, serde_json::Value)>(
        "SELECT url, config_json FROM axon_crawl_jobs WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    let Some((url, cfg_json)) = row else {
        return Ok(());
    };

    let redis_client = redis::Client::open(cfg.redis_url.clone())?;
    let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;
    let cancel_key = format!("axon:crawl:cancel:{id}");
    let cancel_before: Option<String> = redis_conn.get(&cancel_key).await.ok();
    if cancel_before.is_some() {
        sqlx::query("UPDATE axon_crawl_jobs SET status='canceled', updated_at=NOW(), finished_at=NOW() WHERE id=$1")
            .bind(id)
            .execute(pool)
            .await?;
        return Ok(());
    }

    let parsed: CrawlJobConfig = serde_json::from_value(cfg_json)?;
    let mut job_cfg = cfg.clone();
    job_cfg.max_pages = parsed.max_pages;
    job_cfg.max_depth = parsed.max_depth;
    job_cfg.include_subdomains = parsed.include_subdomains;
    job_cfg.exclude_path_prefix = parsed.exclude_path_prefix;
    job_cfg.respect_robots = parsed.respect_robots;
    job_cfg.min_markdown_chars = parsed.min_markdown_chars;
    job_cfg.drop_thin_markdown = parsed.drop_thin_markdown;
    job_cfg.discover_sitemaps = parsed.discover_sitemaps;
    job_cfg.embed = parsed.embed;
    job_cfg.render_mode = parsed.render_mode;
    job_cfg.collection = parsed.collection;
    job_cfg.crawl_concurrency_limit = parsed.crawl_concurrency_limit;
    job_cfg.sitemap_concurrency_limit = parsed.sitemap_concurrency_limit;
    job_cfg.backfill_concurrency_limit = parsed.backfill_concurrency_limit;
    job_cfg.max_sitemaps = parsed.max_sitemaps.max(1);
    job_cfg.delay_ms = parsed.delay_ms;
    job_cfg.request_timeout_ms = parsed.request_timeout_ms;
    job_cfg.fetch_retries = parsed.fetch_retries;
    job_cfg.retry_backoff_ms = parsed.retry_backoff_ms;
    job_cfg.shared_queue = parsed.shared_queue;
    job_cfg.output_dir = PathBuf::from(parsed.output_dir)
        .join("jobs")
        .join(id.to_string());

    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<CrawlSummary>();
    let progress_pool = pool.clone();
    let progress_job_id = id;
    let progress_task = tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            let pages_discovered = progress.pages_seen as u64;
            let filtered_urls = pages_discovered.saturating_sub(progress.markdown_files as u64);
            let pages_crawled = progress.pages_seen as u64;
            let progress_json = serde_json::json!({
                "phase": "crawling",
                "md_created": progress.markdown_files,
                "thin_md": progress.thin_pages,
                "filtered_urls": filtered_urls,
                "pages_crawled": pages_crawled,
                "pages_discovered": pages_discovered,
                "crawl_stream_pages": progress.pages_seen,
            });
            let _ = sqlx::query(
                "UPDATE axon_crawl_jobs SET updated_at=NOW(), result_json=$2 WHERE id=$1 AND status='running'",
            )
            .bind(progress_job_id)
            .bind(progress_json)
            .execute(&progress_pool)
            .await;
        }
    });

    let result = async {
        let initial_mode = match job_cfg.render_mode {
            RenderMode::AutoSwitch => RenderMode::Http,
            m => m,
        };
        let (summary, seen_urls) = run_crawl_once(
            &job_cfg,
            &url,
            initial_mode,
            &job_cfg.output_dir,
            Some(progress_tx),
        )
        .await?;
        let mut final_summary = summary.clone();
        let mut backfill_stats = SitemapBackfillStats::default();

        if job_cfg.discover_sitemaps {
            backfill_stats = append_sitemap_backfill(
                &job_cfg,
                &url,
                &job_cfg.output_dir,
                &seen_urls,
                &mut final_summary,
            )
            .await?;
        }

        if job_cfg.embed {
            let markdown_dir = job_cfg.output_dir.join("markdown");
            let embed_job_id = start_embed_job(&job_cfg, &markdown_dir.to_string_lossy()).await?;
            log_info(&format!(
                "command=crawl enqueue_embed crawl_job_id={} embed_job_id={}",
                id, embed_job_id
            ));
        }

        let crawl_discovered = summary.pages_seen as u64;
        let sitemap_discovered = backfill_stats.sitemap_candidates as u64;
        let pages_discovered = crawl_discovered.saturating_add(sitemap_discovered);
        let filtered_urls = pages_discovered.saturating_sub(final_summary.markdown_files as u64);
        let pages_crawled = summary.pages_seen as u64;

        Ok::<serde_json::Value, Box<dyn Error>>(serde_json::json!({
            "phase": "completed",
            "md_created": final_summary.markdown_files,
            "thin_md": final_summary.thin_pages,
            "filtered_urls": filtered_urls,
            "pages_crawled": pages_crawled,
            "pages_discovered": pages_discovered,
            "crawl_stream_pages": summary.pages_seen,
            "sitemap_discovered": backfill_stats.sitemap_discovered,
            "sitemap_candidates": backfill_stats.sitemap_candidates,
            "sitemap_processed": backfill_stats.processed,
            "sitemap_fetched_ok": backfill_stats.fetched_ok,
            "sitemap_written": backfill_stats.written,
            "sitemap_failed": backfill_stats.failed,
            "sitemap_filtered": backfill_stats.filtered,
            "elapsed_ms": final_summary.elapsed_ms,
            "output_dir": job_cfg.output_dir.to_string_lossy(),
        }))
    }
    .await;

    if let Err(err) = progress_task.await {
        log_warn(&format!(
            "progress_task panicked while serializing progress for crawl job {id}: {err}"
        ));
    }

    match result {
        Ok(result_json) => {
            sqlx::query(
                "UPDATE axon_crawl_jobs SET status='completed', updated_at=NOW(), finished_at=NOW(), error_text=NULL, result_json=$2 WHERE id=$1 AND status='running'",
            )
            .bind(id)
            .bind(result_json)
            .execute(pool)
            .await?;
            log_done(&format!("worker completed crawl job {id}"));
        }
        Err(err) => {
            let msg = err.to_string();
            sqlx::query(
                "UPDATE axon_crawl_jobs SET status='failed', updated_at=NOW(), finished_at=NOW(), error_text=$2 WHERE id=$1 AND status='running'",
            )
            .bind(id)
            .bind(msg)
            .execute(pool)
            .await?;
            log_warn(&format!("worker failed crawl job {id}"));
        }
    }

    Ok(())
}

async fn run_worker_polling_loop(cfg: &Config, pool: &PgPool) -> Result<(), Box<dyn Error>> {
    log_warn("amqp unavailable; running crawl worker in postgres polling mode");
    loop {
        if let Some(job_id) = claim_next_pending(pool, TABLE).await? {
            if let Err(err) = process_job(cfg, pool, job_id).await {
                let error_text = err.to_string();
                mark_job_failed(pool, TABLE, job_id, &error_text).await;
                log_warn(&format!("worker failed crawl job {job_id}: {error_text}"));
            }
        } else {
            tokio::time::sleep(Duration::from_millis(800)).await;
        }
    }
}

pub async fn run_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;

    let ch = match open_amqp_channel(cfg, &cfg.crawl_queue).await {
        Ok(ch) => ch,
        Err(_) => return run_worker_polling_loop(cfg, &pool).await,
    };
    let mut consumer = ch
        .basic_consume(
            &cfg.crawl_queue,
            "axon-rust-crawl-worker",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    log_info(&format!(
        "crawl worker listening on queue={}",
        cfg.crawl_queue
    ));

    while let Some(msg) = consumer.next().await {
        let delivery = match msg {
            Ok(d) => d,
            Err(err) => {
                log_warn(&format!("consumer error: {err}"));
                continue;
            }
        };

        let parsed = std::str::from_utf8(&delivery.data)
            .ok()
            .and_then(|s| Uuid::parse_str(s.trim()).ok());

        if let Some(job_id) = parsed {
            if claim_pending_by_id(&pool, TABLE, job_id)
                .await
                .unwrap_or(false)
            {
                if let Err(err) = process_job(cfg, &pool, job_id).await {
                    let error_text = err.to_string();
                    mark_job_failed(&pool, TABLE, job_id, &error_text).await;
                    log_warn(&format!("worker failed crawl job {job_id}: {error_text}"));
                }
            }
        }

        delivery.ack(BasicAckOptions::default()).await?;
    }

    Ok(())
}
