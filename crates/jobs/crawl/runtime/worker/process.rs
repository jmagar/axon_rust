use crate::crates::core::config::{Config, RenderMode};
use crate::crates::core::content::url_to_domain;
use crate::crates::core::logging::{log_done, log_info, log_warn};
use crate::crates::crawl::engine::{
    CrawlSummary, run_crawl_once, should_fallback_to_chrome, update_latest_reflink,
};
use crate::crates::jobs::embed::start_embed_job_with_pool;
use crate::crates::jobs::status::JobStatus;
use crate::crates::vector::ops::qdrant::qdrant_delete_stale_domain_urls;
use redis::AsyncCommands;
use sqlx::PgPool;
use std::collections::HashSet;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

use super::super::robots::{RobotsBackfillStats, RobotsDiscoveryStats, append_robots_backfill};
use super::super::{read_manifest_urls, resolve_initial_mode, write_audit_diff};
use super::job_context::{JobExecutionContext, load_job_execution_context};
use super::result_builder::{CompletedResultContext, build_completed_result};

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
            let is_canceled = err.contains("canceled");
            let status = if is_canceled {
                JobStatus::Canceled
            } else {
                JobStatus::Failed
            };
            sqlx::query(&format!(
                "UPDATE axon_crawl_jobs SET status='{status}', updated_at=NOW(), finished_at=NOW(), error_text=$2 WHERE id=$1 AND status='{running}'",
                status = status.as_str(),
                running = JobStatus::Running.as_str(),
            ))
            .bind(id)
            .bind(err.to_string())
            .execute(pool)
            .await?;
            if is_canceled {
                log_info(&format!("worker canceled crawl job {id}"));
            } else {
                log_warn(&format!("worker failed crawl job {id}"));
            }
        }
    }
    Ok(())
}

const DEFAULT_CACHE_TTL_SECS: u64 = 24 * 60 * 60;

async fn maybe_complete_cache_hit(
    pool: &PgPool,
    id: Uuid,
    ctx: &JobExecutionContext,
) -> Result<bool, Box<dyn Error>> {
    if !ctx.job_cfg.cache || ctx.previous_urls.is_empty() {
        return Ok(false);
    }

    // Check TTL of previous job result
    if let Some((_, previous_result_json)) =
        super::super::latest_completed_result_for_url(pool, &ctx.url, id).await?
    {
        let Some(previous_manifest_path) = previous_result_json
            .get("output_dir")
            .and_then(|v| v.as_str())
            .map(|d| PathBuf::from(d).join("manifest.jsonl"))
        else {
            // No output_dir in previous result — treat as stale, can't verify freshness
            log_info(&format!(
                "crawl job {id} previous result has no output_dir — starting fresh crawl"
            ));
            return Ok(false);
        };

        if crate::crates::crawl::manifest::manifest_cache_is_stale(
            &previous_manifest_path,
            DEFAULT_CACHE_TTL_SECS,
        )
        .await
        {
            log_info(&format!(
                "crawl job {id} cache source is stale (TTL {}s) — starting fresh crawl",
                DEFAULT_CACHE_TTL_SECS
            ));
            return Ok(false);
        }
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
) -> (
    tokio::sync::mpsc::Sender<CrawlSummary>,
    tokio::task::JoinHandle<()>,
) {
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel::<CrawlSummary>(256);
    let progress_pool = pool.clone();
    let progress_job_id = id;
    let progress_task = tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            let pages_crawled = progress.pages_seen as u64;
            let filtered_urls = pages_crawled.saturating_sub(progress.markdown_files as u64);

            let progress_json = serde_json::json!({
                "phase": "crawling",
                "md_created": progress.markdown_files,
                "thin_md": progress.thin_pages,
                "filtered_urls": filtered_urls,
                "pages_crawled": pages_crawled,
                "crawl_stream_pages": progress.pages_seen,
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

/// Polls Redis every 3 seconds until the cancel key is found, then returns.
/// Creates the Redis connection once and reuses it across all polls.
/// Used as the cancellation arm of a tokio::select! in run_active_crawl_job.
///
/// Fail-safe: if the connection cannot be established or breaks, logs a warning
/// and returns without canceling (never false-cancels).
async fn poll_cancel_key(cfg: &Config, id: Uuid) {
    let Ok(client) = redis::Client::open(cfg.redis_url.clone()) else {
        log_warn(&format!(
            "crawl cancel poll: failed to open Redis client for job {id}; cancellation disabled"
        ));
        // Park forever — tokio::select! will still complete via the crawl future.
        std::future::pending::<()>().await;
        return;
    };
    let conn = tokio::time::timeout(
        Duration::from_secs(3),
        client.get_multiplexed_async_connection(),
    )
    .await;
    let Ok(Ok(mut conn)) = conn else {
        log_warn(&format!(
            "crawl cancel poll: Redis connect failed for job {id}; cancellation disabled"
        ));
        std::future::pending::<()>().await;
        return;
    };
    let key = format!("axon:crawl:cancel:{id}");
    loop {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let result =
            tokio::time::timeout(Duration::from_secs(3), conn.get::<_, Option<String>>(&key)).await;
        match result {
            Ok(Ok(Some(_))) => return,
            Ok(Ok(None)) => {}
            Ok(Err(e)) => {
                log_warn(&format!(
                    "crawl cancel poll: Redis GET failed for job {id}: {e}; cancellation disabled"
                ));
                std::future::pending::<()>().await;
                return;
            }
            Err(_) => {
                log_warn(&format!(
                    "crawl cancel poll: Redis GET timed out for job {id}; cancellation disabled"
                ));
                std::future::pending::<()>().await;
                return;
            }
        }
    }
}

async fn run_primary_with_optional_chrome_fallback(
    ctx: &JobExecutionContext,
    id: Uuid,
    progress_tx: tokio::sync::mpsc::Sender<CrawlSummary>,
) -> Result<(CrawlSummary, HashSet<String>), Box<dyn Error>> {
    let initial_mode =
        resolve_initial_mode(ctx.job_cfg.render_mode, ctx.job_cfg.cache_skip_browser);
    let (http_summary, http_seen_urls) = run_crawl_once(
        &ctx.job_cfg,
        &ctx.url,
        initial_mode,
        &ctx.job_cfg.output_dir,
        Some(progress_tx),
        false, // HTTP probe: sitemap runs in the final pass only
        ctx.previous_manifest.clone(),
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
        ctx.previous_manifest.clone(),
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
    let (progress_tx, progress_task) = spawn_progress_task(pool, id);

    let final_prompt = ctx.extraction_prompt.clone();
    let result = async {
        let (summary, seen_urls) = tokio::select! {
            result = run_primary_with_optional_chrome_fallback(ctx, id, progress_tx) => result?,
            _ = poll_cancel_key(&ctx.job_cfg, id) => {
                log_info(&format!("crawl job {id} canceled mid-crawl; stopping"));
                return Err(format!("crawl job {id} canceled").into());
            }
        };

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

        if let Some(parent) = ctx.job_cfg.output_dir.parent() {
            let latest_dir = parent.join("latest");
            if let Err(err) = update_latest_reflink(&ctx.job_cfg.output_dir, &latest_dir).await {
                log_warn(&format!(
                    "failed to update 'latest' reflink for domain {}: {err}",
                    url_to_domain(&ctx.url)
                ));
            }
        }

        let mut result_json = build_completed_result(
            ctx,
            manifest_path.as_path(),
            CompletedResultContext {
                summary,
                final_summary,
                robots_backfill_stats,
                robots_discovery_stats,
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
