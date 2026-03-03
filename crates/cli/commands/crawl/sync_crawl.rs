use crate::crates::core::config::{Config, RenderMode};
use crate::crates::core::content::url_to_domain;
use crate::crates::core::logging::log_done;
use crate::crates::core::ui::{Spinner, accent, muted};
use crate::crates::crawl::engine::{
    CrawlSummary, chrome_refetch_thin_pages, run_crawl_once, run_sitemap_only,
    should_fallback_to_chrome, update_latest_reflink,
};
use crate::crates::crawl::manifest::{
    manifest_cache_is_stale, read_manifest_data, read_manifest_urls, write_audit_diff,
};
use crate::crates::jobs::embed::start_embed_job;
use std::collections::{HashMap, HashSet};
use std::error::Error;

const DEFAULT_CACHE_TTL_SECS: u64 = 24 * 60 * 60;

pub(super) async fn maybe_return_cached_result(
    cfg: &Config,
    start_url: &str,
    manifest_path: &std::path::Path,
    previous_urls: &HashSet<String>,
) -> Result<bool, Box<dyn Error>> {
    let cache_stale = manifest_cache_is_stale(manifest_path, DEFAULT_CACHE_TTL_SECS).await;
    if !cfg.cache || previous_urls.is_empty() || cache_stale {
        return Ok(false);
    }
    let (report_path, _) = write_audit_diff(
        &cfg.output_dir,
        start_url,
        previous_urls,
        previous_urls,
        true,
        Some(manifest_path.to_string_lossy().to_string()),
    )
    .await?;
    log_done(&format!(
        "command=crawl cache_hit=true cached_urls={} output_dir={} audit_report={}",
        previous_urls.len(),
        cfg.output_dir.to_string_lossy(),
        report_path.to_string_lossy()
    ));
    Ok(true)
}

async fn run_sitemap_only_crawl(cfg: &Config, start_url: &str) -> Result<(), Box<dyn Error>> {
    let spinner = Spinner::new("running sitemap-only crawl");
    let (summary, _) = run_sitemap_only(cfg, start_url, &cfg.output_dir, HashMap::new()).await?;
    spinner.finish(&format!(
        "sitemap-only complete (pages={}, markdown={})",
        summary.pages_seen, summary.markdown_files
    ));
    log_done(&format!(
        "command=crawl sitemap_only=true pages_seen={} markdown_files={} elapsed_ms={} output_dir={}",
        summary.pages_seen,
        summary.markdown_files,
        summary.elapsed_ms,
        cfg.output_dir.to_string_lossy(),
    ));
    Ok(())
}

async fn maybe_chrome_fallback(
    cfg: &Config,
    start_url: &str,
    http_summary: CrawlSummary,
    http_seen_urls: HashSet<String>,
    previous_manifest: HashMap<String, crate::crates::crawl::manifest::ManifestEntry>,
) -> (CrawlSummary, HashSet<String>) {
    if !matches!(cfg.render_mode, RenderMode::AutoSwitch)
        || !should_fallback_to_chrome(&http_summary, cfg.max_pages, cfg)
    {
        return (http_summary, http_seen_urls);
    }

    // WAF-blocked pages: retry with stealth Chrome before falling back to thin-page logic.
    // Feed WAF-blocked URLs into thin_urls so chrome_refetch_thin_pages re-fetches them.
    if http_summary.waf_blocked_pages > 0 && !http_summary.waf_blocked_urls.is_empty() {
        let blocked_count = http_summary.waf_blocked_pages;
        crate::crates::core::logging::log_warn(&format!(
            "waf: {blocked_count} page(s) blocked — retrying with stealth Chrome"
        ));
        let mut waf_summary = http_summary.clone();
        waf_summary.thin_urls = http_summary.waf_blocked_urls.clone();
        let updated = chrome_refetch_thin_pages(cfg, waf_summary, &cfg.output_dir).await;
        return (updated, http_seen_urls);
    }

    // Prefer surgical re-fetch: only re-fetch the remaining thin pages with Chrome
    // and keep the already-good HTTP pages. This avoids re-crawling the entire site.
    //
    // `thin_urls` is populated by the collector when `drop_thin_markdown` is true
    // (the default). After the crawl it may already be partially or fully cleared by
    // the inline Chrome rendering path (collector spawns Chrome tasks while the HTTP
    // crawl is still running). If thin_urls is now empty, the inline path recovered
    // everything — no post-crawl Chrome pass needed.
    if !http_summary.thin_urls.is_empty() {
        let thin_count = http_summary.thin_urls.len();
        let spinner = Spinner::new(&format!(
            "HTTP yielded thin results; re-fetching {thin_count} thin page(s) with Chrome"
        ));
        let updated_summary = chrome_refetch_thin_pages(cfg, http_summary, &cfg.output_dir).await;
        spinner.finish(&format!(
            "Chrome targeted re-fetch complete (pages={}, markdown={}, thin_remaining={})",
            updated_summary.pages_seen, updated_summary.markdown_files, updated_summary.thin_pages,
        ));
        return (updated_summary, http_seen_urls);
    }

    // thin_urls is empty: either the inline Chrome path recovered all thin pages,
    // or drop_thin_markdown=false and no thin URLs were tracked. In both cases we
    // do not have per-URL targets. If the inline path was active, the summary
    // already reflects Chrome-recovered content — return it as-is.
    //
    // If drop_thin_markdown=false (thin pages were saved as-is, URLs not tracked),
    // we fall through to a full Chrome re-crawl as the only remaining option.
    if cfg.drop_thin_markdown {
        // Inline Chrome path was active (or there were simply no thin pages).
        return (http_summary, http_seen_urls);
    }

    // Full Chrome re-crawl: thin URLs were not tracked because drop_thin_markdown
    // is false, so we have no per-URL targets. Re-crawl the whole site with Chrome.
    let spinner = Spinner::new("HTTP yielded thin results; retrying full crawl with Chrome");
    match run_crawl_once(
        cfg,
        start_url,
        RenderMode::Chrome,
        &cfg.output_dir,
        None,
        cfg.discover_sitemaps,
        previous_manifest,
        None,
    )
    .await
    {
        Ok((chrome_summary, chrome_urls)) => {
            spinner.finish(&format!(
                "Chrome fallback complete (pages={}, markdown={})",
                chrome_summary.pages_seen, chrome_summary.markdown_files
            ));
            (chrome_summary, chrome_urls)
        }
        Err(err) => {
            spinner.finish(&format!(
                "Chrome fallback failed ({err}), using HTTP result"
            ));
            (http_summary, http_seen_urls)
        }
    }
}

/// Bootstrap Chrome, run the initial HTTP crawl, and apply any Chrome fallback.
///
/// Returns `(summary, seen_urls, effective_cfg_holder)` — the caller owns the
/// `Config` holder so that `effective_cfg`'s lifetime extends past this call.
async fn run_crawl_phase(
    cfg: &Config,
    start_url: &str,
    previous_manifest: HashMap<String, crate::crates::crawl::manifest::ManifestEntry>,
) -> Result<(CrawlSummary, HashSet<String>, Option<Config>), Box<dyn Error>> {
    let initial_mode = super::runtime::resolve_initial_mode(cfg);
    let chrome_bootstrap = super::runtime::bootstrap_chrome_runtime(cfg).await;
    for warning in &chrome_bootstrap.warnings {
        println!("{} {}", muted("[Chrome Bootstrap]"), warning);
    }

    // Thread the pre-resolved WebSocket URL through cfg so configure_website
    // skips the redundant /json/version fetch on Chrome mode calls.
    let ws_cfg_holder: Option<Config> =
        chrome_bootstrap
            .resolved_ws_url
            .as_deref()
            .map(|ws_url| Config {
                chrome_remote_url: Some(ws_url.to_string()),
                ..cfg.clone()
            });
    let effective_cfg: &Config = ws_cfg_holder.as_ref().unwrap_or(cfg);

    let spinner = Spinner::new("running crawl");
    let (http_summary, http_seen_urls) = run_crawl_once(
        effective_cfg,
        start_url,
        initial_mode,
        &cfg.output_dir,
        None,
        false,
        previous_manifest.clone(),
        None,
    )
    .await?;
    spinner.finish(&format!(
        "crawl phase complete (pages={}, markdown={})",
        http_summary.pages_seen, http_summary.markdown_files
    ));

    let (summary, seen_urls) = maybe_chrome_fallback(
        effective_cfg,
        start_url,
        http_summary,
        http_seen_urls,
        previous_manifest,
    )
    .await;

    Ok((summary, seen_urls, ws_cfg_holder))
}

/// Queue an optional embed job, update the `latest` reflink, write the audit diff,
/// and emit the final structured log line.
async fn finalize_crawl(
    cfg: &Config,
    start_url: &str,
    domain: &str,
    manifest_path: &std::path::Path,
    previous_urls: &HashSet<String>,
    final_summary: &CrawlSummary,
) -> Result<(), Box<dyn Error>> {
    if cfg.embed {
        let markdown_dir = cfg.output_dir.join("markdown");
        let embed_job_id = start_embed_job(cfg, &markdown_dir.to_string_lossy()).await?;
        println!(
            "{} {}",
            muted("Queued embed job:"),
            accent(&embed_job_id.to_string())
        );
    }

    let current_urls = read_manifest_urls(manifest_path).await?;

    let latest_dir = cfg
        .output_dir
        .parent()
        .ok_or_else(|| {
            format!(
                "output_dir '{}' has no parent directory",
                cfg.output_dir.display()
            )
        })?
        .join("latest");
    if let Err(err) = update_latest_reflink(&cfg.output_dir, &latest_dir).await {
        println!(
            "{} failed to update 'latest' reflink for domain {}: {err}",
            muted("[Warning]"),
            domain
        );
    }

    let (report_path, _) = write_audit_diff(
        &cfg.output_dir,
        start_url,
        previous_urls,
        &current_urls,
        false,
        None,
    )
    .await?;
    log_done(&format!(
        "command=crawl pages_seen={} markdown_files={} thin_pages={} error_pages={} waf_blocked={} elapsed_ms={} output_dir={} audit_report={}",
        final_summary.pages_seen,
        final_summary.markdown_files,
        final_summary.thin_pages,
        final_summary.error_pages,
        final_summary.waf_blocked_pages,
        final_summary.elapsed_ms,
        cfg.output_dir.to_string_lossy(),
        report_path.to_string_lossy(),
    ));
    Ok(())
}

pub(super) async fn run_sync_crawl(cfg: &Config, start_url: &str) -> Result<(), Box<dyn Error>> {
    if cfg.sitemap_only {
        return run_sitemap_only_crawl(cfg, start_url).await;
    }

    let domain = url_to_domain(start_url);
    let mut sync_cfg = cfg.clone();
    sync_cfg.output_dir = cfg.output_dir.join("domains").join(&domain).join("sync");
    let cfg = &sync_cfg;

    let manifest_path = cfg.output_dir.join("manifest.jsonl");
    let previous_manifest = if cfg.cache {
        read_manifest_data(&manifest_path).await?
    } else {
        HashMap::new()
    };
    let previous_urls: HashSet<String> = previous_manifest.keys().cloned().collect();

    if maybe_return_cached_result(cfg, start_url, &manifest_path, &previous_urls).await? {
        return Ok(());
    }

    let (mut final_summary, seen_urls, _ws_cfg_holder) =
        run_crawl_phase(cfg, start_url, previous_manifest).await?;

    if cfg.discover_sitemaps {
        // Spider-native sitemap already ran inside run_crawl_once() when run_sitemap=true.
        // append_robots_backfill() supplements that by parsing robots.txt Sitemap: directives,
        // which spider does not handle natively.
        //
        // Re-read the manifest to merge any URLs that were written to disk by run_crawl_once
        // but not surfaced in `seen_urls` (e.g. URLs discovered via Spider's own sitemap pass).
        // This prevents double-fetching pages that were already crawled.
        let merged_seen = {
            let manifest_urls = read_manifest_urls(&manifest_path).await.unwrap_or_default();
            seen_urls
                .iter()
                .cloned()
                .chain(manifest_urls)
                .collect::<HashSet<String>>()
        };
        let spinner = Spinner::new("running robots.txt sitemap supplement");
        let robots_stats = super::audit::append_robots_backfill(
            cfg,
            start_url,
            &cfg.output_dir,
            &merged_seen,
            &mut final_summary,
        )
        .await?;
        spinner.finish(&format!(
            "robots.txt supplement complete (written={})",
            robots_stats.written
        ));
    }

    finalize_crawl(
        cfg,
        start_url,
        &domain,
        &manifest_path,
        &previous_urls,
        &final_summary,
    )
    .await
}
