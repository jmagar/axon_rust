mod sitemap;

#[cfg(test)]
mod tests;

pub use sitemap::{append_sitemap_backfill, crawl_sitemap_urls};

use crate::axon_cli::crates::core::config::{Config, RenderMode};
use crate::axon_cli::crates::core::content::{build_transform_config, url_to_filename};
use crate::axon_cli::crates::core::http::ssrf_blacklist_patterns;
use crate::axon_cli::crates::core::logging::{log_info, log_warn};
use spider::features::chrome_common::RequestInterceptConfiguration;
use spider::tokio;
use spider::url::Url;
use spider::website::Website;
use spider_transformations::transformation::content::{transform_content_input, TransformInput};
use std::collections::HashSet;
use std::error::Error;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Default, Clone)]
pub struct CrawlSummary {
    pub pages_seen: u32,
    pub markdown_files: u32,
    pub thin_pages: u32,
    pub elapsed_ms: u128,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SitemapBackfillStats {
    pub sitemap_discovered: usize,
    pub sitemap_candidates: usize,
    pub processed: usize,
    pub fetched_ok: usize,
    pub written: usize,
    pub failed: usize,
    pub filtered: usize,
}

pub(crate) fn canonicalize_url_for_dedupe(url: &str) -> Option<String> {
    let mut parsed = Url::parse(url).ok()?;
    parsed.set_fragment(None);

    match (parsed.scheme(), parsed.port()) {
        ("http", Some(80)) | ("https", Some(443)) => {
            let _ = parsed.set_port(None);
        }
        _ => {}
    }

    let path = parsed.path().to_string();
    if path.len() > 1 {
        let normalized_path = path.trim_end_matches('/').to_string();
        parsed.set_path(&normalized_path);
    }

    Some(parsed.to_string())
}

pub(crate) fn is_excluded_url_path(url: &str, excludes: &[String]) -> bool {
    if excludes.is_empty() {
        return false;
    }
    let path = Url::parse(url)
        .ok()
        .map(|u| u.path().to_string())
        .unwrap_or_else(|| "/".to_string());
    excludes
        .iter()
        .any(|prefix| is_path_prefix_excluded(&path, prefix))
}

fn is_path_prefix_excluded(path: &str, prefix: &str) -> bool {
    let normalized = if prefix.starts_with('/') {
        prefix
    } else {
        return is_path_prefix_excluded(path, &format!("/{prefix}"));
    };
    let boundary_prefix = normalized.trim_end_matches('/');
    if boundary_prefix.is_empty() {
        return false;
    }
    path == boundary_prefix
        || path
            .strip_prefix(boundary_prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn regex_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 8);
    for ch in value.chars() {
        match ch {
            '.' | '+' | '*' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\'
            | '-' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn build_exclude_blacklist_patterns(start_url: &str, excludes: &[String]) -> Vec<String> {
    let host_pattern = Url::parse(start_url)
        .ok()
        .and_then(|u| u.host_str().map(regex_escape))
        .unwrap_or_else(|| "[^/]+".to_string());

    excludes
        .iter()
        .map(|prefix| {
            let normalized = if prefix.starts_with('/') {
                prefix.clone()
            } else {
                format!("/{prefix}")
            };
            format!(
                "^https?://{}{}(?:/|$|\\?|#)",
                host_pattern,
                regex_escape(&normalized)
            )
        })
        .collect()
}

/// Ensure the Chrome DevTools Protocol URL includes the `/json/version` discovery path.
/// chromiumoxide requires `http://host:port/json/version`, not bare `http://host:port`.
fn normalize_cdp_url(url: &str) -> String {
    match Url::parse(url) {
        Ok(parsed) if parsed.path() == "/" || parsed.path().is_empty() => {
            format!("{}/json/version", url.trim_end_matches('/'))
        }
        _ => url.to_string(),
    }
}

fn configure_website(
    cfg: &Config,
    start_url: &str,
    mode: RenderMode,
) -> Result<Website, Box<dyn Error>> {
    let mut website = Website::new(start_url);
    website.with_depth(cfg.max_depth);
    website.with_subdomains(cfg.include_subdomains);
    website.with_tld(cfg.include_subdomains);

    if cfg.max_pages > 0 {
        website.with_limit(cfg.max_pages);
    }

    if cfg.respect_robots {
        website.with_respect_robots_txt(true);
    }
    if let Some(limit) = cfg.crawl_concurrency_limit {
        website.with_concurrency_limit(Some(limit.max(1)));
    }
    if cfg.delay_ms > 0 {
        website.with_delay(cfg.delay_ms);
    }
    if cfg.shared_queue {
        website.with_shared_queue(true);
    }
    // Always apply SSRF protection. Append path exclusions if configured.
    let mut blacklist_patterns: Vec<spider::compact_str::CompactString> = ssrf_blacklist_patterns()
        .into_iter()
        .map(Into::into)
        .collect();
    if !cfg.exclude_path_prefix.is_empty() {
        blacklist_patterns.extend(
            build_exclude_blacklist_patterns(start_url, &cfg.exclude_path_prefix)
                .into_iter()
                .map(Into::into),
        );
    }
    website.with_blacklist_url(Some(blacklist_patterns));
    if let Some(timeout_ms) = cfg.request_timeout_ms {
        website.with_request_timeout(Some(Duration::from_millis(timeout_ms)));
    }

    if let Some(ref proxy) = cfg.chrome_proxy {
        website.with_proxies(Some(vec![proxy.clone()]));
    }
    if let Some(ref ua) = cfg.chrome_user_agent {
        website.with_user_agent(Some(ua.as_str()));
    }

    if matches!(mode, RenderMode::Chrome) {
        // CDP path — primary browser mode. chromiumoxide connects directly via CDP,
        // giving access to stealth, fingerprint, intercept, and network-idle features.
        website
            .with_chrome_intercept(RequestInterceptConfiguration::new(cfg.chrome_intercept))
            .with_stealth(cfg.chrome_stealth || cfg.chrome_anti_bot)
            .with_fingerprint(true);
        if let Some(ref remote_url) = cfg.chrome_remote_url {
            // chromiumoxide requires the /json/version CDP discovery endpoint.
            website.with_chrome_connection(Some(normalize_cdp_url(remote_url)));
        }
        // `idle_network0` calls `wait_for_network_idle()` — waits until the network
        // has been fully quiet for 500 ms. This is essential for CSR frameworks
        // (React, Vue, etc.) that run XHR/fetch calls during hydration AFTER the
        // initial HTML load. `idle_network` (EventLoadingFinished) fires too early.
        website.with_wait_for_idle_network0(Some(spider::configuration::WaitForIdleNetwork::new(
            Some(Duration::from_secs(15)),
        )));
        website = website
            .build()
            .map_err(|_| "Failed to build website with chrome settings")?;
    } else if let Some(ref wd_url) = cfg.webdriver_url {
        // Selenium/WebDriver — secondary path when CDP remote URL is unavailable.
        use spider::features::webdriver_common::{WebDriverBrowser, WebDriverConfig};
        let wd_cfg = WebDriverConfig {
            server_url: wd_url.clone(),
            browser: WebDriverBrowser::Chrome,
            headless: cfg.chrome_headless,
            proxy: cfg.chrome_proxy.clone(),
            user_agent: cfg.chrome_user_agent.clone(),
            ..WebDriverConfig::default()
        };
        website.with_webdriver(wd_cfg);
        // Same fully-idle wait: WebDriver also needs to wait for JS hydration.
        website.with_wait_for_idle_network0(Some(spider::configuration::WaitForIdleNetwork::new(
            Some(Duration::from_secs(15)),
        )));
    }

    Ok(website)
}

pub fn should_fallback_to_chrome(summary: &CrawlSummary, max_pages: u32) -> bool {
    if summary.markdown_files == 0 {
        return true;
    }
    let thin_ratio = if summary.pages_seen == 0 {
        1.0
    } else {
        summary.thin_pages as f64 / summary.pages_seen as f64
    };
    if thin_ratio > 0.60 {
        return true;
    }
    // When max_pages == 0 (uncapped), there's no expected page count to compare
    // against, so "low coverage" is meaningless — skip that check entirely.
    if max_pages == 0 {
        return false;
    }
    summary.markdown_files < (max_pages / 10).max(10)
}

pub async fn crawl_and_collect_map(
    cfg: &Config,
    start_url: &str,
    mode: RenderMode,
) -> Result<(CrawlSummary, Vec<String>), Box<dyn Error>> {
    let mut website = configure_website(cfg, start_url, mode)?;
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

pub async fn run_crawl_once(
    cfg: &Config,
    start_url: &str,
    mode: RenderMode,
    output_dir: &Path,
    progress_tx: Option<UnboundedSender<CrawlSummary>>,
) -> Result<(CrawlSummary, HashSet<String>), Box<dyn Error>> {
    if output_dir.exists() {
        if std::env::var("AXON_NO_WIPE").is_ok() {
            log_info(&format!(
                "AXON_NO_WIPE set — keeping existing output dir: {}",
                output_dir.display()
            ));
        } else {
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
    tokio::fs::create_dir_all(output_dir.join("markdown")).await?;

    let mut website = configure_website(cfg, start_url, mode)?;
    // Buffer at least max_pages worth of messages to prevent silent page drops
    // under high-throughput crawls (extreme/max profiles). Clamp to 16 384 so
    // a large --max-pages value can't allocate an unbounded broadcast ring buffer.
    let subscribe_buf = (cfg.max_pages as usize).clamp(4096, 16_384);
    let mut rx = website
        .subscribe(subscribe_buf)
        .ok_or("failed to subscribe to spider broadcast channel")?;
    let markdown_dir = output_dir.join("markdown");
    let manifest_path = output_dir.join("manifest.jsonl");

    let min_chars = cfg.min_markdown_chars;
    let drop_thin = cfg.drop_thin_markdown;
    let exclude_path_prefix = cfg.exclude_path_prefix.clone();
    let crawl_start = Instant::now();
    let transform_cfg = build_transform_config();

    let join = tokio::spawn(async move {
        let manifest_file = tokio::fs::File::create(&manifest_path)
            .await
            .map_err(|e| format!("manifest create failed: {e}"))?;
        let mut manifest = tokio::io::BufWriter::new(manifest_file);
        let mut summary = CrawlSummary::default();
        let mut urls = HashSet::new();
        let mut seen_canonical = HashSet::new();

        loop {
            let page = match rx.recv().await {
                Ok(page) => page,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    log_warn(&format!("crawl broadcast lagged: {skipped} pages dropped — increase subscribe buffer or reduce concurrency"));
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            };
            let raw_url = page.get_url().to_string();
            if is_excluded_url_path(&raw_url, &exclude_path_prefix) {
                continue;
            }
            let Some(url) = canonicalize_url_for_dedupe(&raw_url) else {
                continue;
            };
            if !seen_canonical.insert(url.clone()) {
                continue;
            }
            summary.pages_seen += 1;
            urls.insert(url.clone());

            let input = TransformInput {
                url: None,
                content: page.get_html_bytes_u8(),
                screenshot_bytes: None,
                encoding: None,
                selector_config: None,
                ignore_tags: None,
            };
            let markdown = transform_content_input(input, transform_cfg);
            let trimmed = markdown.trim(); // &str borrow — zero allocation
                                           // byte length; sitemap/doc content is ASCII-dominant so bytes ≈ chars
            let chars = trimmed.len();

            if chars < min_chars {
                summary.thin_pages += 1;
                if drop_thin {
                    if summary.pages_seen.is_multiple_of(25) {
                        if let Some(tx) = progress_tx.as_ref() {
                            let _ = tx.send(summary.clone());
                        }
                    }
                    continue;
                }
            }
            if trimmed.is_empty() {
                if summary.pages_seen.is_multiple_of(25) {
                    if let Some(tx) = progress_tx.as_ref() {
                        let _ = tx.send(summary.clone());
                    }
                }
                continue;
            }

            summary.markdown_files += 1;
            let filename = url_to_filename(&url, summary.markdown_files);
            let path = markdown_dir.join(filename);
            tokio::fs::write(&path, trimmed.as_bytes())
                .await
                .map_err(|e| format!("write failed: {e}"))?;
            let rec = serde_json::json!({"url": url, "file_path": path.to_string_lossy(), "markdown_chars": chars});
            let mut line = rec.to_string();
            line.push('\n');
            manifest
                .write_all(line.as_bytes())
                .await
                .map_err(|e| format!("manifest failed: {e}"))?;

            if summary.pages_seen.is_multiple_of(25) {
                if let Some(tx) = progress_tx.as_ref() {
                    let _ = tx.send(summary.clone());
                }
            }
        }

        manifest
            .flush()
            .await
            .map_err(|e| format!("manifest flush failed: {e}"))?;
        if let Some(tx) = progress_tx.as_ref() {
            let _ = tx.send(summary.clone());
        }
        Ok::<(CrawlSummary, HashSet<String>), String>((summary, urls))
    });

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

    Ok((summary, urls))
}
