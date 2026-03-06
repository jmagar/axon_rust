mod cdp_render;
mod collector;
mod runtime;
pub(crate) mod sitemap;
#[cfg(test)]
mod tests;
mod thin_refetch;
mod url_utils;

use crate::crates::core::config::{Config, RenderMode};
use crate::crates::core::content::build_transform_config;
use crate::crates::core::logging::{log_info, log_warn};
use crate::crates::crawl::manifest::ManifestEntry;
use collector::{CollectorConfig, collect_crawl_pages};
use runtime::configure_website;
#[cfg(test)]
use spider::website::Website;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::Path;
use std::time::Instant;
use tokio::sync::mpsc::Sender;

pub(crate) use runtime::resolve_cdp_ws_url;
pub use sitemap::{BackfillStats, append_sitemap_backfill};
pub(crate) use sitemap::{SitemapDiscovery, discover_sitemap_urls};
pub(crate) use thin_refetch::chrome_refetch_thin_pages;
pub(crate) use url_utils::{canonicalize_url_for_dedupe, is_excluded_url_path};
#[cfg(test)]
pub(crate) use url_utils::{is_junk_discovered_url, regex_escape};

#[derive(Debug, Default, Clone)]
pub struct CrawlSummary {
    pub pages_seen: u32,
    pub markdown_files: u32,
    pub thin_pages: u32,
    pub reused_pages: u32,
    pub pages_discovered: u32,
    pub elapsed_ms: u128,
    /// Canonical URLs of pages that were below `min_markdown_chars`.
    /// Populated by the collector and used by the auto-switch path to
    /// perform targeted per-URL Chrome re-fetches instead of a full re-crawl.
    pub thin_urls: HashSet<String>,
    /// Pages skipped due to non-2xx HTTP status codes.
    pub error_pages: u32,
    /// Pages blocked by a WAF or anti-bot system (`waf_check || blocked_crawl`).
    pub waf_blocked_pages: u32,
    /// Canonical URLs of WAF-blocked pages; used for targeted stealth Chrome retry.
    pub waf_blocked_urls: HashSet<String>,
}

pub fn should_fallback_to_chrome(summary: &CrawlSummary, max_pages: u32, cfg: &Config) -> bool {
    if summary.markdown_files == 0 {
        return true;
    }
    // A single-page crawl does not provide enough HTTP-only signal to judge
    // whether the captured content is complete, so give AutoSwitch one Chrome
    // retry even if the page is not technically "thin".
    if summary.pages_seen == 1 {
        return true;
    }
    let thin_ratio = if summary.pages_seen == 0 {
        1.0
    } else {
        summary.thin_pages as f64 / summary.pages_seen as f64
    };
    if thin_ratio > cfg.auto_switch_thin_ratio {
        return true;
    }
    // When max_pages == 0 (uncapped), there's no expected page count to compare
    // against, so "low coverage" is meaningless — skip that check entirely.
    if max_pages == 0 {
        return false;
    }
    summary.markdown_files < (max_pages / 10).max(cfg.auto_switch_min_pages as u32)
}

pub async fn crawl_and_collect_map(
    cfg: &Config,
    start_url: &str,
    mode: RenderMode,
) -> Result<(CrawlSummary, Vec<String>), Box<dyn Error>> {
    let mut website = configure_website(cfg, start_url, mode).await?;
    let start = Instant::now();

    match mode {
        RenderMode::Http => website.crawl_raw().await,
        RenderMode::Chrome | RenderMode::AutoSwitch => website.crawl().await,
    }

    let mut summary = CrawlSummary::default();
    let mut urls = Vec::new();
    let mut seen = HashSet::new();
    let exclude_path_prefix = cfg.exclude_path_prefix.clone();

    for link in website.get_links() {
        let page_url = link.as_ref().to_string();
        if is_excluded_url_path(&page_url, &exclude_path_prefix) {
            continue;
        }
        let Some(canonical_url) = canonicalize_url_for_dedupe(&page_url) else {
            continue;
        };
        if !seen.insert(canonical_url.clone()) {
            continue;
        }
        summary.pages_seen += 1;
        urls.push(canonical_url);
    }

    summary.elapsed_ms = start.elapsed().as_millis();
    Ok((summary, urls))
}

/// The unified result of a `map` operation: crawler-discovered URLs merged with
/// sitemap-discovered URLs, deduplicated and sorted.
#[derive(Debug, Default)]
pub struct MapResult {
    pub summary: CrawlSummary,
    /// All discovered URLs (crawler + sitemap), sorted and deduplicated.
    pub urls: Vec<String>,
    /// Raw number of URLs returned by `discover_sitemap_urls` before any
    /// deduplication against crawler-discovered URLs.  This is the count of
    /// `<loc>` entries in the sitemap(s), not the count of net-new URLs that
    /// were absent from the crawler results.
    pub sitemap_urls: usize,
}

/// Discover all URLs reachable from `start_url` and merge with sitemap
/// discovery, returning a single deduplicated sorted list.
///
/// Handles the AutoSwitch fallback to Chrome when HTTP finds zero pages.
/// Sitemap merge/sort/dedup happens here — callers receive a final unified set.
pub async fn map_with_sitemap(cfg: &Config, start_url: &str) -> Result<MapResult, Box<dyn Error>> {
    let initial_mode = match cfg.render_mode {
        RenderMode::AutoSwitch => RenderMode::Http,
        m => m,
    };

    let (mut summary, mut urls) = crawl_and_collect_map(cfg, start_url, initial_mode).await?;

    if matches!(cfg.render_mode, RenderMode::AutoSwitch) && summary.pages_seen == 0 {
        if let Ok((chrome_summary, chrome_urls)) =
            crawl_and_collect_map(cfg, start_url, RenderMode::Chrome).await
        {
            summary = chrome_summary;
            urls = chrome_urls;
        }
    }

    let raw_sitemap_count = if cfg.discover_sitemaps {
        let mut sitemap_url_list = discover_sitemap_urls(cfg, start_url).await?.urls;
        let count = sitemap_url_list.len();
        urls.append(&mut sitemap_url_list);
        urls.sort();
        urls.dedup();
        count
    } else {
        0
    };

    Ok(MapResult {
        summary,
        urls,
        sitemap_urls: raw_sitemap_count,
    })
}

pub async fn update_latest_reflink(
    source_dir: &Path,
    latest_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    // Guard against accidental self-delete or deleting the parent of source.
    if source_dir == latest_dir {
        return Err("source_dir and latest_dir must not be the same path".into());
    }
    if source_dir.starts_with(latest_dir) {
        return Err("source_dir must not be inside latest_dir".into());
    }

    // 1. Prepare clean slate
    if latest_dir.exists() {
        tokio::fs::remove_dir_all(latest_dir).await?;
    }
    tokio::fs::create_dir_all(latest_dir).await?;

    // 2. Reflink files recursively (shallow for simplicity, we only have one level of markdown)
    // We reflink manifest.jsonl and the markdown/ directory.
    let manifest = "manifest.jsonl";
    let source_manifest = source_dir.join(manifest);
    if source_manifest.exists() {
        let src = source_manifest.clone();
        let dst = latest_dir.join(manifest);
        tokio::task::spawn_blocking(move || reflink_copy::reflink_or_copy(&src, dst)).await??;
    }

    let markdown = "markdown";
    let source_md = source_dir.join(markdown);
    let target_md = latest_dir.join(markdown);
    if source_md.exists() {
        tokio::fs::create_dir_all(&target_md).await?;
        let mut entries = tokio::fs::read_dir(source_md).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                let Some(filename) = path.file_name() else {
                    continue;
                };
                let dst = target_md.join(filename);
                let src = path.clone();
                tokio::task::spawn_blocking(move || reflink_copy::reflink_or_copy(&src, dst))
                    .await??;
            }
        }
    }

    log_info(&format!(
        "Updated 'latest' armory view via reflink: {}",
        latest_dir.display()
    ));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn run_crawl_once(
    cfg: &Config,
    start_url: &str,
    mode: RenderMode,
    output_dir: &Path,
    progress_tx: Option<Sender<CrawlSummary>>,
    run_sitemap: bool,
    previous_manifest: HashMap<String, ManifestEntry>,
    crawl_id: Option<&str>,
) -> Result<(CrawlSummary, HashSet<String>), Box<dyn Error>> {
    let markdown_dir = output_dir.join("markdown");
    let recycling_bin = output_dir.join("markdown.old");

    if output_dir.exists() {
        if cfg.cache {
            // Recycling Bin Pattern: move existing markdown to .old for surgical reuse
            if markdown_dir.exists() {
                if recycling_bin.exists() {
                    tokio::fs::remove_dir_all(&recycling_bin).await?;
                }
                tokio::fs::rename(&markdown_dir, &recycling_bin).await?;
                log_info(&format!(
                    "Archived existing spoils to recycling bin for incremental reuse: {}",
                    recycling_bin.display()
                ));
            }
        } else if std::env::var("AXON_NO_WIPE").is_err() {
            log_warn(&format!(
                "Clearing output directory before crawl: {}",
                output_dir.display()
            ));
            let mut entries = tokio::fs::read_dir(output_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                let meta = tokio::fs::symlink_metadata(&path).await?;
                if meta.is_symlink() || meta.is_file() {
                    tokio::fs::remove_file(&path).await?;
                } else if meta.is_dir() {
                    tokio::fs::remove_dir_all(&path).await?;
                }
            }
        }
    }
    tokio::fs::create_dir_all(&markdown_dir).await?;

    let mut website =
        runtime::configure_website_with_crawl_id(cfg, start_url, mode, crawl_id).await?;
    // Buffer at least max_pages worth of messages to prevent silent page drops
    // under high-throughput crawls (extreme/max profiles). Clamp to 16 384 so
    // a large --max-pages value can't allocate an unbounded broadcast ring buffer.
    let subscribe_buf = (cfg.max_pages as usize).clamp(4096, 16_384);
    let rx = website
        .subscribe(subscribe_buf)
        .ok_or("failed to subscribe to spider broadcast channel")?;
    let markdown_dir = output_dir.join("markdown");
    let manifest_path = output_dir.join("manifest.jsonl");

    let min_chars = cfg.min_markdown_chars;
    let drop_thin = cfg.drop_thin_markdown;
    let exclude_path_prefix = cfg.exclude_path_prefix.clone();
    let crawl_start = Instant::now();
    let transform_cfg = build_transform_config();

    // Enable inline Chrome re-rendering when the *config* requests AutoSwitch,
    // even though `mode` is `Http` for the initial crawl phase (AutoSwitch
    // always starts with HTTP — `resolve_initial_mode` converts AutoSwitch→Http).
    // Chrome mode does its own rendering; Http mode with no AutoSwitch intent
    // has no Chrome target. When cfg.render_mode is AutoSwitch and Chrome is
    // configured, thin pages are re-rendered immediately while the HTTP crawl
    // continues — no second pass needed.
    let inline_chrome_ws_url = if matches!(cfg.render_mode, RenderMode::AutoSwitch) {
        cfg.chrome_remote_url.clone()
    } else {
        None
    };

    let join = tokio::spawn(collect_crawl_pages(
        rx,
        CollectorConfig {
            markdown_dir,
            manifest_path,
            min_chars,
            drop_thin,
            exclude_path_prefix,
            transform_cfg,
            progress_tx,
            previous_manifest,
            chrome_ws_url: inline_chrome_ws_url,
            chrome_timeout_secs: cfg.chrome_network_idle_timeout_secs,
            output_dir: output_dir.to_path_buf(),
        },
    ));

    // Spider-native sitemap phase: pages flow through the live subscription above.
    // persist_links() carries accumulated sitemap links into the subsequent main crawl.
    if run_sitemap && cfg.discover_sitemaps {
        website.crawl_sitemap().await;
        website.persist_links();
    }

    match mode {
        RenderMode::Http => website.crawl_raw().await,
        RenderMode::Chrome | RenderMode::AutoSwitch => website.crawl().await,
    }
    website.unsubscribe();

    let (mut summary, urls) = join
        .await
        .map_err(|e| format!("collector join failure: {e}"))?
        .map_err(|e| format!("collector failure: {e}"))?;
    summary.elapsed_ms = crawl_start.elapsed().as_millis();

    if recycling_bin.exists() {
        tokio::fs::remove_dir_all(&recycling_bin).await?;
        log_info("Purged recycling bin — armory is now synchronized with battlefield.");
    }

    Ok((summary, urls))
}

/// Crawl only the sitemap — no follow-on main crawl.
/// Pages flow through the same subscription pipeline as `run_crawl_once`.
pub async fn run_sitemap_only(
    cfg: &Config,
    start_url: &str,
    output_dir: &Path,
    previous_manifest: HashMap<String, ManifestEntry>,
) -> Result<(CrawlSummary, HashSet<String>), Box<dyn Error>> {
    tokio::fs::create_dir_all(output_dir.join("markdown")).await?;

    let mut website = configure_website(cfg, start_url, cfg.render_mode).await?;
    // Override the default set by configure_website: sitemap IS the crawl here.
    website.with_ignore_sitemap(false);

    let subscribe_buf = (cfg.max_pages as usize).clamp(4096, 16_384);
    let rx = website
        .subscribe(subscribe_buf)
        .ok_or("failed to subscribe to spider broadcast channel")?;
    let manifest_path = output_dir.join("manifest.jsonl");
    let markdown_dir = output_dir.join("markdown");
    let transform_cfg = build_transform_config();
    let crawl_start = Instant::now();

    let join = tokio::spawn(collect_crawl_pages(
        rx,
        CollectorConfig {
            markdown_dir,
            manifest_path,
            min_chars: cfg.min_markdown_chars,
            drop_thin: cfg.drop_thin_markdown,
            exclude_path_prefix: cfg.exclude_path_prefix.clone(),
            transform_cfg,
            progress_tx: None,
            previous_manifest,
            // Sitemap-only crawl: no inline Chrome rendering (HTTP-only path).
            chrome_ws_url: None,
            chrome_timeout_secs: cfg.chrome_network_idle_timeout_secs,
            output_dir: output_dir.to_path_buf(),
        },
    ));

    website.crawl_sitemap().await;
    website.unsubscribe();

    let (mut summary, urls) = join
        .await
        .map_err(|e| format!("collector join failure: {e}"))?
        .map_err(|e| format!("collector failure: {e}"))?;
    summary.elapsed_ms = crawl_start.elapsed().as_millis();

    Ok((summary, urls))
}
