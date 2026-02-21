use crate::crates::core::config::Config;
use crate::crates::core::content::{
    canonicalize_url, extract_loc_values, extract_robots_sitemaps, is_excluded_url_path,
    to_markdown, url_to_filename,
};
use crate::crates::core::http::validate_url;
use crate::crates::crawl::engine::CrawlSummary;
use spider::tokio;
use spider::url::Url;
use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncWriteExt, BufWriter};

use super::read_manifest_urls;

#[derive(Debug, Clone, Default)]
pub(super) struct RobotsDiscoveryStats {
    pub(super) robots_declared_sitemaps: usize,
    pub(super) parsed_sitemap_documents: usize,
    pub(super) discovered_urls: usize,
    pub(super) filtered_out_of_scope_host: usize,
    pub(super) filtered_out_of_scope_path: usize,
    pub(super) filtered_excluded_prefix: usize,
    pub(super) failed_fetches: usize,
    pub(super) parse_errors: usize,
}

#[derive(Debug, Clone, Default)]
struct RobotsDiscoveryResult {
    urls: Vec<String>,
    stats: RobotsDiscoveryStats,
}

#[derive(Debug, Clone, Default)]
pub(super) struct RobotsBackfillStats {
    pub(super) discovered_urls: usize,
    pub(super) candidates: usize,
    pub(super) written: usize,
    pub(super) failed: usize,
    pub(super) filtered_existing: usize,
}

async fn fetch_text_with_retry(
    client: &reqwest::Client,
    url: &str,
    retries: usize,
    backoff_ms: u64,
) -> Option<String> {
    for attempt in 0..=retries {
        let response = client.get(url).send().await;
        if let Ok(resp) = response {
            if resp.status().is_success() {
                if let Ok(text) = resp.text().await {
                    return Some(text);
                }
            }
        }
        if attempt < retries {
            let delay = backoff_ms.saturating_mul((attempt + 1) as u64);
            if delay > 0 {
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
        }
    }
    None
}

async fn discover_sitemap_urls_with_robots(
    cfg: &Config,
    start_url: &str,
) -> Result<RobotsDiscoveryResult, Box<dyn Error>> {
    let parsed = Url::parse(start_url)?;
    let scheme = parsed.scheme().to_string();
    let host = parsed.host_str().ok_or("missing host")?.to_string();
    let root_path = parsed.path().trim_end_matches('/').to_string();
    let scoped_to_root = root_path.is_empty();
    let timeout = Duration::from_millis(cfg.request_timeout_ms.unwrap_or(30_000));
    let client = reqwest::Client::builder().timeout(timeout).build()?;

    let mut queue: VecDeque<String> = VecDeque::from(vec![
        format!("{scheme}://{host}/sitemap.xml"),
        format!("{scheme}://{host}/sitemap_index.xml"),
        format!("{scheme}://{host}/sitemap-index.xml"),
    ]);
    let mut stats = RobotsDiscoveryStats::default();
    let robots_url = format!("{scheme}://{host}/robots.txt");
    if let Some(robots_txt) = fetch_text_with_retry(
        &client,
        &robots_url,
        cfg.fetch_retries,
        cfg.retry_backoff_ms,
    )
    .await
    {
        let robots_sitemaps = extract_robots_sitemaps(&robots_txt);
        stats.robots_declared_sitemaps = robots_sitemaps.len();
        for sitemap in robots_sitemaps {
            queue.push_back(sitemap);
        }
    }

    let mut seen_sitemaps = HashSet::new();
    let mut out = HashSet::new();
    let max_sitemaps = cfg.max_sitemaps.max(1);
    while let Some(candidate) = queue.pop_front() {
        if seen_sitemaps.len() >= max_sitemaps {
            break;
        }
        let Some(canonical_sitemap) = canonicalize_url(&candidate) else {
            stats.parse_errors += 1;
            continue;
        };
        if !seen_sitemaps.insert(canonical_sitemap.clone()) {
            continue;
        }
        if validate_url(&canonical_sitemap).is_err() {
            stats.failed_fetches += 1;
            continue;
        }
        let Some(xml) = fetch_text_with_retry(
            &client,
            &canonical_sitemap,
            cfg.fetch_retries,
            cfg.retry_backoff_ms,
        )
        .await
        else {
            stats.failed_fetches += 1;
            continue;
        };
        stats.parsed_sitemap_documents += 1;
        let is_index = xml.to_ascii_lowercase().contains("<sitemapindex");
        for loc in extract_loc_values(&xml) {
            let Ok(url) = Url::parse(&loc) else {
                stats.parse_errors += 1;
                continue;
            };
            let Some(url_host) = url.host_str() else {
                stats.parse_errors += 1;
                continue;
            };
            let host_ok = if cfg.include_subdomains {
                url_host == host || url_host.ends_with(&format!(".{host}"))
            } else {
                url_host == host
            };
            if !host_ok {
                stats.filtered_out_of_scope_host += 1;
                continue;
            }
            if !scoped_to_root {
                let p = url.path();
                let scoped_prefix = format!("{root_path}/");
                if p != root_path && !p.starts_with(&scoped_prefix) {
                    stats.filtered_out_of_scope_path += 1;
                    continue;
                }
            }
            if is_excluded_url_path(&loc, &cfg.exclude_path_prefix) {
                stats.filtered_excluded_prefix += 1;
                continue;
            }
            let Some(canonical_loc) = canonicalize_url(&loc) else {
                stats.parse_errors += 1;
                continue;
            };
            if is_index {
                queue.push_back(canonical_loc);
            } else {
                out.insert(canonical_loc);
            }
        }
    }
    let mut urls: Vec<String> = out.into_iter().collect();
    urls.sort();
    stats.discovered_urls = urls.len();
    Ok(RobotsDiscoveryResult { urls, stats })
}

pub(super) async fn append_robots_backfill(
    cfg: &Config,
    start_url: &str,
    output_dir: &Path,
    seen_urls: &HashSet<String>,
    summary: &mut CrawlSummary,
) -> Result<(RobotsBackfillStats, RobotsDiscoveryStats), Box<dyn Error>> {
    let discovery = discover_sitemap_urls_with_robots(cfg, start_url).await?;
    let discovery_stats = discovery.stats.clone();
    let manifest_path = output_dir.join("manifest.jsonl");
    let already_written = read_manifest_urls(&manifest_path).await?;
    let candidates: Vec<String> = discovery
        .urls
        .iter()
        .filter(|url| !seen_urls.contains(*url) && !already_written.contains(*url))
        .cloned()
        .collect();
    if candidates.is_empty() {
        return Ok((
            RobotsBackfillStats {
                discovered_urls: discovery.urls.len(),
                filtered_existing: discovery.urls.len(),
                ..RobotsBackfillStats::default()
            },
            discovery_stats,
        ));
    }

    let markdown_dir = output_dir.join("markdown");
    let timeout = Duration::from_millis(cfg.request_timeout_ms.unwrap_or(30_000));
    let client = reqwest::Client::builder().timeout(timeout).build()?;
    let mut manifest = BufWriter::new(
        tokio::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&manifest_path)
            .await?,
    );
    let mut idx = summary.markdown_files;
    let mut stats = RobotsBackfillStats {
        discovered_urls: discovery.urls.len(),
        candidates: candidates.len(),
        filtered_existing: discovery.urls.len().saturating_sub(candidates.len()),
        ..RobotsBackfillStats::default()
    };

    for url in candidates {
        let Some(html) =
            fetch_text_with_retry(&client, &url, cfg.fetch_retries, cfg.retry_backoff_ms).await
        else {
            stats.failed += 1;
            continue;
        };
        let markdown = to_markdown(&html);
        let markdown_chars = markdown.chars().count();
        if markdown_chars < cfg.min_markdown_chars {
            summary.thin_pages += 1;
        }
        if markdown_chars < cfg.min_markdown_chars && cfg.drop_thin_markdown {
            continue;
        }
        idx += 1;
        let file = markdown_dir.join(url_to_filename(&url, idx));
        tokio::fs::write(&file, markdown).await?;
        let rec = serde_json::json!({
            "url": url,
            "file_path": file.to_string_lossy(),
            "markdown_chars": markdown_chars,
            "source": "robots_sitemap_backfill"
        });
        let mut line = rec.to_string();
        line.push('\n');
        manifest.write_all(line.as_bytes()).await?;
        summary.markdown_files += 1;
        stats.written += 1;
    }
    manifest.flush().await?;
    Ok((stats, discovery_stats))
}
