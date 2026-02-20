use crate::axon_cli::crates::core::config::{Config, RenderMode};
use crate::axon_cli::crates::core::logging::{log_done, log_info, log_warn};
use crate::axon_cli::crates::crawl::engine::{
    append_sitemap_backfill, run_crawl_once, should_fallback_to_chrome, CrawlSummary,
    SitemapBackfillStats,
};
use crate::axon_cli::crates::jobs::batch_jobs::apply_queue_injection;
use crate::axon_cli::crates::jobs::embed_jobs::start_embed_job;
use crate::axon_cli::crates::vector::ops_v2::qdrant::qdrant_delete_stale_domain_urls;
use redis::AsyncCommands;
use sqlx::PgPool;
use std::collections::HashSet;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

use super::super::robots::{append_robots_backfill, RobotsBackfillStats, RobotsDiscoveryStats};
use super::super::{
    latest_completed_result_for_url, read_manifest_candidates, read_manifest_urls,
    resolve_initial_mode, write_audit_diff, CrawlJobConfig, MID_CRAWL_INJECTION_MIN_CANDIDATES,
    MID_CRAWL_INJECTION_TRIGGER_PAGES,
};

pub(super) async fn process_job(
    cfg: &Config,
    pool: &PgPool,
    id: Uuid,
) -> Result<(), Box<dyn Error>> {
    process_job_impl(cfg, pool, id).await
}

async fn process_job_impl(cfg: &Config, pool: &PgPool, id: Uuid) -> Result<(), Box<dyn Error>> {
    let Some(ctx) = load_job_execution_context(cfg, pool, id).await? else {
        return Ok(());
    };
    log_info(&format!("crawl worker started job {} url={}", id, ctx.url));
    if maybe_complete_cache_hit(pool, id, &ctx).await? {
        return Ok(());
    }

    // Convert Box<dyn Error> to String before the match so no !Send type
    // is held across any await inside the match arms (tokio::spawn Send bound).
    let result = run_active_crawl_job(pool, id, &ctx)
        .await
        .map_err(|e| e.to_string());
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
            sqlx::query(
                "UPDATE axon_crawl_jobs SET status='failed', updated_at=NOW(), finished_at=NOW(), error_text=$2 WHERE id=$1 AND status='running'",
            )
            .bind(id)
            .bind(err.to_string())
            .execute(pool)
            .await?;
            log_warn(&format!("worker failed crawl job {id}"));
        }
    }
    Ok(())
}

struct JobExecutionContext {
    url: String,
    job_cfg: Config,
    extraction_prompt: Option<String>,
    previous_urls: HashSet<String>,
    cache_source: Option<String>,
}

async fn fetch_job_row(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<(String, serde_json::Value)>, Box<dyn Error>> {
    let row = sqlx::query_as::<_, (String, serde_json::Value)>(
        "SELECT url, config_json FROM axon_crawl_jobs WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

async fn maybe_cancel_job_before_start(
    cfg: &Config,
    pool: &PgPool,
    id: Uuid,
) -> Result<bool, Box<dyn Error>> {
    let redis_client = redis::Client::open(cfg.redis_url.clone())?;
    let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;
    let cancel_key = format!("axon:crawl:cancel:{id}");
    let cancel_before: Option<String> = redis_conn
        .get(&cancel_key)
        .await
        .map_err(|e| format!("failed to check crawl cancellation key {cancel_key}: {e}"))?;
    if cancel_before.is_none() {
        return Ok(false);
    }

    sqlx::query("UPDATE axon_crawl_jobs SET status='canceled', updated_at=NOW(), finished_at=NOW() WHERE id=$1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(true)
}

fn build_job_config(cfg: &Config, parsed: &CrawlJobConfig, id: Uuid) -> Config {
    let mut job_cfg = cfg.clone();
    job_cfg.max_pages = parsed.max_pages;
    job_cfg.max_depth = parsed.max_depth;
    job_cfg.include_subdomains = parsed.include_subdomains;
    job_cfg.exclude_path_prefix = parsed.exclude_path_prefix.clone();
    job_cfg.respect_robots = parsed.respect_robots;
    job_cfg.min_markdown_chars = parsed.min_markdown_chars;
    job_cfg.drop_thin_markdown = parsed.drop_thin_markdown;
    job_cfg.discover_sitemaps = parsed.discover_sitemaps;
    job_cfg.embed = parsed.embed;
    job_cfg.render_mode = parsed.render_mode;
    job_cfg.collection = parsed.collection.clone();
    job_cfg.crawl_concurrency_limit = parsed.crawl_concurrency_limit;
    job_cfg.sitemap_concurrency_limit = parsed.sitemap_concurrency_limit;
    job_cfg.backfill_concurrency_limit = parsed.backfill_concurrency_limit;
    job_cfg.max_sitemaps = parsed.max_sitemaps.max(1);
    job_cfg.delay_ms = parsed.delay_ms;
    job_cfg.request_timeout_ms = parsed.request_timeout_ms;
    job_cfg.fetch_retries = parsed.fetch_retries;
    job_cfg.retry_backoff_ms = parsed.retry_backoff_ms;
    job_cfg.shared_queue = parsed.shared_queue;
    job_cfg.query = parsed.extraction_prompt.clone();
    job_cfg.cache = parsed.cache;
    job_cfg.cache_skip_browser = parsed.cache_skip_browser;
    job_cfg.output_dir = PathBuf::from(parsed.output_dir.clone())
        .join("jobs")
        .join(id.to_string());
    job_cfg
}

async fn load_previous_urls_for_cache(
    pool: &PgPool,
    id: Uuid,
    url: &str,
    job_cfg: &Config,
) -> Result<(HashSet<String>, Option<String>), Box<dyn Error>> {
    let mut previous_urls = HashSet::new();
    let mut cache_source: Option<String> = None;

    if !job_cfg.cache {
        return Ok((previous_urls, cache_source));
    }

    if let Some((previous_job_id, previous_result_json)) =
        latest_completed_result_for_url(pool, url, id).await?
    {
        let previous_output_dir = previous_result_json
            .get("output_dir")
            .and_then(|value| value.as_str())
            .map(PathBuf::from);
        if let Some(previous_output_dir) = previous_output_dir {
            let previous_manifest = previous_output_dir.join("manifest.jsonl");
            previous_urls = read_manifest_urls(&previous_manifest).await?;
            if !previous_urls.is_empty() {
                cache_source = Some(format!(
                    "job:{} manifest:{}",
                    previous_job_id,
                    previous_manifest.to_string_lossy()
                ));
            }
        }
    }

    Ok((previous_urls, cache_source))
}

async fn load_job_execution_context(
    cfg: &Config,
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<JobExecutionContext>, Box<dyn Error>> {
    let row = fetch_job_row(pool, id).await?;
    let Some((url, cfg_json)) = row else {
        return Ok(None);
    };

    if maybe_cancel_job_before_start(cfg, pool, id).await? {
        return Ok(None);
    }

    let parsed: CrawlJobConfig = serde_json::from_value(cfg_json)?;
    let extraction_prompt = parsed.extraction_prompt.clone();
    let job_cfg = build_job_config(cfg, &parsed, id);
    let (previous_urls, cache_source) =
        load_previous_urls_for_cache(pool, id, &url, &job_cfg).await?;

    Ok(Some(JobExecutionContext {
        url,
        job_cfg,
        extraction_prompt,
        previous_urls,
        cache_source,
    }))
}

async fn maybe_complete_cache_hit(
    pool: &PgPool,
    id: Uuid,
    ctx: &JobExecutionContext,
) -> Result<bool, Box<dyn Error>> {
    if !ctx.job_cfg.cache || ctx.previous_urls.is_empty() {
        return Ok(false);
    }
    let (report_path, diff_report) = write_audit_diff(
        &ctx.job_cfg.output_dir,
        &ctx.url,
        &ctx.previous_urls,
        &ctx.previous_urls,
        true,
        ctx.cache_source.clone(),
    )
    .await?;
    let result_json = serde_json::json!({
        "phase": "completed",
        "cache_hit": true,
        "cache_skip_browser": ctx.job_cfg.cache_skip_browser,
        "md_created": ctx.previous_urls.len(),
        "thin_md": 0,
        "filtered_urls": 0,
        "pages_crawled": 0,
        "pages_discovered": ctx.previous_urls.len(),
        "crawl_stream_pages": 0,
        "sitemap_discovered": 0,
        "sitemap_candidates": 0,
        "sitemap_processed": 0,
        "sitemap_fetched_ok": 0,
        "sitemap_written": 0,
        "sitemap_failed": 0,
        "sitemap_filtered": 0,
        "elapsed_ms": 0,
        "output_dir": ctx.job_cfg.output_dir.to_string_lossy(),
        "audit_diff": diff_report,
        "audit_report_path": report_path.to_string_lossy(),
    });
    sqlx::query(
        "UPDATE axon_crawl_jobs SET status='completed', updated_at=NOW(), finished_at=NOW(), error_text=NULL, result_json=$2 WHERE id=$1 AND status='running'",
    )
    .bind(id)
    .bind(result_json)
    .execute(pool)
    .await?;
    log_done(&format!("worker completed crawl job {id} (cache hit)"));
    Ok(true)
}

fn spawn_progress_task(
    pool: &PgPool,
    id: Uuid,
    job_cfg: &Config,
    extraction_prompt: Option<String>,
    manifest_path: PathBuf,
    injection_state: Arc<tokio::sync::Mutex<Option<serde_json::Value>>>,
) -> (
    tokio::sync::mpsc::UnboundedSender<CrawlSummary>,
    tokio::task::JoinHandle<()>,
) {
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<CrawlSummary>();
    let progress_pool = pool.clone();
    let progress_job_id = id;
    let progress_cfg = job_cfg.clone();
    let progress_prompt = extraction_prompt;
    let progress_manifest_path = manifest_path;
    let progress_injection_state = Arc::clone(&injection_state);
    let progress_task = tokio::spawn(async move {
        let mut injection_attempted = false;
        while let Some(progress) = progress_rx.recv().await {
            let pages_crawled = progress.pages_seen as u64;
            let filtered_urls = pages_crawled.saturating_sub(progress.markdown_files as u64);

            if !injection_attempted && progress.pages_seen >= MID_CRAWL_INJECTION_TRIGGER_PAGES {
                match read_manifest_candidates(&progress_manifest_path).await {
                    Ok(candidates) if candidates.len() >= MID_CRAWL_INJECTION_MIN_CANDIDATES => {
                        let injection = match apply_queue_injection(
                            &progress_cfg,
                            &candidates,
                            progress_prompt.as_deref(),
                            "mid-crawl",
                            true,
                        )
                        .await
                        {
                            Ok(value) => value,
                            Err(err) => serde_json::json!({
                                "phase": "mid-crawl",
                                "queue_status": "failed",
                                "error": err.to_string(),
                            }),
                        };
                        *progress_injection_state.lock().await = Some(injection);
                        injection_attempted = true;
                    }
                    Ok(_) => {}
                    Err(err) => {
                        log_warn(&format!(
                            "mid-crawl queue injection probe failed for crawl job {progress_job_id}: {err}"
                        ));
                    }
                }
            }

            let mid_queue_injection = progress_injection_state.lock().await.clone();
            let progress_json = serde_json::json!({
                "phase": "crawling",
                "md_created": progress.markdown_files,
                "thin_md": progress.thin_pages,
                "filtered_urls": filtered_urls,
                "pages_crawled": pages_crawled,
                "crawl_stream_pages": progress.pages_seen,
                "mid_queue_injection": mid_queue_injection,
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

    (progress_tx, progress_task)
}

async fn run_primary_with_optional_chrome_fallback(
    ctx: &JobExecutionContext,
    id: Uuid,
    progress_tx: tokio::sync::mpsc::UnboundedSender<CrawlSummary>,
) -> Result<(CrawlSummary, std::collections::HashSet<String>), Box<dyn Error>> {
    let initial_mode =
        resolve_initial_mode(ctx.job_cfg.render_mode, ctx.job_cfg.cache_skip_browser);
    let (http_summary, http_seen_urls) = run_crawl_once(
        &ctx.job_cfg,
        &ctx.url,
        initial_mode,
        &ctx.job_cfg.output_dir,
        Some(progress_tx),
    )
    .await?;

    if !matches!(ctx.job_cfg.render_mode, RenderMode::AutoSwitch)
        || !should_fallback_to_chrome(&http_summary, ctx.job_cfg.max_pages)
    {
        return Ok((http_summary, http_seen_urls));
    }

    log_warn(&format!(
        "crawl job {id}: HTTP yielded thin results (pages={} md={}); retrying with Chrome",
        http_summary.pages_seen, http_summary.markdown_files
    ));
    match run_crawl_once(
        &ctx.job_cfg,
        &ctx.url,
        RenderMode::Chrome,
        &ctx.job_cfg.output_dir,
        None,
    )
    .await
    {
        Ok((chrome_summary, chrome_urls)) => {
            log_info(&format!(
                "crawl job {id}: Chrome fallback complete (pages={} md={})",
                chrome_summary.pages_seen, chrome_summary.markdown_files
            ));
            Ok((chrome_summary, chrome_urls))
        }
        Err(err) => {
            log_warn(&format!(
                "crawl job {id}: Chrome fallback failed ({err}), using HTTP result"
            ));
            Ok((http_summary, http_seen_urls))
        }
    }
}

async fn maybe_append_backfills(
    ctx: &JobExecutionContext,
    seen_urls: &HashSet<String>,
    final_summary: &mut CrawlSummary,
) -> Result<
    (
        SitemapBackfillStats,
        RobotsBackfillStats,
        RobotsDiscoveryStats,
    ),
    Box<dyn Error>,
> {
    let mut backfill_stats = SitemapBackfillStats::default();
    let mut robots_backfill_stats = RobotsBackfillStats::default();
    let mut robots_discovery_stats = RobotsDiscoveryStats::default();

    if ctx.job_cfg.discover_sitemaps {
        backfill_stats = append_sitemap_backfill(
            &ctx.job_cfg,
            &ctx.url,
            &ctx.job_cfg.output_dir,
            seen_urls,
            final_summary,
        )
        .await?;
        (robots_backfill_stats, robots_discovery_stats) = append_robots_backfill(
            &ctx.job_cfg,
            &ctx.url,
            &ctx.job_cfg.output_dir,
            seen_urls,
            final_summary,
        )
        .await?;
    }

    Ok((
        backfill_stats,
        robots_backfill_stats,
        robots_discovery_stats,
    ))
}

async fn maybe_reconcile_stale_urls(
    ctx: &JobExecutionContext,
    manifest_path: &Path,
) -> Result<usize, Box<dyn Error>> {
    // Skip if embedding is off or Qdrant isn't configured.
    if !ctx.job_cfg.embed || ctx.job_cfg.qdrant_url.is_empty() {
        return Ok(0);
    }
    // Skip for page-limited crawls: the manifest is intentionally incomplete,
    // so reconciling against it would delete live URLs outside the page cap.
    if ctx.job_cfg.max_pages > 0 {
        return Ok(0);
    }
    let domain = spider::url::Url::parse(&ctx.url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_string()))
        .unwrap_or_default();
    if domain.is_empty() {
        return Ok(0);
    }
    let current_urls = read_manifest_urls(manifest_path).await?;
    if current_urls.is_empty() {
        return Ok(0);
    }
    let deleted = qdrant_delete_stale_domain_urls(&ctx.job_cfg, &domain, &current_urls).await?;
    if deleted > 0 {
        log_info(&format!(
            "crawl reconcile: removed {} stale Qdrant URL(s) for domain={}",
            deleted, domain
        ));
    }
    Ok(deleted)
}

async fn maybe_enqueue_embed_job(
    ctx: &JobExecutionContext,
    crawl_job_id: Uuid,
) -> Result<(), Box<dyn Error>> {
    if !ctx.job_cfg.embed {
        return Ok(());
    }
    let markdown_dir = ctx.job_cfg.output_dir.join("markdown");
    let embed_job_id = start_embed_job(&ctx.job_cfg, &markdown_dir.to_string_lossy()).await?;
    log_info(&format!(
        "command=crawl enqueue_embed crawl_job_id={} embed_job_id={}",
        crawl_job_id, embed_job_id
    ));
    Ok(())
}

struct CompletedResultContext {
    summary: CrawlSummary,
    final_summary: CrawlSummary,
    backfill_stats: SitemapBackfillStats,
    robots_backfill_stats: RobotsBackfillStats,
    robots_discovery_stats: RobotsDiscoveryStats,
    mid_injection_state: Arc<tokio::sync::Mutex<Option<serde_json::Value>>>,
    final_prompt: Option<String>,
}

async fn build_completed_result(
    ctx: &JobExecutionContext,
    manifest_path: &Path,
    result_ctx: CompletedResultContext,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let crawl_discovered = result_ctx.summary.pages_seen as u64;
    let sitemap_discovered = result_ctx.backfill_stats.sitemap_candidates as u64;
    let robots_extra = result_ctx.robots_backfill_stats.candidates as u64;
    let pages_discovered = crawl_discovered
        .saturating_add(sitemap_discovered)
        .saturating_add(robots_extra);
    let filtered_urls =
        pages_discovered.saturating_sub(result_ctx.final_summary.markdown_files as u64);
    let pages_crawled = result_ctx.summary.pages_seen as u64;
    let current_urls = read_manifest_urls(manifest_path).await?;
    let candidates = read_manifest_candidates(manifest_path).await?;
    let mid_queue_injection = result_ctx.mid_injection_state.lock().await.clone();
    let mid_enqueued = mid_queue_injection
        .as_ref()
        .and_then(|value| value.get("queue_status"))
        .and_then(|value| value.as_str())
        == Some("enqueued");
    let queue_injection = apply_queue_injection(
        &ctx.job_cfg,
        &candidates,
        result_ctx.final_prompt.as_deref(),
        if mid_enqueued {
            "post-crawl-review"
        } else {
            "post-crawl"
        },
        !mid_enqueued,
    )
    .await?;
    let (report_path, diff_report) = write_audit_diff(
        &ctx.job_cfg.output_dir,
        &ctx.url,
        &ctx.previous_urls,
        &current_urls,
        false,
        ctx.cache_source.clone(),
    )
    .await?;

    Ok(serde_json::json!({
        "phase": "completed",
        "cache_hit": false,
        "cache_skip_browser": ctx.job_cfg.cache_skip_browser,
        "md_created": result_ctx.final_summary.markdown_files,
        "thin_md": result_ctx.final_summary.thin_pages,
        "filtered_urls": filtered_urls,
        "pages_crawled": pages_crawled,
        "pages_discovered": pages_discovered,
        "crawl_stream_pages": result_ctx.summary.pages_seen,
        "sitemap_discovered": result_ctx.backfill_stats.sitemap_discovered,
        "sitemap_candidates": result_ctx.backfill_stats.sitemap_candidates,
        "sitemap_processed": result_ctx.backfill_stats.processed,
        "sitemap_fetched_ok": result_ctx.backfill_stats.fetched_ok,
        "sitemap_written": result_ctx.backfill_stats.written,
        "sitemap_failed": result_ctx.backfill_stats.failed,
        "sitemap_filtered": result_ctx.backfill_stats.filtered,
        "robots_sitemap_docs_parsed": result_ctx.robots_discovery_stats.parsed_sitemap_documents,
        "robots_declared_sitemaps": result_ctx.robots_discovery_stats.robots_declared_sitemaps,
        "robots_discovered_urls": result_ctx.robots_backfill_stats.discovered_urls,
        "robots_candidates": result_ctx.robots_backfill_stats.candidates,
        "robots_written": result_ctx.robots_backfill_stats.written,
        "robots_failed": result_ctx.robots_backfill_stats.failed,
        "robots_filtered_existing": result_ctx.robots_backfill_stats.filtered_existing,
        "elapsed_ms": result_ctx.final_summary.elapsed_ms,
        "output_dir": ctx.job_cfg.output_dir.to_string_lossy(),
        "audit_diff": diff_report,
        "audit_report_path": report_path.to_string_lossy(),
        "mid_queue_injection": mid_queue_injection,
        "queue_injection": queue_injection,
        "extraction_observability": queue_injection["observability"].clone(),
    }))
}

async fn run_active_crawl_job(
    pool: &PgPool,
    id: Uuid,
    ctx: &JobExecutionContext,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let manifest_path = ctx.job_cfg.output_dir.join("manifest.jsonl");
    let mid_injection_state = Arc::new(tokio::sync::Mutex::new(None::<serde_json::Value>));
    let (progress_tx, progress_task) = spawn_progress_task(
        pool,
        id,
        &ctx.job_cfg,
        ctx.extraction_prompt.clone(),
        manifest_path.clone(),
        Arc::clone(&mid_injection_state),
    );

    let final_prompt = ctx.extraction_prompt.clone();
    let result = async {
        let (summary, seen_urls) =
            run_primary_with_optional_chrome_fallback(ctx, id, progress_tx).await?;

        let mut final_summary = summary.clone();
        let (backfill_stats, robots_backfill_stats, robots_discovery_stats) =
            maybe_append_backfills(ctx, &seen_urls, &mut final_summary).await?;

        let stale_deleted = maybe_reconcile_stale_urls(ctx, &manifest_path)
            .await
            .unwrap_or_else(|err| {
                log_warn(&format!("crawl reconcile stale URLs failed: {err}"));
                0
            });

        maybe_enqueue_embed_job(ctx, id).await?;

        let mut result_json = build_completed_result(
            ctx,
            manifest_path.as_path(),
            CompletedResultContext {
                summary,
                final_summary,
                backfill_stats,
                robots_backfill_stats,
                robots_discovery_stats,
                mid_injection_state: Arc::clone(&mid_injection_state),
                final_prompt,
            },
        )
        .await?;
        if let Some(obj) = result_json.as_object_mut() {
            obj.insert("stale_urls_deleted".to_string(), stale_deleted.into());
        }
        Ok(result_json)
    }
    .await;

    if let Err(err) = progress_task.await {
        log_warn(&format!(
            "progress_task panicked while serializing progress for crawl job {id}: {err:?}"
        ));
    }
    result
}
