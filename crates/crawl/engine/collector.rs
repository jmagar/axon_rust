use super::{canonicalize_url_for_dedupe, is_excluded_url_path, CrawlSummary};
use crate::crates::core::content::url_to_filename;
use crate::crates::core::logging::log_warn;
use spider_transformations::transformation::content::{transform_content_input, TransformInput};
use std::collections::HashSet;
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
