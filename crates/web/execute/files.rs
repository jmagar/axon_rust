use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;

/// Resolve the output directory for reading crawl results.
/// Checks `AXON_WORKER_OUTPUT_DIR` first (host path to Docker bind mount),
/// then `AXON_OUTPUT_DIR`, then the default relative path.
pub fn output_dir() -> PathBuf {
    std::env::var("AXON_WORKER_OUTPUT_DIR")
        .or_else(|_| std::env::var("AXON_OUTPUT_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".cache/axon-rust/output"))
}

/// Find the most recently modified `.md` file in a directory.
async fn newest_md_file(dir: &Path) -> Option<PathBuf> {
    let mut entries = tokio::fs::read_dir(dir).await.ok()?;
    let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "md") {
            if let Ok(meta) = entry.metadata().await {
                if let Ok(modified) = meta.modified() {
                    if newest.as_ref().is_none_or(|(_, t)| modified > *t) {
                        newest = Some((path, modified));
                    }
                }
            }
        }
    }
    newest.map(|(p, _)| p)
}

/// Send a single scraped markdown file to the frontend.
pub(super) async fn send_scrape_file(tx: &mpsc::Sender<String>) {
    let md_dir = output_dir().join("scrape-markdown");
    match newest_md_file(&md_dir).await {
        Some(path) => match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                let _ = tx
                    .send(
                        json!({
                            "type": "file_content",
                            "path": path.to_string_lossy(),
                            "content": content,
                        })
                        .to_string(),
                    )
                    .await;
            }
            Err(e) => {
                let _ = tx
                    .send(
                        json!({"type": "log", "line": format!("[web] read error: {e}")})
                            .to_string(),
                    )
                    .await;
            }
        },
        None => {
            let _ = tx
                .send(
                    json!({"type": "log", "line": format!("[web] no .md files found in {}", md_dir.display())})
                        .to_string(),
                )
                .await;
        }
    }
}

/// Build a `screenshot_files` message from captured stdout JSON objects.
///
/// Each screenshot JSON has shape `{url, path, size_bytes}`. We extract the
/// filename from `path` and construct a serve URL so the frontend can display
/// the image inline. This is deterministic — no filesystem timestamp scan.
pub(super) async fn send_screenshot_files_from_json(
    jsons: &[serde_json::Value],
    tx: &mpsc::Sender<String>,
) {
    let mut files = Vec::new();
    for obj in jsons {
        let path_str = obj.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let size_bytes = obj.get("size_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
        let url = obj.get("url").and_then(|v| v.as_str()).unwrap_or("");
        if path_str.is_empty() {
            continue;
        }
        let name = Path::new(path_str)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        files.push(json!({
            "path": path_str,
            "name": name,
            "serve_url": format!("/output/screenshots/{name}"),
            "size_bytes": size_bytes,
            "url": url,
        }));
    }
    if !files.is_empty() {
        let _ = tx
            .send(json!({"type": "screenshot_files", "files": files}).to_string())
            .await;
    }
}

/// Send recently modified screenshot files to the frontend after a screenshot command.
/// Globs `output_dir/screenshots/*.png` and sends paths for any files modified in
/// the last 60 seconds (conservative window to catch the just-completed run).
#[allow(dead_code)]
pub(super) async fn send_screenshot_files(tx: &mpsc::Sender<String>) {
    let screenshots_dir = output_dir().join("screenshots");
    let Ok(mut entries) = tokio::fs::read_dir(&screenshots_dir).await else {
        let _ = tx
            .send(
                json!({"type": "log", "line": format!("[web] no screenshots dir at {}", screenshots_dir.display())})
                    .to_string(),
            )
            .await;
        return;
    };

    let cutoff = std::time::SystemTime::now() - Duration::from_secs(60);
    let mut files = Vec::new();

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "png") {
            if let Ok(meta) = entry.metadata().await {
                if let Ok(modified) = meta.modified() {
                    if modified >= cutoff {
                        let name = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();
                        let size_bytes = meta.len();
                        files.push(json!({
                            "path": path.to_string_lossy(),
                            "name": name,
                            "serve_url": format!("/output/screenshots/{name}"),
                            "size_bytes": size_bytes,
                        }));
                    }
                }
            }
        }
    }

    if !files.is_empty() {
        let _ = tx
            .send(json!({"type": "screenshot_files", "files": files}).to_string())
            .await;
    }
}

/// Send the crawl manifest file list to the frontend from a job output directory.
/// When `job_id` is provided, it is included in the `crawl_files` message for download routes.
pub(super) async fn send_crawl_manifest(
    job_dir: &Path,
    tx: &mpsc::Sender<String>,
    job_id: Option<&str>,
) {
    let manifest = job_dir.join("manifest.jsonl");

    // If the job-specific dir doesn't have a manifest yet, try `latest/`
    // (the reflink is updated after crawl completes, before marking done)
    let manifest = if tokio::fs::metadata(&manifest).await.is_ok() {
        manifest
    } else if let Some(parent) = job_dir.parent() {
        let latest = parent.join("latest").join("manifest.jsonl");
        if tokio::fs::metadata(&latest).await.is_ok() {
            latest
        } else {
            let _ = tx
                .send(
                    json!({"type": "log", "line": format!("[web] no manifest at {}", manifest.display())})
                        .to_string(),
                )
                .await;
            return;
        }
    } else {
        let _ = tx
            .send(json!({"type": "log", "line": "[web] no crawl manifest found"}).to_string())
            .await;
        return;
    };

    let base_dir = manifest
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    match tokio::fs::read_to_string(&manifest).await {
        Ok(raw) => {
            let mut files = Vec::new();
            for line in raw.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
                    continue;
                };
                let url = entry.get("url").and_then(|v| v.as_str()).unwrap_or("");
                let rel = entry
                    .get("relative_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let chars = entry
                    .get("markdown_chars")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if !rel.is_empty() {
                    files.push(json!({
                        "url": url,
                        "relative_path": rel,
                        "markdown_chars": chars,
                    }));
                }
            }

            let mut msg = json!({
                "type": "crawl_files",
                "files": files,
                "output_dir": base_dir.to_string_lossy(),
            });
            if let Some(id) = job_id {
                msg["job_id"] = serde_json::Value::String(id.to_string());
            }
            let _ = tx.send(msg.to_string()).await;

            // Auto-load the first file
            if let Some(first) = files.first() {
                if let Some(rel) = first.get("relative_path").and_then(|v| v.as_str()) {
                    let full = base_dir.join(rel);
                    if let Ok(content) = tokio::fs::read_to_string(&full).await {
                        let _ = tx
                            .send(
                                json!({
                                    "type": "file_content",
                                    "path": full.to_string_lossy(),
                                    "content": content,
                                })
                                .to_string(),
                            )
                            .await;
                    }
                }
            }
        }
        Err(e) => {
            let _ = tx
                .send(
                    json!({"type": "log", "line": format!("[web] manifest read error: {e}")})
                        .to_string(),
                )
                .await;
        }
    }
}

/// Read a file on demand from a crawl output directory.
/// Validates the path is within the base directory to prevent traversal attacks.
pub(crate) async fn handle_read_file(
    relative_path: &str,
    base_dir: &Path,
    tx: mpsc::Sender<String>,
) {
    let full_path = base_dir.join(relative_path);
    let Ok(canonical_base) = tokio::fs::canonicalize(base_dir).await else {
        let _ = tx
            .send(json!({"type": "error", "message": "invalid base directory"}).to_string())
            .await;
        return;
    };
    let Ok(canonical_path) = tokio::fs::canonicalize(&full_path).await else {
        let _ = tx
            .send(json!({"type": "error", "message": "file not found"}).to_string())
            .await;
        return;
    };

    if !canonical_path.starts_with(&canonical_base) {
        let _ = tx
            .send(json!({"type": "error", "message": "path outside output directory"}).to_string())
            .await;
        return;
    }

    match tokio::fs::read_to_string(&canonical_path).await {
        Ok(content) => {
            let _ = tx
                .send(
                    json!({
                        "type": "file_content",
                        "path": canonical_path.to_string_lossy(),
                        "content": content,
                    })
                    .to_string(),
                )
                .await;
        }
        Err(e) => {
            let _ = tx
                .send(json!({"type": "error", "message": format!("read error: {e}")}).to_string())
                .await;
        }
    }
}
