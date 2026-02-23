use crate::crates::core::config::{Config, RenderMode};
use crate::crates::core::content::url_to_domain;
use crate::crates::core::logging::log_done;
use crate::crates::core::ui::{accent, muted, Spinner};
use crate::crates::crawl::engine::{
    run_crawl_once, run_sitemap_only, should_fallback_to_chrome, update_latest_reflink,
    CrawlSummary,
};
use crate::crates::crawl::manifest::{
    manifest_cache_is_stale, read_manifest_data, read_manifest_urls, write_audit_diff,
};
use crate::crates::jobs::embed_jobs::start_embed_job;
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
    let spinner = Spinner::new("HTTP yielded thin results; retrying with Chrome");
    match run_crawl_once(
        cfg,
        start_url,
        RenderMode::Chrome,
        &cfg.output_dir,
        None,
        cfg.discover_sitemaps,
        previous_manifest,
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

    let initial_mode = super::runtime::resolve_initial_mode(cfg);
    let chrome_bootstrap = super::runtime::bootstrap_chrome_runtime(cfg).await;
    for warning in &chrome_bootstrap.warnings {
        println!("{} {}", muted("[Chrome Bootstrap]"), warning);
    }

    // Thread the pre-resolved WebSocket URL through cfg so configure_website
    // skips the redundant /json/version fetch on Chrome mode calls.
    let ws_cfg_holder: Config;
    let effective_cfg: &Config = if let Some(ref ws_url) = chrome_bootstrap.resolved_ws_url {
        ws_cfg_holder = Config {
            chrome_remote_url: Some(ws_url.clone()),
            ..cfg.clone()
        };
        &ws_cfg_holder
    } else {
        cfg
    };

    let spinner = Spinner::new("running crawl");
    let (http_summary, http_seen_urls) = run_crawl_once(
        effective_cfg,
        start_url,
        initial_mode,
        &cfg.output_dir,
        None,
        false,
        previous_manifest.clone(),
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

    let mut final_summary = summary;

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

    if cfg.embed {
        let markdown_dir = cfg.output_dir.join("markdown");
        let embed_job_id = start_embed_job(cfg, &markdown_dir.to_string_lossy()).await?;
        println!(
            "{} {}",
            muted("Queued embed job:"),
            accent(&embed_job_id.to_string())
        );
    }

    let current_urls = read_manifest_urls(&manifest_path).await?;

    let latest_dir = cfg.output_dir.parent().unwrap().join("latest");
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
        &previous_urls,
        &current_urls,
        false,
        None,
    )
    .await?;
    log_done(&format!(
        "command=crawl pages_seen={} markdown_files={} thin_pages={} elapsed_ms={} output_dir={} audit_report={}",
        final_summary.pages_seen,
        final_summary.markdown_files,
        final_summary.thin_pages,
        final_summary.elapsed_ms,
        cfg.output_dir.to_string_lossy(),
        report_path.to_string_lossy(),
    ));
    Ok(())
}
