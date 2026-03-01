//! Post-crawl processing: sitemap backfill, stale URL reconciliation, latest reflink.

use super::super::read_manifest_urls;
use crate::crates::core::content::url_to_domain;
use crate::crates::core::logging::{log_info, log_warn};
use crate::crates::crawl::engine::{CrawlSummary, update_latest_reflink};
use crate::crates::vector::ops::qdrant::qdrant_delete_stale_domain_urls;
use std::collections::HashSet;
use std::error::Error;
use std::path::Path;

use super::super::robots::{RobotsBackfillStats, RobotsDiscoveryStats, append_robots_backfill};
use super::job_context::JobExecutionContext;

pub(super) async fn maybe_append_backfills(
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

pub(super) async fn maybe_reconcile_stale_urls(
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

pub(super) async fn update_latest_link(ctx: &JobExecutionContext) {
    if let Some(parent) = ctx.job_cfg.output_dir.parent() {
        let latest_dir = parent.join("latest");
        if let Err(err) = update_latest_reflink(&ctx.job_cfg.output_dir, &latest_dir).await {
            log_warn(&format!(
                "failed to update 'latest' reflink for domain {}: {err}",
                url_to_domain(&ctx.url)
            ));
        }
    }
}
