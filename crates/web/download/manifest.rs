//! Manifest reading and file loading for download routes.
//!
//! Reads `manifest.jsonl` from a job directory, validates entries,
//! and loads file contents with size limits.

use std::path::Path;

use axum::http::StatusCode;
use tracing::warn;

use super::validation::{is_safe_relative_manifest_path, max_download_bytes, max_files};

/// Read the manifest.jsonl and collect (url, relative_path) pairs.
pub(crate) async fn read_manifest(
    job_dir: &Path,
) -> Result<Vec<(String, String)>, (StatusCode, &'static str)> {
    let manifest = job_dir.join("manifest.jsonl");
    let raw = tokio::fs::read_to_string(&manifest)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "manifest.jsonl not found"))?;

    let mut entries = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            warn!("download: skipping malformed manifest line: {:?}", line);
            continue;
        };
        let url = entry
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let rel = entry
            .get("relative_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if is_safe_relative_manifest_path(&rel) {
            entries.push((url, rel));
        }
    }

    if entries.len() > max_files() {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "too many files — increase AXON_DOWNLOAD_MAX_FILES",
        ));
    }

    Ok(entries)
}

/// Read all markdown file contents from a manifest, returning (url, rel_path, content).
pub(crate) async fn load_all_files(
    job_dir: &Path,
) -> Result<(String, Vec<(String, String, String)>), (StatusCode, &'static str)> {
    let manifest_entries = read_manifest(job_dir).await?;
    let canonical_base = tokio::fs::canonicalize(job_dir)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "job output directory not found"))?;
    let mut resolved_entries: Vec<(String, String, std::path::PathBuf)> = Vec::new();

    // Pre-check total file size before loading anything into memory
    let byte_limit = max_download_bytes();
    let mut total_bytes: u64 = 0;
    for (url, rel_path) in &manifest_entries {
        let file_path = canonical_base.join(rel_path);
        let Ok(canonical_file) = tokio::fs::canonicalize(&file_path).await else {
            continue;
        };
        if !canonical_file.starts_with(&canonical_base) {
            continue;
        }
        if let Ok(meta) = tokio::fs::metadata(&canonical_file).await {
            total_bytes += meta.len();
            if total_bytes > byte_limit {
                return Err((
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "total download size exceeds AXON_DOWNLOAD_MAX_BYTES limit",
                ));
            }
            resolved_entries.push((url.clone(), rel_path.clone(), canonical_file));
        }
    }

    let mut loaded = Vec::with_capacity(resolved_entries.len());
    let mut domain = String::new();

    for (url, rel_path, canonical_file) in &resolved_entries {
        if domain.is_empty() {
            if let Ok(parsed) = reqwest::Url::parse(url) {
                domain = parsed.host_str().unwrap_or("unknown").to_string();
            }
        }
        match tokio::fs::read_to_string(canonical_file).await {
            Ok(content) => loaded.push((url.clone(), rel_path.clone(), content)),
            Err(e) => {
                warn!(
                    "download: skipping unreadable file {}: {e}",
                    canonical_file.display()
                );
                continue;
            }
        }
    }

    if domain.is_empty() {
        domain = "unknown".to_string();
    }

    Ok((domain, loaded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn load_all_files_ignores_manifest_traversal_entries() {
        let temp = tempfile::tempdir().expect("tempdir");
        let job_dir = temp.path().join("job");
        std::fs::create_dir_all(job_dir.join("markdown")).expect("mkdir");
        std::fs::write(job_dir.join("markdown").join("ok.md"), "ok").expect("write");
        std::fs::write(
            job_dir.join("manifest.jsonl"),
            [
                serde_json::json!({
                    "url": "https://example.com/ok",
                    "relative_path": "markdown/ok.md"
                })
                .to_string(),
                serde_json::json!({
                    "url": "https://example.com/bad",
                    "relative_path": "../../etc/passwd"
                })
                .to_string(),
            ]
            .join("\n"),
        )
        .expect("manifest");

        let (_domain, loaded) = load_all_files(&job_dir).await.expect("load files");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].1, "markdown/ok.md");
        assert_eq!(loaded[0].2, "ok");
    }
}
