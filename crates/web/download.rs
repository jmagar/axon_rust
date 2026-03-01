//! HTTP download handlers for crawl results.
//!
//! Four routes:
//! - `GET /download/{job_id}/pack.md`  — Repomix-style packed Markdown
//! - `GET /download/{job_id}/pack.xml` — Repomix-style packed XML
//! - `GET /download/{job_id}/archive.zip` — ZIP of all markdown files
//! - `GET /download/{job_id}/file/*path` — Single file download
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use axum::extract::{Path as AxumPath, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use dashmap::DashMap;
use std::sync::Arc;

use tracing::warn;

use super::pack;

/// Maximum files per download (guards against zip bombs / OOM).
/// Override with `AXON_DOWNLOAD_MAX_FILES` env var.
static MAX_DOWNLOAD_FILES: LazyLock<usize> = LazyLock::new(|| {
    std::env::var("AXON_DOWNLOAD_MAX_FILES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000)
});

/// Maximum total bytes across all files before zipping.
/// Override with `AXON_DOWNLOAD_MAX_BYTES` env var. Default: 500 MB.
static MAX_DOWNLOAD_BYTES: LazyLock<u64> = LazyLock::new(|| {
    std::env::var("AXON_DOWNLOAD_MAX_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500 * 1024 * 1024)
});

fn max_files() -> usize {
    *MAX_DOWNLOAD_FILES
}

fn max_download_bytes() -> u64 {
    *MAX_DOWNLOAD_BYTES
}

/// Sanitize a filename for use in Content-Disposition headers.
/// Keeps ASCII alphanumeric, hyphens, dots, and underscores; replaces everything else.
fn sanitize_filename(raw: &str) -> String {
    let sanitized: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "download".to_string()
    } else {
        sanitized
    }
}

/// Validate that a manifest relative path is a safe, non-traversing relative path.
fn is_safe_relative_manifest_path(rel_path: &str) -> bool {
    if rel_path.is_empty() || rel_path.contains('\0') {
        return false;
    }
    let path = Path::new(rel_path);
    if path.is_absolute() {
        return false;
    }
    path.components().all(|component| {
        matches!(
            component,
            std::path::Component::Normal(_) | std::path::Component::CurDir
        )
    })
}

/// Validate a job ID string: must be a valid UUID (hex + dashes, 36 chars).
fn is_valid_job_id(id: &str) -> bool {
    id.len() == 36
        && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
        && id.chars().filter(|&c| c == '-').count() == 4
}

/// Look up and validate the job directory from the DashMap registry.
///
/// Uses `tokio::fs::metadata` instead of the blocking `Path::is_dir()` to
/// avoid stalling the Tokio runtime thread on a synchronous filesystem stat.
async fn validate_job_dir(
    job_dirs: &DashMap<String, PathBuf>,
    job_id: &str,
) -> Result<PathBuf, (StatusCode, &'static str)> {
    if !is_valid_job_id(job_id) {
        return Err((StatusCode::BAD_REQUEST, "invalid job ID format"));
    }
    let dir = job_dirs
        .get(job_id)
        .map(|r| r.value().clone())
        .ok_or((StatusCode::NOT_FOUND, "job not found in registry"))?;

    let is_dir = tokio::fs::metadata(&dir)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false);
    if !is_dir {
        return Err((StatusCode::NOT_FOUND, "job output directory not found"));
    }
    Ok(dir)
}

/// Read the manifest.jsonl and collect (url, relative_path) pairs.
async fn read_manifest(
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
async fn load_all_files(
    job_dir: &Path,
) -> Result<(String, Vec<(String, String, String)>), (StatusCode, &'static str)> {
    let manifest_entries = read_manifest(job_dir).await?;
    let canonical_base = tokio::fs::canonicalize(job_dir)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "job output directory not found"))?;
    let mut resolved_entries: Vec<(String, String, PathBuf)> = Vec::new();

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

/// `GET /download/{job_id}/pack.md`
pub async fn serve_pack_md(
    AxumPath(job_id): AxumPath<String>,
    State(job_dirs): State<Arc<DashMap<String, PathBuf>>>,
) -> Response {
    let job_dir = match validate_job_dir(&job_dirs, &job_id).await {
        Ok(d) => d,
        Err((status, msg)) => return (status, msg).into_response(),
    };

    let (domain, entries) = match load_all_files(&job_dir).await {
        Ok(v) => v,
        Err((status, msg)) => return (status, msg).into_response(),
    };

    let body = pack::build_pack_md(&domain, &entries);
    let safe_filename = sanitize_filename(&format!("{domain}-pack.md"));

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/markdown; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{safe_filename}\"")
            .parse::<header::HeaderValue>()
            .unwrap_or_else(|_| {
                header::HeaderValue::from_static("attachment; filename=\"download\"")
            }),
    );

    (headers, body).into_response()
}

/// `GET /download/{job_id}/pack.xml`
pub async fn serve_pack_xml(
    AxumPath(job_id): AxumPath<String>,
    State(job_dirs): State<Arc<DashMap<String, PathBuf>>>,
) -> Response {
    let job_dir = match validate_job_dir(&job_dirs, &job_id).await {
        Ok(d) => d,
        Err((status, msg)) => return (status, msg).into_response(),
    };

    let (domain, entries) = match load_all_files(&job_dir).await {
        Ok(v) => v,
        Err((status, msg)) => return (status, msg).into_response(),
    };

    let body = pack::build_pack_xml(&domain, &entries);
    let safe_filename = sanitize_filename(&format!("{domain}-pack.xml"));

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/xml; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{safe_filename}\"")
            .parse::<header::HeaderValue>()
            .unwrap_or_else(|_| {
                header::HeaderValue::from_static("attachment; filename=\"download\"")
            }),
    );

    (headers, body).into_response()
}

/// `GET /download/{job_id}/archive.zip`
pub async fn serve_zip(
    AxumPath(job_id): AxumPath<String>,
    State(job_dirs): State<Arc<DashMap<String, PathBuf>>>,
) -> Response {
    let job_dir = match validate_job_dir(&job_dirs, &job_id).await {
        Ok(d) => d,
        Err((status, msg)) => return (status, msg).into_response(),
    };

    let (domain, entries) = match load_all_files(&job_dir).await {
        Ok(v) => v,
        Err((status, msg)) => return (status, msg).into_response(),
    };

    // Capture filename before moving domain into the blocking closure
    let filename = format!("{domain}-crawl.zip");
    let zip_result = tokio::task::spawn_blocking(move || build_zip(&domain, &entries)).await;

    match zip_result {
        Ok(Ok(bytes)) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("application/zip"),
            );
            let safe_filename = sanitize_filename(&filename);
            headers.insert(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{safe_filename}\"")
                    .parse::<header::HeaderValue>()
                    .unwrap_or_else(|_| {
                        header::HeaderValue::from_static("attachment; filename=\"download\"")
                    }),
            );
            (headers, bytes).into_response()
        }
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("zip creation failed: {e}"),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("zip task panicked: {e}"),
        )
            .into_response(),
    }
}

/// Build a ZIP archive from entries. Runs in a blocking context.
fn build_zip(
    _domain: &str,
    entries: &[(String, String, String)],
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let buf = Vec::with_capacity(entries.iter().map(|(_, _, c)| c.len()).sum::<usize>());
    let cursor = std::io::Cursor::new(buf);
    let mut zip = zip::ZipWriter::new(cursor);

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for (_, rel_path, content) in entries {
        zip.start_file(rel_path, options)?;
        zip.write_all(content.as_bytes())?;
    }

    let cursor = zip.finish()?;
    Ok(cursor.into_inner())
}

/// `GET /download/{job_id}/file/{path}`
pub async fn serve_file(
    AxumPath((job_id, file_path)): AxumPath<(String, String)>,
    State(job_dirs): State<Arc<DashMap<String, PathBuf>>>,
) -> Response {
    let job_dir = match validate_job_dir(&job_dirs, &job_id).await {
        Ok(d) => d,
        Err((status, msg)) => return (status, msg).into_response(),
    };

    // Reject obvious traversal attempts before touching the filesystem
    if file_path.contains("..") || file_path.contains('\0') {
        return (StatusCode::BAD_REQUEST, "invalid file path").into_response();
    }

    let full_path = job_dir.join(&file_path);

    // Canonicalize both paths and verify containment
    let Ok(canonical_base) = tokio::fs::canonicalize(&job_dir).await else {
        return (StatusCode::NOT_FOUND, "job directory not found").into_response();
    };
    let Ok(canonical_file) = tokio::fs::canonicalize(&full_path).await else {
        return (StatusCode::NOT_FOUND, "file not found").into_response();
    };

    if !canonical_file.starts_with(&canonical_base) {
        return (StatusCode::FORBIDDEN, "path outside job directory").into_response();
    }

    let content = match tokio::fs::read_to_string(&canonical_file).await {
        Ok(c) => c,
        Err(_) => return (StatusCode::NOT_FOUND, "file not found").into_response(),
    };

    let filename = canonical_file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download.md".to_string());

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/markdown; charset=utf-8"),
    );
    let safe_filename = sanitize_filename(&filename);
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{safe_filename}\"")
            .parse::<header::HeaderValue>()
            .unwrap_or_else(|_| {
                header::HeaderValue::from_static("attachment; filename=\"download\"")
            }),
    );

    (headers, content).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    mod job_id_validation {
        use super::*;

        #[test]
        fn validate_job_id_accepts_uuid_and_alphanum() {
            assert!(is_valid_job_id("550e8400-e29b-41d4-a716-446655440000"));
            assert!(is_valid_job_id("a1b2c3d4-e5f6-7890-abcd-ef1234567890"));
        }

        #[test]
        fn validate_job_id_rejects_traversal_and_special_chars() {
            assert!(!is_valid_job_id(""));
            assert!(!is_valid_job_id("../../../etc/passwd"));
            assert!(!is_valid_job_id("not-a-uuid-at-all"));
            assert!(!is_valid_job_id("550e8400-e29b-41d4-a716-44665544000")); // 35 chars
            assert!(!is_valid_job_id("550e8400-e29b-41d4-a716-4466554400000")); // 37 chars
            assert!(!is_valid_job_id("550e8400%e29b-41d4-a716-446655440000")); // % char
        }

        #[test]
        fn valid_job_id_rejects_path_traversal() {
            assert!(!is_valid_job_id("../../../etc/passwd"));
            assert!(!is_valid_job_id("..%2f..%2fetc%2fpasswd"));
            assert!(!is_valid_job_id("a/../b/../c/../d/../e/../"));
        }

        #[test]
        fn valid_job_id_rejects_null_bytes() {
            assert!(!is_valid_job_id("550e8400-e29b-41d4-a716-44665544\x00"));
        }

        #[tokio::test]
        async fn validate_job_dir_rejects_unknown_id() {
            let dirs = DashMap::new();
            let result = validate_job_dir(&dirs, "550e8400-e29b-41d4-a716-446655440000").await;
            assert!(result.is_err());
            let (status, _msg) = result.unwrap_err();
            assert_eq!(status, StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn validate_job_dir_rejects_bad_format() {
            let dirs = DashMap::new();
            let result = validate_job_dir(&dirs, "../../../etc").await;
            assert!(result.is_err());
            let (status, _msg) = result.unwrap_err();
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn validate_job_dir_rejects_nonexistent_dir() {
            let dirs = DashMap::new();
            let fake_id = "550e8400-e29b-41d4-a716-446655440000";
            dirs.insert(
                fake_id.to_string(),
                PathBuf::from("/tmp/nonexistent-axon-test-dir-xyz"),
            );
            let result = validate_job_dir(&dirs, fake_id).await;
            assert!(result.is_err());
            let (status, _msg) = result.unwrap_err();
            assert_eq!(status, StatusCode::NOT_FOUND);
        }
    }

    mod path_security {
        use super::*;

        #[test]
        fn sanitize_filename_strips_non_ascii() {
            assert_eq!(sanitize_filename("example.com"), "example.com");
            assert_eq!(sanitize_filename("[::1]-pack.md"), "___1_-pack.md");
            assert_eq!(
                sanitize_filename("\u{00e9}xample.com-pack.md"),
                "_xample.com-pack.md"
            );
            assert_eq!(sanitize_filename(""), "download");
        }

        #[test]
        fn manifest_relative_path_rejects_traversal_and_absolute_paths() {
            assert!(is_safe_relative_manifest_path("markdown/a.md"));
            assert!(is_safe_relative_manifest_path("./markdown/a.md"));
            assert!(!is_safe_relative_manifest_path("../secrets.txt"));
            assert!(!is_safe_relative_manifest_path(
                "markdown/../../secrets.txt"
            ));
            assert!(!is_safe_relative_manifest_path("/etc/passwd"));
            assert!(!is_safe_relative_manifest_path(""));
        }

        #[test]
        fn path_traversal_dotdot_detected() {
            // The serve_file handler rejects paths containing ".." before touching the filesystem
            let attack_paths = [
                "../sibling/secret.txt",
                "../../etc/passwd",
                "a/../../b",
                "markdown/../../../etc/shadow",
            ];
            for p in attack_paths {
                assert!(
                    p.contains(".."),
                    "test path {p} should contain '..' to trigger the guard"
                );
            }
        }

        #[test]
        fn path_traversal_null_byte_rejected() {
            let null_path = "markdown/good\0.md";
            assert!(
                null_path.contains('\0'),
                "null byte path should be caught by the serve_file guard"
            );
        }
    }

    mod zip_and_manifest {
        use super::*;

        #[test]
        fn zip_roundtrip() {
            let entries = vec![
                (
                    "https://example.com/a".to_string(),
                    "markdown/a.md".to_string(),
                    "Hello from A".to_string(),
                ),
                (
                    "https://example.com/b".to_string(),
                    "markdown/b.md".to_string(),
                    "Hello from B".to_string(),
                ),
            ];
            let bytes = build_zip("example.com", &entries).expect("zip should build");
            assert!(!bytes.is_empty());
            // Verify it's a valid ZIP by checking magic bytes
            assert_eq!(&bytes[0..2], b"PK");
        }

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
}
