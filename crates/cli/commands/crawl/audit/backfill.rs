use super::sitemap::discover_sitemap_urls_with_robots;
use crate::crates::cli::commands::crawl::manifest::read_manifest_urls;
use crate::crates::core::config::Config;
use crate::crates::core::content::{to_markdown, url_to_filename};
use crate::crates::core::http::validate_url;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::error::Error;
use std::path::Path;
use tokio::io::{AsyncWriteExt, BufWriter};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RobotsBackfillStats {
    pub discovered_urls: usize,
    pub candidates: usize,
    pub fetched_ok: usize,
    pub written: usize,
    pub failed: usize,
}

pub async fn append_robots_backfill(
    cfg: &Config,
    start_url: &str,
    output_dir: &Path,
    seen_urls: &HashSet<String>,
    summary: &mut crate::crates::crawl::engine::CrawlSummary,
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
    tokio::fs::create_dir_all(&markdown_dir).await?;
    let client = crate::crates::core::http::http_client()?;
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
            super::fetch_text_with_retry(client, &url, cfg.fetch_retries, cfg.retry_backoff_ms)
                .await
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
