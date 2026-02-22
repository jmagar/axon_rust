mod collector;
#[cfg(test)]
mod tests;

use crate::crates::core::config::parse::is_docker_service_host;
use crate::crates::core::config::{Config, RenderMode};
use crate::crates::core::content::build_transform_config;
use crate::crates::core::http::{cdp_discovery_url, ssrf_blacklist_patterns};
use crate::crates::core::logging::{log_info, log_warn};
use collector::collect_crawl_pages;
use spider::features::chrome_common::RequestInterceptConfiguration;
use spider::url::Url;
use spider::website::Website;
use std::collections::HashSet;
use std::error::Error;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Default, Clone)]
pub struct CrawlSummary {
    pub pages_seen: u32,
    pub markdown_files: u32,
    pub thin_pages: u32,
    pub elapsed_ms: u128,
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
        prefix.to_owned()
    } else {
        format!("/{prefix}")
    };
    let boundary = normalized.trim_end_matches('/');
    if boundary.is_empty() {
        return false;
    }
    path == boundary
        || path
            .strip_prefix(boundary)
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

/// Pre-resolve the Chrome DevTools WebSocket URL from the CDP discovery endpoint.
///
/// If `remote_url` is already a `ws://` / `wss://` URL (pre-resolved by the
/// bootstrap probe), return it directly without a second fetch — eliminating
/// the redundant `/json/version` round-trip when bootstrap succeeded.
///
/// Otherwise, fetch `/json/version`, extract `webSocketDebuggerUrl`, and rewrite
/// any known Docker service hostname (from the explicit allowlist) to `127.0.0.1`
/// so the host CLI can reach the Chrome proxy.
///
/// Returns `None` inside Docker (container hostnames resolve on the bridge
/// network) or when the fetch/parse fails.
async fn resolve_cdp_ws_url(remote_url: &str) -> Option<String> {
    // ws:// shortcut: bootstrap already resolved the URL — use it directly.
    if remote_url.starts_with("ws://") || remote_url.starts_with("wss://") {
        return Some(remote_url.to_string());
    }

    // Inside Docker the container hostname resolves on the Docker network.
    if Path::new("/.dockerenv").exists() {
        return None;
    }

    // Build the discovery URL (appends /json/version, converts ws→http).
    let discovery_url = cdp_discovery_url(remote_url)?;

    let client = crate::crates::core::http::http_client().ok()?;

    let body: serde_json::Value = client
        .get(&discovery_url)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let ws_url = body.get("webSocketDebuggerUrl")?.as_str()?;

    // Rewrite known Docker service hostnames to 127.0.0.1, preserving the port.
    let mut parsed = Url::parse(ws_url).ok()?;
    if let Some(host) = parsed.host_str() {
        let host = host.to_string();
        if is_docker_service_host(&host) {
            let _ = parsed.set_host(Some("127.0.0.1"));
        }
    }

    Some(parsed.to_string())
}

async fn configure_website(
    cfg: &Config,
    start_url: &str,
    mode: RenderMode,
) -> Result<Website, Box<dyn Error>> {
    let mut website = Website::new(start_url);
    website.with_depth(cfg.max_depth);
    website.with_subdomains(cfg.include_subdomains);
    // Disable TLD crawling unconditionally — we don't want to silently expand
    // example.com into example.co.uk, example.de, etc.  If TLD-scope crawling
    // is ever needed, add an explicit --include-tld flag.
    website.with_tld(false);

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
    // Wire retry count from config / performance profile.
    // with_retry takes u8; cfg.fetch_retries is usize — clamp to u8::MAX.
    if cfg.fetch_retries > 0 {
        website.with_retry(cfg.fetch_retries.min(u8::MAX as usize) as u8);
    }
    // Deduplicate trailing-slash URL variants when requested.
    website.with_normalize(cfg.normalize);

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
            // If remote_url is already a ws:// URL (threaded from the bootstrap
            // probe), resolve_cdp_ws_url returns it directly with no second fetch.
            // Otherwise it discovers via /json/version and normalises any Docker
            // hostname to 127.0.0.1.  Inside Docker, resolve_cdp_ws_url returns None
            // and we fall back to the discovery URL (spider.rs fetches it itself).
            let chrome_url = match resolve_cdp_ws_url(remote_url).await {
                Some(ws_url) => {
                    log_info(&format!("[Chrome] CDP WebSocket resolved: {ws_url}"));
                    ws_url
                }
                None => cdp_discovery_url(remote_url).unwrap_or_else(|| remote_url.to_string()),
            };
            website.with_chrome_connection(Some(chrome_url));
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
            .map_err(|e| format!("failed to build website with chrome settings: {e}"))?;
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

    // We always control the sitemap phase explicitly via run_crawl_once(run_sitemap: bool).
    // Prevent spider from auto-running sitemap during crawl()/crawl_raw().
    website.with_ignore_sitemap(true);

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

pub async fn run_crawl_once(
    cfg: &Config,
    start_url: &str,
    mode: RenderMode,
    output_dir: &Path,
    progress_tx: Option<UnboundedSender<CrawlSummary>>,
    run_sitemap: bool,
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

    let mut website = configure_website(cfg, start_url, mode).await?;
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

    let join = tokio::spawn(collect_crawl_pages(
        rx,
        markdown_dir,
        manifest_path,
        min_chars,
        drop_thin,
        exclude_path_prefix,
        transform_cfg,
        progress_tx,
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

    Ok((summary, urls))
}

/// Crawl only the sitemap — no follow-on main crawl.
/// Pages flow through the same subscription pipeline as `run_crawl_once`.
pub async fn run_sitemap_only(
    cfg: &Config,
    start_url: &str,
    output_dir: &Path,
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
        markdown_dir,
        manifest_path,
        cfg.min_markdown_chars,
        cfg.drop_thin_markdown,
        cfg.exclude_path_prefix.clone(),
        transform_cfg,
        None,
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
