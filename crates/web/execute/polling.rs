use crate::crates::core::content::url_to_domain;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::mpsc;

use super::events::{
    CommandContext, JobProgressPayload, JobStatusPayload, WsEventV2, serialize_v2_event,
};
use super::files::{output_dir, send_crawl_manifest};
use super::{resolve_exe, send_done_dual, send_error_dual};

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
fn legacy_crawl_progress_message(
    job_id: &str,
    status: &str,
    status_json: &serde_json::Value,
) -> String {
    let m = status_json.get("metrics").cloned().unwrap_or(json!({}));
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
    .to_string()
}

fn metrics_map(status_json: &serde_json::Value) -> Option<BTreeMap<String, serde_json::Value>> {
    status_json
        .get("metrics")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<BTreeMap<_, _>>()
        })
}

fn derive_progress_payload(
    status: &str,
    status_json: &serde_json::Value,
) -> Option<JobProgressPayload> {
    let metrics = status_json.get("metrics").and_then(|v| v.as_object());
    let phase_from_metrics = metrics
        .and_then(|m| m.get("phase"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let phase = phase_from_metrics
        .clone()
        .unwrap_or_else(|| status.to_string());

    let processed = metrics
        .and_then(|m| m.get("pages_crawled").and_then(|v| v.as_u64()))
        .or_else(|| metrics.and_then(|m| m.get("processed").and_then(|v| v.as_u64())))
        .or_else(|| metrics.and_then(|m| m.get("completed").and_then(|v| v.as_u64())))
        .or_else(|| metrics.and_then(|m| m.get("current").and_then(|v| v.as_u64())));

    let total = metrics
        .and_then(|m| m.get("pages_discovered").and_then(|v| v.as_u64()))
        .or_else(|| metrics.and_then(|m| m.get("total").and_then(|v| v.as_u64())))
        .or_else(|| metrics.and_then(|m| m.get("target").and_then(|v| v.as_u64())));

    let percent = metrics
        .and_then(|m| m.get("percent").and_then(|v| v.as_f64()))
        .or_else(|| metrics.and_then(|m| m.get("progress_percent").and_then(|v| v.as_f64())))
        .or_else(|| match (processed, total) {
            (Some(done), Some(all)) if all > 0 => Some((done as f64 / all as f64) * 100.0),
            _ => None,
        });

    let has_progress_data = percent.is_some()
        || processed.is_some()
        || total.is_some()
        || phase_from_metrics.is_some()
        || !phase.is_empty();
    if !has_progress_data {
        return None;
    }

    Some(JobProgressPayload {
        phase,
        percent,
        processed,
        total,
    })
}

pub(super) fn poll_messages_for_status(
    mode: &str,
    job_id: &str,
    status: &str,
    status_json: &serde_json::Value,
    ctx: &CommandContext,
) -> Vec<String> {
    let mut messages = Vec::new();
    if mode == "crawl" {
        messages.push(legacy_crawl_progress_message(job_id, status, status_json));
    }

    if let Some(v2) = serialize_v2_event(WsEventV2::JobStatus {
        ctx: ctx.clone(),
        payload: JobStatusPayload {
            status: status.to_string(),
            error: status_json
                .get("error")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            metrics: metrics_map(status_json),
        },
    }) {
        messages.push(v2);
    }

    if let Some(payload) = derive_progress_payload(status, status_json) {
        if let Some(v2) = serialize_v2_event(WsEventV2::JobProgress {
            ctx: ctx.clone(),
            payload,
        }) {
            messages.push(v2);
        }
    }

    messages
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
    ctx: &CommandContext,
    tx: &mpsc::Sender<String>,
    start: Instant,
) {
    // Status subcommand: `axon <mode> status <job_id> --json`
    let status_mode = mode.to_string();
    let exe = match resolve_exe() {
        Ok(e) => e,
        Err(e) => {
            send_error_dual(
                tx,
                ctx,
                format!("poll aborted: cannot find axon binary: {e}"),
                Some(start.elapsed().as_millis() as u64),
            )
            .await;
            return;
        }
    };
    let timeout = poll_timeout();
    let poll_start = Instant::now();

    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Check timeout before issuing the next status query
        if poll_start.elapsed() >= timeout {
            let elapsed = start.elapsed().as_millis() as u64;
            send_error_dual(
                tx,
                ctx,
                format!("Job poll timed out after {}s", timeout.as_secs()),
                Some(elapsed),
            )
            .await;
            break;
        }

        let out = Command::new(&exe)
            .args([&status_mode, "status", job_id, "--json"])
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

        for msg in poll_messages_for_status(mode, job_id, status, &status_json, ctx) {
            let _ = tx.send(msg).await;
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
                    send_crawl_manifest(&job_dir, tx, Some(job_id), ctx).await;
                }
                let elapsed = start.elapsed().as_millis() as u64;
                send_done_dual(tx, ctx, 0, Some(elapsed)).await;
                break;
            }
            "failed" => {
                let err = status_json
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                let elapsed = start.elapsed().as_millis() as u64;
                send_error_dual(tx, ctx, err.to_string(), Some(elapsed)).await;
                break;
            }
            "canceled" => {
                let elapsed = start.elapsed().as_millis() as u64;
                send_done_dual(tx, ctx, 1, Some(elapsed)).await;
                break;
            }
            _ => {} // pending, running — continue polling
        }
    }
}
