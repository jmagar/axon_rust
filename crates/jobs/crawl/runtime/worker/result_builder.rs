use crate::crates::crawl::engine::CrawlSummary;
use std::error::Error;
use std::path::Path;

use super::super::robots::{RobotsBackfillStats, RobotsDiscoveryStats};
use super::super::{read_manifest_urls, write_audit_diff};
use super::job_context::JobExecutionContext;

pub(super) struct CompletedResultContext {
    pub(super) summary: CrawlSummary,
    pub(super) final_summary: CrawlSummary,
    pub(super) robots_backfill_stats: RobotsBackfillStats,
    pub(super) robots_discovery_stats: RobotsDiscoveryStats,
    pub(super) final_prompt: Option<String>,
}

pub(super) async fn build_completed_result(
    ctx: &JobExecutionContext,
    manifest_path: &Path,
    result_ctx: CompletedResultContext,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let crawl_discovered = result_ctx.summary.pages_seen as u64;
    let robots_extra = result_ctx.robots_backfill_stats.candidates as u64;
    let pages_discovered = crawl_discovered.saturating_add(robots_extra);
    let filtered_urls =
        pages_discovered.saturating_sub(result_ctx.final_summary.markdown_files as u64);
    let pages_crawled = result_ctx.summary.pages_seen as u64;
    let current_urls = read_manifest_urls(manifest_path).await?;
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
        "extraction_prompt": result_ctx.final_prompt,
    }))
}
