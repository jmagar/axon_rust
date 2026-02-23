use crate::crates::core::config::{Config, RenderMode};
use crate::crates::core::logging::{log_done, log_info, log_warn};
use crate::crates::crawl::engine::{run_crawl_once, should_fallback_to_chrome, CrawlSummary};
use crate::crates::jobs::batch_jobs::apply_queue_injection_with_pool;
use crate::crates::jobs::embed_jobs::start_embed_job_with_pool;
use crate::crates::jobs::status::JobStatus;
use crate::crates::vector::ops::qdrant::qdrant_delete_stale_domain_urls;
use sqlx::PgPool;
use std::collections::HashSet;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

use super::super::robots::{append_robots_backfill, RobotsBackfillStats, RobotsDiscoveryStats};
use super::super::{
    read_manifest_candidates, read_manifest_urls, resolve_initial_mode, write_audit_diff,
    MID_CRAWL_INJECTION_MIN_CANDIDATES, MID_CRAWL_INJECTION_TRIGGER_PAGES,
};
use super::job_context::{load_job_execution_context, JobExecutionContext};
use super::result_builder::{build_completed_result, CompletedResultContext};

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
            sqlx::query(&format!(
                "UPDATE axon_crawl_jobs SET status='{completed}', updated_at=NOW(), finished_at=NOW(), error_text=NULL, result_json=$2 WHERE id=$1 AND status='{running}'",
                completed = JobStatus::Completed.as_str(),
                running = JobStatus::Running.as_str(),
            ))
            .bind(id)
            .bind(result_json)
            .execute(pool)
            .await?;
            log_done(&format!("worker completed crawl job {id}"));
        }
        Err(err) => {
            sqlx::query(&format!(
                "UPDATE axon_crawl_jobs SET status='{failed}', updated_at=NOW(), finished_at=NOW(), error_text=$2 WHERE id=$1 AND status='{running}'",
                failed = JobStatus::Failed.as_str(),
                running = JobStatus::Running.as_str(),
            ))
            .bind(id)
            .bind(err.to_string())
            .execute(pool)
            .await?;
            log_warn(&format!("worker failed crawl job {id}"));
        }
    }
    Ok(())
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
        "elapsed_ms": 0,
        "output_dir": ctx.job_cfg.output_dir.to_string_lossy(),
        "audit_diff": diff_report,
        "audit_report_path": report_path.to_string_lossy(),
    });
    sqlx::query(&format!(
        "UPDATE axon_crawl_jobs SET status='{completed}', updated_at=NOW(), finished_at=NOW(), error_text=NULL, result_json=$2 WHERE id=$1 AND status='{running}'",
        completed = JobStatus::Completed.as_str(),
        running = JobStatus::Running.as_str(),
    ))
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
    tokio::sync::mpsc::Sender<CrawlSummary>,
    tokio::task::JoinHandle<()>,
) {
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel::<CrawlSummary>(256);
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
                        let injection = match apply_queue_injection_with_pool(
                            &progress_pool,
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
            let _ = sqlx::query(&format!(
                "UPDATE axon_crawl_jobs SET updated_at=NOW(), result_json=$2 WHERE id=$1 AND status='{running}'",
                running = JobStatus::Running.as_str(),
            ))
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
    progress_tx: tokio::sync::mpsc::Sender<CrawlSummary>,
) -> Result<(CrawlSummary, std::collections::HashSet<String>), Box<dyn Error>> {
    let initial_mode =
        resolve_initial_mode(ctx.job_cfg.render_mode, ctx.job_cfg.cache_skip_browser);
    let (http_summary, http_seen_urls) = run_crawl_once(
        &ctx.job_cfg,
        &ctx.url,
        initial_mode,
        &ctx.job_cfg.output_dir,
        Some(progress_tx),
        false, // HTTP probe: sitemap runs in the final pass only
    )
    .await?;

    if !matches!(ctx.job_cfg.render_mode, RenderMode::AutoSwitch)
        || !should_fallback_to_chrome(&http_summary, ctx.job_cfg.max_pages, &ctx.job_cfg)
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
        ctx.job_cfg.discover_sitemaps, // Chrome final pass: run sitemap if enabled
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
) -> Result<(RobotsBackfillStats, RobotsDiscoveryStats), Box<dyn Error>> {
    if !ctx.job_cfg.discover_sitemaps {
        return Ok((
            RobotsBackfillStats::default(),
            RobotsDiscoveryStats::default(),
        ));
    }
    // Robots.txt sitemap discovery supplements spider-native crawl_sitemap().
    // Spider doesn't parse robots.txt for Sitemap: directives, so we do it here.
    append_robots_backfill(
        &ctx.job_cfg,
        &ctx.url,
        &ctx.job_cfg.output_dir,
        seen_urls,
        final_summary,
    )
    .await
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
    pool: &PgPool,
    ctx: &JobExecutionContext,
    crawl_job_id: Uuid,
) -> Result<(), Box<dyn Error>> {
    if !ctx.job_cfg.embed {
        return Ok(());
    }
    let markdown_dir = ctx.job_cfg.output_dir.join("markdown");
    let embed_job_id =
        start_embed_job_with_pool(pool, &ctx.job_cfg, &markdown_dir.to_string_lossy()).await?;
    log_info(&format!(
        "command=crawl enqueue_embed crawl_job_id={} embed_job_id={}",
        crawl_job_id, embed_job_id
    ));
    Ok(())
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
        let (robots_backfill_stats, robots_discovery_stats) =
            maybe_append_backfills(ctx, &seen_urls, &mut final_summary).await?;

        let stale_deleted = maybe_reconcile_stale_urls(ctx, &manifest_path)
            .await
            .unwrap_or_else(|err| {
                log_warn(&format!("crawl reconcile stale URLs failed: {err}"));
                0
            });

        maybe_enqueue_embed_job(pool, ctx, id).await?;

        let mut result_json = build_completed_result(
            ctx,
            manifest_path.as_path(),
            CompletedResultContext {
                summary,
                final_summary,
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
