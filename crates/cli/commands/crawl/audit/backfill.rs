use super::sitemap::discover_sitemap_urls_with_robots;
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

use crate::crates::crawl::manifest::ManifestEntry;
use sha2::{Digest, Sha256};

pub async fn append_robots_backfill(
    cfg: &Config,
    start_url: &str,
    output_dir: &Path,
    seen_urls: &HashSet<String>,
    summary: &mut crate::crates::crawl::engine::CrawlSummary,
) -> Result<RobotsBackfillStats, Box<dyn Error>> {
    let discovery = discover_sitemap_urls_with_robots(cfg, start_url).await?;
    let manifest_path = output_dir.join("manifest.jsonl");

    // Use the comprehensive manifest reader to get hashes
    let previous_manifest =
        crate::crates::crawl::manifest::read_manifest_data(&manifest_path).await?;
    let manifest_urls: HashSet<String> = previous_manifest.keys().cloned().collect();

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
        let trimmed = md.trim();
        let markdown_chars = trimmed.len();

        let mut hasher = Sha256::new();
        hasher.update(trimmed.as_bytes());
        let content_hash = hex::encode(hasher.finalize());

        if markdown_chars < cfg.min_markdown_chars {
            summary.thin_pages += 1;
        }
        if markdown_chars < cfg.min_markdown_chars && cfg.drop_thin_markdown {
            continue;
        }

        idx += 1;
        let filename = url_to_filename(&url, idx);
        let file = markdown_dir.join(&filename);
        tokio::fs::write(&file, trimmed.as_bytes()).await?;

        let entry = ManifestEntry {
            url: url.clone(),
            relative_path: format!("markdown/{}", filename),
            markdown_chars,
            content_hash: Some(content_hash),
            changed: true, // Backfill items are by definition new to this crawl session
        };
        let mut line = serde_json::to_string(&entry)?;
        line.push('\n');
        manifest.write_all(line.as_bytes()).await?;
        summary.markdown_files += 1;
        stats.written += 1;
    }
    manifest.flush().await?;
    Ok(stats)
}
