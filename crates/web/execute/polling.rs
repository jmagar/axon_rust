use crate::crates::core::content::url_to_domain;
use serde_json::json;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::mpsc;

use super::files::{output_dir, send_crawl_manifest};
use super::resolve_exe;

/// Default poll timeout in seconds (10 minutes). Override with
/// `AXON_WEB_POLL_TIMEOUT_SECS` env var.
const DEFAULT_POLL_TIMEOUT_SECS: u64 = 600;

/// Read the poll timeout from environment, falling back to the default.
fn poll_timeout() -> Duration {
    let secs = std::env::var("AXON_WEB_POLL_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_TIMEOUT_SECS);
    Duration::from_secs(secs)
}

/// Resolve the job output directory on the host filesystem.
/// The worker (possibly inside Docker) reports a relative `output_dir` like
/// `.cache/axon-rust/output/domains/example.com/{uuid}`. We extract the
/// `domains/{domain}/{uuid}` suffix and resolve it against the host-side
/// `output_dir()` so the path points to the Docker bind mount.
fn resolve_job_output_dir(metrics_dir: Option<&str>, input_url: &str, job_id: &str) -> PathBuf {
    if let Some(dir) = metrics_dir {
        // Try to extract the suffix after "domains/"
        if let Some(idx) = dir.find("domains/") {
            return output_dir().join(&dir[idx..]);
        }
        // If the path is absolute and exists, use it directly
        let p = PathBuf::from(dir);
        if p.is_absolute() {
            return p;
        }
    }
    // Fallback: compute from domain + job_id
    let domain = url_to_domain(input_url);
    output_dir().join("domains").join(&domain).join(job_id)
}

/// Send a `crawl_progress` WS message with live page counts from job metrics.
async fn send_crawl_progress(
    job_id: &str,
    status: &str,
    status_json: &serde_json::Value,
    tx: &mpsc::Sender<String>,
) {
    let m = status_json.get("metrics").cloned().unwrap_or(json!({}));
    let _ = tx
        .send(
            json!({
                "type": "crawl_progress",
                "job_id": job_id,
                "status": status,
                "pages_crawled": m.get("pages_crawled").and_then(|v| v.as_u64()).unwrap_or(0),
                "pages_discovered": m.get("pages_discovered").and_then(|v| v.as_u64()).unwrap_or(0),
                "md_created": m.get("md_created").and_then(|v| v.as_u64()).unwrap_or(0),
                "thin_md": m.get("thin_md").and_then(|v| v.as_u64()).unwrap_or(0),
                "phase": m.get("phase").and_then(|v| v.as_str()).unwrap_or("pending"),
            })
            .to_string(),
        )
        .await;
}

/// Poll an async job for completion. For crawl jobs, also sends `crawl_progress`
/// messages with live page counts and reads the manifest on completion.
///
/// Polls every 1 second up to `AXON_WEB_POLL_TIMEOUT_SECS` (default 600s).
/// After the timeout, sends an error message and stops polling.
pub(super) async fn poll_async_job(
    job_id: &str,
    mode: &str,
    input_url: &str,
    tx: &mpsc::Sender<String>,
    start: Instant,
) {
    // Status subcommand: `axon <mode> status <job_id> --json`
    let status_cmd = mode.to_string();
    let timeout = poll_timeout();
    let poll_start = Instant::now();

    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Check timeout before issuing the next status query
        if poll_start.elapsed() >= timeout {
            let elapsed = start.elapsed().as_millis() as u64;
            let _ = tx
                .send(
                    json!({
                        "type": "error",
                        "message": format!(
                            "Job poll timed out after {}s",
                            timeout.as_secs()
                        ),
                        "elapsed_ms": elapsed
                    })
                    .to_string(),
                )
                .await;
            break;
        }

        let exe = match resolve_exe() {
            Ok(e) => e,
            Err(_) => break,
        };

        let out = Command::new(&exe)
            .args([&status_cmd, "status", job_id, "--json"])
            .output()
            .await;

        let Ok(out) = out else { continue };
        let Ok(text) = String::from_utf8(out.stdout) else {
            continue;
        };
        let Ok(status_json) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };

        let status = status_json
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // For crawl mode, send progress updates with page counts
        if mode == "crawl" {
            send_crawl_progress(job_id, status, &status_json, tx).await;
        }

        match status {
            "completed" => {
                // For crawl, read manifest and send file list.
                if mode == "crawl" {
                    let result = status_json.get("metrics").cloned().unwrap_or(json!({}));
                    let job_dir = resolve_job_output_dir(
                        result.get("output_dir").and_then(|v| v.as_str()),
                        input_url,
                        job_id,
                    );
                    send_crawl_manifest(&job_dir, tx).await;
                }
                let elapsed = start.elapsed().as_millis() as u64;
                let _ = tx
                    .send(
                        json!({"type": "done", "exit_code": 0, "elapsed_ms": elapsed}).to_string(),
                    )
                    .await;
                break;
            }
            "failed" => {
                let err = status_json
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                let elapsed = start.elapsed().as_millis() as u64;
                let _ = tx
                    .send(
                        json!({"type": "error", "message": err, "elapsed_ms": elapsed}).to_string(),
                    )
                    .await;
                break;
            }
            "canceled" => {
                let elapsed = start.elapsed().as_millis() as u64;
                let _ = tx
                    .send(
                        json!({"type": "done", "exit_code": 1, "elapsed_ms": elapsed}).to_string(),
                    )
                    .await;
                break;
            }
            _ => {} // pending, running — continue polling
        }
    }
}
