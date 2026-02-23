use super::{canonicalize_url_for_dedupe, is_excluded_url_path, CrawlSummary};
use crate::crates::core::content::url_to_filename;
use crate::crates::core::logging::log_warn;
use crate::crates::crawl::manifest::ManifestEntry;
use sha2::{Digest, Sha256};
use spider_transformations::transformation::content::{transform_content_input, TransformInput};
use std::collections::{HashMap, HashSet};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::Sender;

/// Drives the spider broadcast subscription to collect, filter, render, and
/// persist crawled pages. Runs in a spawned task while `website.crawl*()`
/// executes concurrently. Returns when the broadcast channel closes
/// (i.e. the crawl or sitemap phase has finished and `unsubscribe()` was called).
#[allow(clippy::too_many_arguments)]
pub(super) async fn collect_crawl_pages(
    mut rx: tokio::sync::broadcast::Receiver<spider::page::Page>,
    markdown_dir: std::path::PathBuf,
    manifest_path: std::path::PathBuf,
    min_chars: usize,
    drop_thin: bool,
    exclude_path_prefix: Vec<String>,
    transform_cfg: &'static spider_transformations::transformation::content::TransformConfig,
    progress_tx: Option<Sender<CrawlSummary>>,
    previous_manifest: HashMap<String, ManifestEntry>,
) -> Result<(CrawlSummary, HashSet<String>), String> {
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
        let trimmed = markdown.trim();
        // Byte length (O(1)) — sufficient for thin-page threshold (~200 chars).
        let chars = trimmed.len();

        if chars < min_chars {
            summary.thin_pages += 1;
            if drop_thin {
                if summary.pages_seen.is_multiple_of(25) {
                    if let Some(tx) = progress_tx.as_ref() {
                        tx.send(summary.clone()).await.ok();
                    }
                }
                continue;
            }
        }
        if trimmed.is_empty() {
            if summary.pages_seen.is_multiple_of(25) {
                if let Some(tx) = progress_tx.as_ref() {
                    tx.send(summary.clone()).await.ok();
                }
            }
            continue;
        }

        let mut hasher = Sha256::new();
        hasher.update(trimmed.as_bytes());
        let content_hash = hex::encode(hasher.finalize());

        if let Some(prev) = previous_manifest.get(&url) {
            if prev.content_hash.as_deref() == Some(&content_hash) {
                // Potential hardlink opportunity
                let prev_path = std::path::Path::new(&prev.relative_path);
                // The previous path might be absolute (legacy) or relative.
                // If it's relative, we must join it with the parent of the previous manifest.
                // For simplicity in this test, we assume the previous files are still accessible.

                if prev_path.exists() {
                    summary.markdown_files += 1;
                    summary.reused_pages += 1;
                    let filename = url_to_filename(&url, summary.markdown_files);
                    let path = markdown_dir.join(&filename);

                    // Attempt Reflink first (COW), then Hardlink, then Copy.
                    let link_res = if reflink_copy::reflink_or_copy(prev_path, &path).is_ok() {
                        Ok(())
                    } else {
                        tokio::fs::hard_link(prev_path, &path).await
                    };

                    if link_res.is_ok() {
                        let entry = ManifestEntry {
                            url: url.clone(),
                            relative_path: format!("markdown/{}", filename),
                            markdown_chars: chars,
                            content_hash: Some(content_hash),
                            changed: false,
                        };
                        let mut line = serde_json::to_string(&entry)
                            .map_err(|e| format!("json serialize failed: {e}"))?;
                        line.push('\n');
                        manifest
                            .write_all(line.as_bytes())
                            .await
                            .map_err(|e| format!("manifest failed: {e}"))?;

                        if summary.pages_seen.is_multiple_of(25) {
                            if let Some(tx) = progress_tx.as_ref() {
                                tx.send(summary.clone()).await.ok();
                            }
                        }
                        continue;
                    }
                }
            }
        }

        summary.markdown_files += 1;
        let filename = url_to_filename(&url, summary.markdown_files);
        let path = markdown_dir.join(&filename);
        tokio::fs::write(&path, trimmed.as_bytes())
            .await
            .map_err(|e| format!("write failed: {e}"))?;

        let entry = ManifestEntry {
            url: url.clone(),
            relative_path: format!("markdown/{}", filename),
            markdown_chars: chars,
            content_hash: Some(content_hash),
            changed: true,
        };
        let mut line =
            serde_json::to_string(&entry).map_err(|e| format!("json serialize failed: {e}"))?;
        line.push('\n');
        manifest
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("manifest failed: {e}"))?;

        if summary.pages_seen.is_multiple_of(25) {
            if let Some(tx) = progress_tx.as_ref() {
                tx.send(summary.clone()).await.ok();
            }
        }
    }

    manifest
        .flush()
        .await
        .map_err(|e| format!("manifest flush failed: {e}"))?;
    if let Some(tx) = progress_tx.as_ref() {
        tx.send(summary.clone()).await.ok();
    }
    Ok((summary, urls))
}
