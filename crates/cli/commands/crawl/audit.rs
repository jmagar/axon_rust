mod audit_diff;

use super::manifest::read_manifest_urls;
use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::content::{
    canonicalize_url, extract_loc_values, extract_robots_sitemaps, is_excluded_url_path,
    to_markdown, url_to_filename,
};
use crate::axon_cli::crates::core::http::validate_url;
use crate::axon_cli::crates::core::ui::{muted, primary};
use serde::{Deserialize, Serialize};
use spider::url::Url;
use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SitemapDiscoveryStats {
    pub robots_declared_sitemaps: usize,
    pub seeded_default_sitemaps: usize,
    pub discovered_sitemap_documents: usize,
    pub parsed_sitemap_documents: usize,
    pub discovered_urls: usize,
    pub filtered_out_of_scope_host: usize,
    pub filtered_out_of_scope_path: usize,
    pub filtered_excluded_prefix: usize,
    pub failed_fetches: usize,
    pub parse_errors: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SitemapDiscoveryResult {
    pub urls: Vec<String>,
    pub stats: SitemapDiscoveryStats,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct RobotsBackfillStats {
    pub discovered_urls: usize,
    pub candidates: usize,
    pub fetched_ok: usize,
    pub written: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestAuditEntry {
    url: String,
    file_path: String,
    markdown_chars: usize,
    fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct CrawlAuditSnapshot {
    generated_at_epoch_ms: u128,
    start_url: String,
    output_dir: String,
    exclude_path_prefix: Vec<String>,
    sitemap: SitemapDiscoveryStats,
    discovered_url_count: usize,
    manifest_entry_count: usize,
    manifest_entries: Vec<ManifestAuditEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct CrawlAuditSnapshotDiff {
    generated_at_epoch_ms: u128,
    previous_report: String,
    current_report: String,
    discovered_added: usize,
    discovered_removed: usize,
    manifest_added: usize,
    manifest_removed: usize,
    manifest_changed: usize,
}

pub(super) fn now_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
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
    for attempt in 0..=retries {
        let response = client.get(url).send().await;
        if let Ok(resp) = response {
            if resp.status().is_success() {
                if let Ok(text) = resp.text().await {
                    return Some(text);
                }
            } else if resp.status().is_client_error()
                && resp.status() != reqwest::StatusCode::TOO_MANY_REQUESTS
            {
                return None;
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

fn default_sitemap_queue(scheme: &str, host: &str) -> VecDeque<String> {
    VecDeque::from(vec![
        format!("{scheme}://{host}/sitemap.xml"),
        format!("{scheme}://{host}/sitemap_index.xml"),
        format!("{scheme}://{host}/sitemap-index.xml"),
    ])
}

async fn enqueue_robots_sitemaps(
    cfg: &Config,
    client: &reqwest::Client,
    scheme: &str,
    host: &str,
    queue: &mut VecDeque<String>,
    stats: &mut SitemapDiscoveryStats,
) {
    let robots_url = format!("{scheme}://{host}/robots.txt");
    if validate_url(&robots_url).is_err() {
        stats.failed_fetches += 1;
        return;
    }
    if let Some(robots_txt) =
        fetch_text_with_retry(client, &robots_url, cfg.fetch_retries, cfg.retry_backoff_ms).await
    {
        let robots_sitemaps = extract_robots_sitemaps(&robots_txt);
        stats.robots_declared_sitemaps = robots_sitemaps.len();
        for sitemap in robots_sitemaps {
            queue.push_back(sitemap);
        }
    }
}

fn in_host_scope(url_host: &str, host: &str, include_subdomains: bool, host_suffix: &str) -> bool {
    if include_subdomains {
        url_host == host || url_host.ends_with(host_suffix)
    } else {
        url_host == host
    }
}

fn in_path_scope(path: &str, root_path: &str, scoped_to_root: bool) -> bool {
    if scoped_to_root {
        return true;
    }
    let scoped_prefix = format!("{root_path}/");
    path == root_path || path.starts_with(&scoped_prefix)
}

struct SitemapScope<'a> {
    host: &'a str,
    host_suffix: String,
    include_subdomains: bool,
    root_path: &'a str,
    scoped_to_root: bool,
}

fn canonical_sitemap_loc(
    cfg: &Config,
    loc: &str,
    scope: &SitemapScope<'_>,
    stats: &mut SitemapDiscoveryStats,
) -> Option<String> {
    let Ok(url) = Url::parse(loc) else {
        stats.parse_errors += 1;
        return None;
    };
    let Some(url_host) = url.host_str() else {
        stats.parse_errors += 1;
        return None;
    };
    if !in_host_scope(
        url_host,
        scope.host,
        scope.include_subdomains,
        &scope.host_suffix,
    ) {
        stats.filtered_out_of_scope_host += 1;
        return None;
    }
    if !in_path_scope(url.path(), scope.root_path, scope.scoped_to_root) {
        stats.filtered_out_of_scope_path += 1;
        return None;
    }
    if is_excluded_url_path(loc, &cfg.exclude_path_prefix) {
        stats.filtered_excluded_prefix += 1;
        return None;
    }
    let Some(canonical_loc) = canonicalize_url(loc) else {
        stats.parse_errors += 1;
        return None;
    };
    Some(canonical_loc)
}

pub(crate) async fn discover_sitemap_urls_with_robots(
    cfg: &Config,
    start_url: &str,
) -> Result<SitemapDiscoveryResult, Box<dyn Error>> {
    let parsed = Url::parse(start_url)?;
    let scheme = parsed.scheme().to_string();
    let host = parsed.host_str().ok_or("missing host")?.to_string();
    let root_path = parsed.path().trim_end_matches('/').to_string();
    let scoped_to_root = root_path.is_empty();
    let host_suffix = format!(".{host}");
    let timeout = Duration::from_millis(cfg.request_timeout_ms.unwrap_or(30_000));
    let client = reqwest::Client::builder().timeout(timeout).build()?;

    let mut stats = SitemapDiscoveryStats {
        seeded_default_sitemaps: 3,
        ..Default::default()
    };
    let mut queue = default_sitemap_queue(&scheme, &host);
    enqueue_robots_sitemaps(cfg, &client, &scheme, &host, &mut queue, &mut stats).await;

    let scope = SitemapScope {
        host: &host,
        host_suffix,
        include_subdomains: cfg.include_subdomains,
        root_path: &root_path,
        scoped_to_root,
    };
    let mut seen_sitemaps = HashSet::new();
    let mut urls = HashSet::new();
    let max_sitemaps = cfg.max_sitemaps.max(1);
    while let Some(next_sitemap) = queue.pop_front() {
        if seen_sitemaps.len() >= max_sitemaps {
            break;
        }
        let Some(canonical_sitemap) = canonicalize_url(&next_sitemap) else {
            stats.parse_errors += 1;
            continue;
        };
        if !seen_sitemaps.insert(canonical_sitemap.clone()) {
            continue;
        }
        stats.discovered_sitemap_documents = seen_sitemaps.len();
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
            if let Some(canonical_loc) = canonical_sitemap_loc(cfg, &loc, &scope, &mut stats) {
                if is_index {
                    queue.push_back(canonical_loc);
                } else {
                    urls.insert(canonical_loc);
                }
            }
        }
    }

    let mut discovered_urls: Vec<String> = urls.into_iter().collect();
    discovered_urls.sort();
    stats.discovered_urls = discovered_urls.len();
    Ok(SitemapDiscoveryResult {
        urls: discovered_urls,
        stats,
    })
}

pub(super) async fn append_robots_backfill(
    cfg: &Config,
    start_url: &str,
    output_dir: &Path,
    seen_urls: &HashSet<String>,
    summary: &mut crate::axon_cli::crates::crawl::engine::CrawlSummary,
) -> Result<RobotsBackfillStats, Box<dyn Error>> {
    let discovery = discover_sitemap_urls_with_robots(cfg, start_url).await?;
    let manifest_path = output_dir.join("manifest.jsonl");
    let manifest_urls = read_manifest_urls(&manifest_path).await?;
    let candidates: Vec<String> = discovery
        .urls
        .iter()
        .filter(|url| !seen_urls.contains(*url) && !manifest_urls.contains(*url))
        .cloned()
        .collect();
    if candidates.is_empty() {
        return Ok(RobotsBackfillStats {
            discovered_urls: discovery.urls.len(),
            ..RobotsBackfillStats::default()
        });
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
        ..RobotsBackfillStats::default()
    };

    for url in candidates {
        if validate_url(&url).is_err() {
            stats.failed += 1;
            continue;
        }
        let Some(html) =
            fetch_text_with_retry(&client, &url, cfg.fetch_retries, cfg.retry_backoff_ms).await
        else {
            stats.failed += 1;
            continue;
        };
        stats.fetched_ok += 1;
        let md = to_markdown(&html);
        let markdown_chars = md.trim().len();
        if markdown_chars < cfg.min_markdown_chars {
            summary.thin_pages += 1;
        }
        if markdown_chars < cfg.min_markdown_chars && cfg.drop_thin_markdown {
            continue;
        }

        idx += 1;
        let file = markdown_dir.join(url_to_filename(&url, idx));
        tokio::fs::write(&file, md).await?;
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
    Ok(stats)
}

async fn read_manifest_entries(
    output_dir: &Path,
) -> Result<Vec<ManifestAuditEntry>, Box<dyn Error>> {
    let manifest_path = output_dir.join("manifest.jsonl");
    if !tokio::fs::try_exists(&manifest_path).await? {
        return Ok(Vec::new());
    }
    let file = tokio::fs::File::open(&manifest_path).await?;
    let mut reader = BufReader::new(file).lines();
    let mut entries = Vec::new();
    while let Some(line) = reader.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        let Some(url) = json.get("url").and_then(|v| v.as_str()) else {
            continue;
        };
        let file_path = json
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let markdown_chars = json
            .get("markdown_chars")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let fingerprint = if file_path.is_empty() {
            "no-file-path".to_string()
        } else {
            match tokio::fs::read(&file_path).await {
                Ok(bytes) => fnv1a64_hex(&bytes),
                Err(_) => "file-not-found".to_string(),
            }
        };
        entries.push(ManifestAuditEntry {
            url: url.to_string(),
            file_path,
            markdown_chars,
            fingerprint,
        });
    }
    Ok(entries)
}

fn fnv1a64_hex(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

async fn persist_audit_snapshot(
    cfg: &Config,
    start_url: &str,
) -> Result<(PathBuf, CrawlAuditSnapshot), Box<dyn Error>> {
    let now = now_epoch_ms();
    let discovery = discover_sitemap_urls_with_robots(cfg, start_url).await?;
    let manifest_entries = read_manifest_entries(&cfg.output_dir).await?;
    let snapshot = CrawlAuditSnapshot {
        generated_at_epoch_ms: now,
        start_url: start_url.to_string(),
        output_dir: cfg.output_dir.to_string_lossy().to_string(),
        exclude_path_prefix: cfg.exclude_path_prefix.clone(),
        sitemap: discovery.stats,
        discovered_url_count: discovery.urls.len(),
        manifest_entry_count: manifest_entries.len(),
        manifest_entries,
    };
    let audit_dir = cfg.output_dir.join("reports").join("crawl-audit");
    tokio::fs::create_dir_all(&audit_dir).await?;
    let path = audit_dir.join(format!("audit-{now}.json"));
    tokio::fs::write(&path, serde_json::to_string_pretty(&snapshot)?).await?;
    Ok((path, snapshot))
}

pub(super) async fn run_crawl_audit(cfg: &Config, start_url: &str) -> Result<(), Box<dyn Error>> {
    validate_url(start_url)?;
    let (path, snapshot) = persist_audit_snapshot(cfg, start_url).await?;
    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "audit_report_path": path,
                "snapshot": snapshot,
            }))?
        );
    } else {
        println!("{}", primary("Crawl Audit"));
        println!("  {} {}", muted("Report:"), path.to_string_lossy());
        println!(
            "  {} {}",
            muted("Discovered URLs:"),
            snapshot.discovered_url_count
        );
        println!(
            "  {} {}",
            muted("Manifest entries:"),
            snapshot.manifest_entry_count
        );
    }
    Ok(())
}

pub(super) async fn run_crawl_audit_diff(cfg: &Config) -> Result<(), Box<dyn Error>> {
    audit_diff::run_crawl_audit_diff(cfg).await
}
