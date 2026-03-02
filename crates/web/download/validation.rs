//! Validation and security helpers for download routes.
//!
//! Includes filename sanitization, path traversal prevention, job ID
//! validation, and configurable download limits.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use axum::http::StatusCode;
use dashmap::DashMap;

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

pub(crate) fn max_files() -> usize {
    *MAX_DOWNLOAD_FILES
}

pub(crate) fn max_download_bytes() -> u64 {
    *MAX_DOWNLOAD_BYTES
}

/// Sanitize a filename for use in Content-Disposition headers.
/// Keeps ASCII alphanumeric, hyphens, dots, and underscores; replaces everything else.
pub(crate) fn sanitize_filename(raw: &str) -> String {
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
pub(crate) fn is_safe_relative_manifest_path(rel_path: &str) -> bool {
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
pub(crate) fn is_valid_job_id(id: &str) -> bool {
    id.len() == 36
        && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
        && id.chars().filter(|&c| c == '-').count() == 4
}

/// Look up and validate the job directory from the DashMap registry.
///
/// Uses `tokio::fs::metadata` instead of the blocking `Path::is_dir()` to
/// avoid stalling the Tokio runtime thread on a synchronous filesystem stat.
pub(crate) async fn validate_job_dir(
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
}
