use crate::axon_cli::crates::core::config::{Config, RenderMode};
use crate::axon_cli::crates::core::content::{
    build_transform_config, extract_loc_values, to_markdown, url_to_filename,
};
use crate::axon_cli::crates::core::http::validate_url;
use crate::axon_cli::crates::core::logging::{log_info, log_warn};
use spider::features::chrome_common::RequestInterceptConfiguration;
use spider::tokio;
use spider::url::Url;
use spider::website::Website;
use spider_transformations::transformation::content::{transform_content_input, TransformInput};
use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;
use tokio::io::AsyncWriteExt;

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

fn configure_website(
    cfg: &Config,
    start_url: &str,
    mode: RenderMode,
) -> Result<Website, Box<dyn Error>> {
    let mut website = Website::new(start_url);
    website.with_depth(cfg.max_depth);
    website.with_subdomains(cfg.include_subdomains);
    // Include root-domain siblings when crawling from a subdomain (e.g. code.claude.com -> claude.com).
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

    if matches!(mode, RenderMode::Chrome) {
        website
            .with_chrome_intercept(RequestInterceptConfiguration::new(false))
            .with_stealth(true);
        website = website
            .build()
            .map_err(|_| "Failed to build website with chrome settings")?;
    }

    Ok(website)
}

fn should_retry_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

async fn fetch_text_with_retry(
    client: &reqwest::Client,
    url: &str,
    retries: usize,
    backoff_ms: u64,
) -> Option<String> {
    if validate_url(url).is_err() {
        return None;
    }
    let mut attempt = 0usize;
    loop {
        match client.get(url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    return resp.text().await.ok();
                }
                if attempt >= retries || !should_retry_status(status) {
                    return None;
                }
            }
            Err(_) if attempt >= retries => return None,
            Err(_) => {}
        }

        attempt = attempt.saturating_add(1);
        tokio::time::sleep(Duration::from_millis(
            backoff_ms.saturating_mul(attempt as u64).max(1),
        ))
        .await;
    }
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
    let low_coverage = summary.markdown_files < (max_pages / 10).max(10);
    thin_ratio > 0.60 || low_coverage
}

pub async fn crawl_and_collect_map(
    cfg: &Config,
    start_url: &str,
    mode: RenderMode,
) -> Result<(CrawlSummary, Vec<String>), Box<dyn Error>> {
    let mut website = configure_website(cfg, start_url, mode)?;
    let mut rx = website.subscribe(4096).ok_or("subscribe failed")?;
    let start = Instant::now();

    let transform_cfg = build_transform_config();
    let join = tokio::spawn(async move {
        let mut summary = CrawlSummary::default();
        let mut urls = Vec::new();
        let mut seen = HashSet::new();

        loop {
            let page = match rx.recv().await {
                Ok(page) => page,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            };

            summary.pages_seen += 1;
            let page_url = page.get_url().to_string();
            if seen.insert(page_url.clone()) {
                urls.push(page_url);
            }

            let input = TransformInput {
                url: None,
                content: page.get_html_bytes_u8(),
                screenshot_bytes: None,
                encoding: None,
                selector_config: None,
                ignore_tags: None,
            };
            let markdown = transform_content_input(input, &transform_cfg);
            let chars = markdown.trim().chars().count();
            if chars < 200 {
                summary.thin_pages += 1;
            }
            if chars > 0 {
                summary.markdown_files += 1;
            }
        }

        Ok::<(CrawlSummary, Vec<String>), String>((summary, urls))
    });

    match mode {
        RenderMode::Http => website.crawl_raw().await,
        RenderMode::Chrome | RenderMode::AutoSwitch => website.crawl().await,
    }
    website.unsubscribe();

    let (mut summary, urls) = join
        .await
        .map_err(|e| format!("join failure: {e}"))?
        .map_err(|e| format!("collector failure: {e}"))?;
    summary.elapsed_ms = start.elapsed().as_millis();
    Ok((summary, urls))
}

pub async fn crawl_sitemap_urls(
    cfg: &Config,
    start_url: &str,
) -> Result<Vec<String>, Box<dyn Error>> {
    let parsed = Url::parse(start_url)?;
    let scheme = parsed.scheme().to_string();
    let host = parsed.host_str().ok_or("missing host")?.to_string();

    let mut queue = VecDeque::new();
    queue.push_back(format!("{scheme}://{host}/sitemap.xml"));
    queue.push_back(format!("{scheme}://{host}/sitemap_index.xml"));
    queue.push_back(format!("{scheme}://{host}/sitemap-index.xml"));

    let mut seen_sitemaps = HashSet::new();
    let mut out = HashSet::new();
    let timeout = Duration::from_millis(cfg.request_timeout_ms.unwrap_or(30_000));
    let client = reqwest::Client::builder().timeout(timeout).build()?;
    let start_host = host.clone();
    let start_path = parsed.path().trim_end_matches('/').to_string();
    let scoped_to_root = start_path.is_empty();
    let worker_limit = cfg
        .sitemap_concurrency_limit
        .unwrap_or(64)
        .clamp(1, 1024);
    let max_sitemaps = cfg.max_sitemaps.max(1);
    let mut parsed_sitemaps = 0usize;

    while !queue.is_empty() && parsed_sitemaps < max_sitemaps {
        let mut batch = Vec::new();
        while batch.len() < worker_limit && parsed_sitemaps + batch.len() < max_sitemaps {
            let Some(url) = queue.pop_front() else {
                break;
            };
            if seen_sitemaps.insert(url.clone()) {
                batch.push(url);
            }
        }
        if batch.is_empty() {
            break;
        }

        let mut joins = tokio::task::JoinSet::new();
        for sitemap_url in batch {
            let http = client.clone();
            let retries = cfg.fetch_retries;
            let backoff = cfg.retry_backoff_ms;
            joins.spawn(async move {
                fetch_text_with_retry(&http, &sitemap_url, retries, backoff)
                    .await
                    .map(|xml| (sitemap_url, xml))
            });
        }

        while let Some(joined) = joins.join_next().await {
            let Ok(Some((sitemap_url, xml))) = joined else {
                continue;
            };
            parsed_sitemaps += 1;
            let is_index = xml.to_ascii_lowercase().contains("<sitemapindex");
            for loc in extract_loc_values(&xml) {
                let Ok(u) = Url::parse(&loc) else {
                    continue;
                };
                let Some(h) = u.host_str() else {
                    continue;
                };
                let in_scope = if cfg.include_subdomains {
                    h == start_host || h.ends_with(&format!(".{start_host}"))
                } else {
                    h == start_host
                };
                if !in_scope {
                    continue;
                }
                if !scoped_to_root {
                    let p = u.path();
                    let exact = p == start_path;
                    let nested = p.starts_with(&(start_path.clone() + "/"));
                    if !exact && !nested {
                        continue;
                    }
                }

                if is_index {
                    if !seen_sitemaps.contains(&loc) {
                        queue.push_back(loc);
                    }
                } else {
                    out.insert(loc);
                }
            }
            if parsed_sitemaps % 64 == 0 {
                log_info(&format!(
                    "command=sitemap parsed={} discovered_urls={} queue={}",
                    parsed_sitemaps,
                    out.len(),
                    queue.len()
                ));
            }
            let _ = sitemap_url;
        }
    }

    let mut urls: Vec<String> = out.into_iter().collect();
    urls.sort();
    Ok(urls)
}

pub async fn run_crawl_once(
    cfg: &Config,
    start_url: &str,
    mode: RenderMode,
    output_dir: &Path,
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
            tokio::fs::remove_dir_all(output_dir).await?;
        }
    }
    tokio::fs::create_dir_all(output_dir.join("markdown")).await?;

    let mut website = configure_website(cfg, start_url, mode)?;
    let mut rx = website.subscribe(4096).ok_or("subscribe failed")?;
    let markdown_dir = output_dir.join("markdown");
    let manifest_path = output_dir.join("manifest.jsonl");

    let min_chars = cfg.min_markdown_chars;
    let drop_thin = cfg.drop_thin_markdown;
    let crawl_start = Instant::now();
    let transform_cfg = build_transform_config();

    let join = tokio::spawn(async move {
        let manifest_file = tokio::fs::File::create(&manifest_path)
            .await
            .map_err(|e| format!("manifest create failed: {e}"))?;
        let mut manifest = tokio::io::BufWriter::new(manifest_file);
        let mut summary = CrawlSummary::default();
        let mut urls = HashSet::new();

        loop {
            let page = match rx.recv().await {
                Ok(page) => page,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            };
            summary.pages_seen += 1;
            let url = page.get_url().to_string();
            urls.insert(url.clone());

            let input = TransformInput {
                url: None,
                content: page.get_html_bytes_u8(),
                screenshot_bytes: None,
                encoding: None,
                selector_config: None,
                ignore_tags: None,
            };
            let markdown = transform_content_input(input, &transform_cfg);
            let trimmed = markdown.trim().to_string();
            let chars = trimmed.chars().count();

            if chars < min_chars {
                summary.thin_pages += 1;
                if drop_thin {
                    continue;
                }
            }
            if trimmed.is_empty() {
                continue;
            }

            summary.markdown_files += 1;
            let filename = url_to_filename(&url, summary.markdown_files);
            let path = markdown_dir.join(filename);
            tokio::fs::write(&path, trimmed).await.map_err(|e| format!("write failed: {e}"))?;
            let rec = serde_json::json!({"url": url, "file_path": path.to_string_lossy(), "markdown_chars": chars});
            let mut line = rec.to_string();
            line.push('\n');
            manifest.write_all(line.as_bytes()).await.map_err(|e| format!("manifest failed: {e}"))?;
        }

        manifest
            .flush()
            .await
            .map_err(|e| format!("manifest flush failed: {e}"))?;
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

pub async fn try_auto_switch(
    cfg: &Config,
    start_url: &str,
    summary: &CrawlSummary,
    urls: &[String],
) -> Result<(CrawlSummary, Vec<String>), Box<dyn Error>> {
    if !matches!(cfg.render_mode, RenderMode::AutoSwitch)
        || !should_fallback_to_chrome(summary, cfg.max_pages)
    {
        return Ok((
            CrawlSummary {
                pages_seen: summary.pages_seen,
                markdown_files: summary.markdown_files,
                thin_pages: summary.thin_pages,
                elapsed_ms: summary.elapsed_ms,
            },
            urls.to_vec(),
        ));
    }

    log_warn("HTTP output looked thin/low-coverage; attempting chrome fallback");
    match crawl_and_collect_map(cfg, start_url, RenderMode::Chrome).await {
        Ok((chrome_summary, chrome_urls)) if !chrome_urls.is_empty() => {
            Ok((chrome_summary, chrome_urls))
        }
        _ => Ok((
            CrawlSummary {
                pages_seen: summary.pages_seen,
                markdown_files: summary.markdown_files,
                thin_pages: summary.thin_pages,
                elapsed_ms: summary.elapsed_ms,
            },
            urls.to_vec(),
        )),
    }
}

pub async fn append_sitemap_backfill(
    cfg: &Config,
    start_url: &str,
    output_dir: &Path,
    seen_urls: &HashSet<String>,
    summary: &mut CrawlSummary,
) -> Result<SitemapBackfillStats, Box<dyn Error>> {
    let sitemap_urls = crawl_sitemap_urls(cfg, start_url).await?;
    let sitemap_discovered = sitemap_urls.len();
    log_info(&format!(
        "command=crawl sitemap_backfill_discovered={} concurrency={}",
        sitemap_discovered,
        cfg.backfill_concurrency_limit
            .unwrap_or(cfg.batch_concurrency)
            .max(1)
    ));
    let markdown_dir = output_dir.join("markdown");
    let manifest_path = output_dir.join("manifest.jsonl");
    let manifest_file = tokio::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&manifest_path)
        .await?;
    let mut manifest = tokio::io::BufWriter::new(manifest_file);

    let timeout = Duration::from_millis(cfg.request_timeout_ms.unwrap_or(30_000));
    let client = reqwest::Client::builder().timeout(timeout).build()?;
    let mut idx = summary.markdown_files;
    let mut processed: usize = 0;
    let mut fetched_ok: usize = 0;
    let mut written: usize = 0;
    let mut failed_fetches: usize = 0;

    let mut pending = tokio::task::JoinSet::new();
    let candidates_vec: Vec<String> = sitemap_urls
        .into_iter()
        .filter(|url| !seen_urls.contains(url))
        .collect();
    let sitemap_candidates = candidates_vec.len();
    let mut candidates = candidates_vec.into_iter();
    let concurrency = cfg
        .backfill_concurrency_limit
        .unwrap_or(cfg.batch_concurrency)
        .max(1);

    let push_task = |set: &mut tokio::task::JoinSet<(String, Result<String, String>)>,
                     url: String,
                     http: reqwest::Client,
                     retries: usize,
                     backoff_ms: u64| {
        set.spawn(async move {
            let result = fetch_text_with_retry(&http, &url, retries, backoff_ms)
                .await
                .ok_or_else(|| format!("fetch failed for {url}"));
            (url, result)
        });
    };

    for _ in 0..concurrency {
        if let Some(url) = candidates.next() {
            push_task(
                &mut pending,
                url,
                client.clone(),
                cfg.fetch_retries,
                cfg.retry_backoff_ms,
            );
        }
    }

    while let Some(joined) = pending.join_next().await {
        processed += 1;
        match joined {
            Ok((url, Ok(html))) => {
                fetched_ok += 1;
                let md = to_markdown(&html);
                let chars = md.chars().count();
                if chars < cfg.min_markdown_chars {
                    summary.thin_pages += 1;
                }

                if chars >= cfg.min_markdown_chars || !cfg.drop_thin_markdown {
                    idx += 1;
                    let file = markdown_dir.join(url_to_filename(&url, idx));
                    tokio::fs::write(&file, md).await?;
                    let rec = serde_json::json!({
                        "url": url,
                        "file_path": file.to_string_lossy(),
                        "markdown_chars": chars,
                        "source": "sitemap_backfill"
                    });
                    let mut line = rec.to_string();
                    line.push('\n');
                    manifest.write_all(line.as_bytes()).await?;
                    summary.markdown_files += 1;
                    written += 1;
                }
            }
            Ok((url, Err(err))) => {
                let _ = url;
                let _ = err;
                failed_fetches += 1;
            }
            Err(err) => {
                let _ = err;
                failed_fetches += 1;
            }
        }

        if processed.is_multiple_of(50) {
            log_info(&format!(
                "command=crawl sitemap_backfill_progress processed={} fetched_ok={} written={} failed={}",
                processed, fetched_ok, written, failed_fetches
            ));
        }

        if let Some(url) = candidates.next() {
            push_task(
                &mut pending,
                url,
                client.clone(),
                cfg.fetch_retries,
                cfg.retry_backoff_ms,
            );
        }
    }

    manifest.flush().await?;
    log_info(&format!(
        "command=crawl sitemap_backfill_complete processed={} fetched_ok={} written={} failed={}",
        processed, fetched_ok, written, failed_fetches
    ));
    Ok(SitemapBackfillStats {
        sitemap_discovered,
        sitemap_candidates,
        processed,
        fetched_ok,
        written,
        failed: failed_fetches,
        filtered: processed.saturating_sub(written),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(pages_seen: u32, thin: u32, markdown_files: u32) -> CrawlSummary {
        CrawlSummary {
            pages_seen,
            thin_pages: thin,
            markdown_files,
            elapsed_ms: 0,
        }
    }

    #[test]
    fn test_fallback_when_no_markdown_files() {
        // markdown_files == 0 → always fallback (early return)
        assert!(should_fallback_to_chrome(&summary(100, 0, 0), 200));
    }

    #[test]
    fn test_fallback_thin_ratio_above_threshold() {
        // 61/100 thin → ratio 0.61 > 0.60 → should fallback
        assert!(should_fallback_to_chrome(&summary(100, 61, 50), 200));
    }

    #[test]
    fn test_no_fallback_at_threshold() {
        // exactly 60/100 thin → ratio 0.60 NOT > 0.60 → no fallback
        // markdown_files=50 >= max(200/10, 10) = 20 → coverage OK
        assert!(!should_fallback_to_chrome(&summary(100, 60, 50), 200));
    }

    #[test]
    fn test_fallback_low_coverage() {
        // thin_ratio = 10/100 = 0.10 (OK)
        // but markdown_files = 5 < max(200/10, 10) = 20 → low coverage → fallback
        assert!(should_fallback_to_chrome(&summary(100, 10, 5), 200));
    }

    #[test]
    fn test_no_divide_by_zero() {
        // pages_seen = 0 → thin_ratio defaults to 1.0 → should fallback
        // But markdown_files = 0 triggers the early return first
        assert!(should_fallback_to_chrome(&summary(0, 0, 0), 200));
    }

    #[test]
    fn test_no_fallback_healthy_crawl() {
        // 10/200 thin → ratio 0.05 (OK)
        // markdown_files = 150 >= max(200/10, 10) = 20 (OK)
        assert!(!should_fallback_to_chrome(&summary(200, 10, 150), 200));
    }

    #[test]
    fn test_fallback_low_max_pages() {
        // With max_pages=50: threshold = max(50/10, 10) = 10
        // markdown_files = 8 < 10 → low coverage → fallback
        assert!(should_fallback_to_chrome(&summary(50, 5, 8), 50));
    }

    #[test]
    fn test_no_fallback_small_crawl_sufficient_coverage() {
        // max_pages=50: threshold = max(50/10, 10) = 10
        // markdown_files = 15 >= 10 → coverage OK
        // thin_ratio = 5/50 = 0.10 → OK
        assert!(!should_fallback_to_chrome(&summary(50, 5, 15), 50));
    }
}
