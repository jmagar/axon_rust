use crate::crates::core::config::Config;
use crate::crates::core::content::{
    canonicalize_url, extract_loc_values, extract_robots_sitemaps, is_excluded_url_path,
};
use crate::crates::core::http::validate_url;
use serde::{Deserialize, Serialize};
use spider::url::Url;
use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::time::Duration;

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
        super::fetch_text_with_retry(client, &robots_url, cfg.fetch_retries, cfg.retry_backoff_ms)
            .await
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

fn in_path_scope(path: &str, root_path: &str, scoped_to_root: bool, scoped_prefix: &str) -> bool {
    if scoped_to_root {
        return true;
    }
    path == root_path || path.starts_with(scoped_prefix)
}

struct SitemapScope<'a> {
    host: &'a str,
    host_suffix: String,
    include_subdomains: bool,
    root_path: &'a str,
    scoped_to_root: bool,
    scoped_prefix: String,
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
    if !in_path_scope(
        url.path(),
        scope.root_path,
        scope.scoped_to_root,
        &scope.scoped_prefix,
    ) {
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

    let mut queue = default_sitemap_queue(&scheme, &host);
    let mut stats = SitemapDiscoveryStats {
        seeded_default_sitemaps: queue.len(),
        ..Default::default()
    };
    enqueue_robots_sitemaps(cfg, &client, &scheme, &host, &mut queue, &mut stats).await;

    let scope = SitemapScope {
        host: &host,
        host_suffix,
        include_subdomains: cfg.include_subdomains,
        scoped_prefix: format!("{root_path}/"),
        root_path: &root_path,
        scoped_to_root,
    };
    let mut seen_sitemaps = HashSet::new();
    let mut urls = HashSet::new();
    let max_sitemaps = if cfg.max_pages > 0 {
        (cfg.max_pages as usize).max(64)
    } else {
        512
    };
    let max_urls: usize = if cfg.max_pages > 0 {
        cfg.max_pages as usize
    } else {
        100_000
    };
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
        let Some(xml) = super::fetch_text_with_retry(
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
            if !is_index && urls.len() >= max_urls {
                break;
            }
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
