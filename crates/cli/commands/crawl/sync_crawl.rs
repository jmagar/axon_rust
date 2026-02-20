use crate::axon_cli::crates::core::config::{Config, RenderMode};
use crate::axon_cli::crates::core::logging::log_done;
use crate::axon_cli::crates::core::ui::{accent, muted, Spinner};
use crate::axon_cli::crates::crawl::engine::{
    append_sitemap_backfill, run_crawl_once, should_fallback_to_chrome,
};
use crate::axon_cli::crates::jobs::embed_jobs::start_embed_job;
use std::collections::HashSet;
use std::error::Error;
use std::time::SystemTime;

pub(super) fn manifest_cache_is_stale(manifest_path: &std::path::Path, ttl_secs: u64) -> bool {
    manifest_path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|mtime| SystemTime::now().duration_since(mtime).ok())
        .is_some_and(|age| age.as_secs() > ttl_secs)
}

pub(super) async fn maybe_return_cached_result(
    cfg: &Config,
    start_url: &str,
    manifest_path: &std::path::Path,
    previous_urls: &HashSet<String>,
) -> Result<bool, Box<dyn Error>> {
    let cache_stale = manifest_cache_is_stale(manifest_path, 24 * 60 * 60);
    if !cfg.cache || previous_urls.is_empty() || cache_stale {
        return Ok(false);
    }
    let report_path = super::manifest::write_audit_diff(
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

pub(super) async fn run_sync_crawl(cfg: &Config, start_url: &str) -> Result<(), Box<dyn Error>> {
    let manifest_path = cfg.output_dir.join("manifest.jsonl");
    let previous_urls = if cfg.cache {
        super::manifest::read_manifest_urls(&manifest_path).await?
    } else {
        HashSet::new()
    };
    if maybe_return_cached_result(cfg, start_url, &manifest_path, &previous_urls).await? {
        return Ok(());
    }

    let initial_mode = super::runtime::resolve_initial_mode(cfg);
    let chrome_bootstrap = super::runtime::bootstrap_chrome_runtime(cfg).await;
    for warning in &chrome_bootstrap.warnings {
        println!("{} {}", muted("[Chrome Bootstrap]"), warning);
    }

    let spinner = Spinner::new("running crawl");
    let (http_summary, http_seen_urls) =
        run_crawl_once(cfg, start_url, initial_mode, &cfg.output_dir, None).await?;
    spinner.finish(&format!(
        "crawl phase complete (pages={}, markdown={})",
        http_summary.pages_seen, http_summary.markdown_files
    ));

    let (summary, seen_urls) = if matches!(cfg.render_mode, RenderMode::AutoSwitch)
        && should_fallback_to_chrome(&http_summary, cfg.max_pages)
    {
        let chrome_spinner = Spinner::new("HTTP yielded thin results; retrying with Chrome");
        match run_crawl_once(cfg, start_url, RenderMode::Chrome, &cfg.output_dir, None).await {
            Ok((chrome_summary, chrome_urls)) => {
                chrome_spinner.finish(&format!(
                    "Chrome fallback complete (pages={}, markdown={})",
                    chrome_summary.pages_seen, chrome_summary.markdown_files
                ));
                (chrome_summary, chrome_urls)
            }
            Err(err) => {
                chrome_spinner.finish(&format!(
                    "Chrome fallback failed ({err}), using HTTP result"
                ));
                (http_summary, http_seen_urls)
            }
        }
    } else {
        (http_summary, http_seen_urls)
    };

    let mut final_summary = summary;

    if cfg.discover_sitemaps {
        let spinner = Spinner::new("running sitemap backfill");
        let _ = append_sitemap_backfill(
            cfg,
            start_url,
            &cfg.output_dir,
            &seen_urls,
            &mut final_summary,
        )
        .await?;
        let robots_stats = super::audit::append_robots_backfill(
            cfg,
            start_url,
            &cfg.output_dir,
            &seen_urls,
            &mut final_summary,
        )
        .await?;
        spinner.finish(&format!(
            "sitemap backfill complete (robots_extra_written={})",
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

    let current_urls = super::manifest::read_manifest_urls(&manifest_path).await?;
    let report_path = super::manifest::write_audit_diff(
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
