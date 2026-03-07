//! Cancel handler for async jobs via direct service dispatch.
//!
//! Previously this module spawned an `axon <mode> cancel <job_id> --json`
//! subprocess. Now it calls the jobs layer directly — no subprocess, no
//! binary discovery required.

use super::events::{self, JobCancelResponsePayload, WsEventV2, serialize_v2_event};
use super::ws_send::{send_done_dual, send_error_dual};
use crate::crates::core::config::Config;
use crate::crates::jobs;
use std::string::ToString;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn cancel_ok_from_output(
    parsed: Option<&serde_json::Value>,
    status_success: bool,
) -> bool {
    parsed
        .and_then(|v| v.get("ok"))
        .and_then(|v| v.as_bool())
        .or_else(|| {
            parsed
                .and_then(|v| v.get("canceled"))
                .and_then(|v| v.as_bool())
        })
        .unwrap_or(status_success)
}

pub(super) fn is_valid_cancel_job_id(job_id: &str) -> bool {
    Uuid::parse_str(job_id).is_ok()
}

/// Cancel a job via direct service call. No subprocess is spawned.
///
/// The `mode` argument selects which job table to cancel from
/// (`crawl`, `extract`, `embed`). Unknown or unsupported modes fall back to
/// `crawl` for backward compatibility with older browser clients that may not
/// set the mode field.
pub(super) async fn handle_cancel(
    mode: &str,
    job_id: &str,
    tx: mpsc::Sender<String>,
    cfg: Arc<Config>,
) {
    let cancel_mode = if mode.is_empty() { "crawl" } else { mode };

    // Validate mode against the allowlist before doing any work.
    if !super::constants::ALLOWED_MODES.contains(&cancel_mode) {
        let ws_ctx = events::CommandContext {
            exec_id: format!("exec-{}", Uuid::new_v4()),
            mode: cancel_mode.to_string(),
            input: job_id.to_string(),
        };
        send_error_dual(
            &tx,
            &ws_ctx,
            format!("cancel failed: unknown mode '{cancel_mode}'"),
            None,
        )
        .await;
        return;
    }

    let ws_ctx = events::CommandContext {
        exec_id: format!("exec-{}", Uuid::new_v4()),
        mode: cancel_mode.to_string(),
        input: job_id.to_string(),
    };

    // Validate job_id is a UUID before hitting the DB.
    let uuid = match Uuid::parse_str(job_id) {
        Ok(u) => u,
        Err(_) => {
            if let Some(v2) = serialize_v2_event(WsEventV2::JobCancelResponse {
                ctx: ws_ctx.clone(),
                payload: JobCancelResponsePayload {
                    ok: false,
                    mode: Some(cancel_mode.to_string()),
                    job_id: Some(job_id.to_string()),
                    message: Some("invalid job_id format".to_string()),
                },
            }) {
                let _ = tx.send(v2).await;
            }
            send_error_dual(
                &tx,
                &ws_ctx,
                "cancel failed: invalid job_id format".to_string(),
                None,
            )
            .await;
            return;
        }
    };

    // Dispatch cancel to the appropriate job table.
    // Map the error to String immediately so no non-Send Box<dyn Error> is held
    // across the subsequent .await points in the match arms below.
    let cancel_result: Result<bool, String> = match cancel_mode {
        "crawl" => jobs::crawl::cancel_job(&cfg, uuid)
            .await
            .map_err(|e| e.to_string()),
        "extract" => jobs::extract::cancel_extract_job(&cfg, uuid)
            .await
            .map_err(|e| e.to_string()),
        "embed" => jobs::embed::cancel_embed_job(&cfg, uuid)
            .await
            .map_err(|e| e.to_string()),
        other => Err(format!("cancel not supported for mode '{other}'")),
    };

    match cancel_result {
        Ok(ok) => {
            let message = if ok {
                Some("cancel requested".to_string())
            } else {
                Some("job not found or already terminal".to_string())
            };

            if let Some(v2) = serialize_v2_event(WsEventV2::JobCancelResponse {
                ctx: ws_ctx.clone(),
                payload: JobCancelResponsePayload {
                    ok,
                    mode: Some(cancel_mode.to_string()),
                    job_id: Some(job_id.to_string()),
                    message,
                },
            }) {
                let _ = tx.send(v2).await;
            }

            if ok {
                send_done_dual(&tx, &ws_ctx, 0, None).await;
            } else {
                send_error_dual(
                    &tx,
                    &ws_ctx,
                    "cancel failed: job not found or already terminal".to_string(),
                    None,
                )
                .await;
            }
        }
        Err(err_msg) => {
            if let Some(v2) = serialize_v2_event(WsEventV2::JobCancelResponse {
                ctx: ws_ctx.clone(),
                payload: JobCancelResponsePayload {
                    ok: false,
                    mode: Some(cancel_mode.to_string()),
                    job_id: Some(job_id.to_string()),
                    message: Some(err_msg.clone()),
                },
            }) {
                let _ = tx.send(v2).await;
            }
            send_error_dual(&tx, &ws_ctx, format!("cancel failed: {err_msg}"), None).await;
        }
    }
}
