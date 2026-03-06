//! Command execution bridge for `axon serve`.
//! Validates frontend requests, calls services directly for sync modes, enqueues
//! async jobs with fire-and-forget semantics, and streams output over WebSocket.
//!
//! # Execution paths
//! - **Async modes** (`crawl`, `extract`, `embed`, `github`, `reddit`, `youtube`):
//!   `async_mode::handle_async_command` — direct service enqueue, fire-and-forget.
//! - **Sync direct modes** (scrape, map, query, retrieve, ask, search, research,
//!   stats, sources, domains, doctor, status, pulse_chat):
//!   `sync_mode::handle_sync_direct` — direct service call, awaited inline.
//! - **Sync subprocess fallback** (suggest, screenshot, evaluate, sessions,
//!   dedupe, debug, refresh): spawns the `axon` binary until direct dispatch is wired.
mod args;
mod async_mode;
mod cancel;
pub(crate) mod constants;
mod context;
pub(crate) mod events;
mod exe;
pub(crate) mod files;
pub mod overrides;
mod sync_mode;
mod ws_send;

#[cfg(test)]
#[path = "execute/tests/ws_event_v2_tests.rs"]
mod ws_event_v2_tests;

#[cfg(test)]
#[path = "execute/tests/ws_protocol_tests.rs"]
mod ws_protocol_tests;

pub(crate) use files::handle_read_file;

#[cfg(test)]
fn build_args(mode: &str, input: &str, flags: &serde_json::Value) -> Vec<String> {
    args::build_args(mode, input, flags)
}

#[cfg(test)]
fn strip_ansi(s: &str) -> String {
    exe::strip_ansi(s)
}

#[cfg(test)]
fn allowed_modes() -> &'static [&'static str] {
    ALLOWED_MODES
}

#[cfg(test)]
fn allowed_flags() -> &'static [(&'static str, &'static str)] {
    ALLOWED_FLAGS
}

#[cfg(test)]
fn direct_sync_modes() -> &'static [&'static str] {
    sync_mode::DIRECT_SYNC_MODES
}

#[cfg(test)]
fn async_modes() -> &'static [&'static str] {
    ASYNC_MODES
}

// Public re-exports for integration tests in tests/web_ws_async_fire_and_forget.rs.
// These forward to the same internal constants/functions but are exposed via the
// public `execute` module path so integration tests can import them without
// reaching into private submodule internals.
pub fn async_modes_pub() -> &'static [&'static str] {
    ASYNC_MODES
}

pub fn direct_sync_modes_pub() -> &'static [&'static str] {
    sync_mode::DIRECT_SYNC_MODES
}

pub fn allowed_modes_pub() -> &'static [&'static str] {
    ALLOWED_MODES
}

pub fn is_valid_cancel_job_id_pub(job_id: &str) -> bool {
    cancel::is_valid_cancel_job_id(job_id)
}

use crate::crates::core::config::Config;
use std::sync::Arc;
use std::time::Instant;
use tokio::process::Command;
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;

#[cfg(test)]
use constants::ALLOWED_FLAGS;
use constants::{ALLOWED_MODES, ASYNC_MODES, ASYNC_SUBPROCESS_MODES};
use context::ExecCommandContext;

fn resolve_exe() -> Result<std::path::PathBuf, String> {
    exe::resolve_exe()
}

#[cfg(test)]
fn cancel_ok_from_output(parsed: Option<&serde_json::Value>, status_success: bool) -> bool {
    cancel::cancel_ok_from_output(parsed, status_success)
}

#[cfg(test)]
fn is_valid_cancel_job_id(job_id: &str) -> bool {
    cancel::is_valid_cancel_job_id(job_id)
}

#[cfg(test)]
async fn send_command_output_line(
    tx: &mpsc::Sender<String>,
    context: &events::CommandContext,
    line: String,
) {
    ws_send::send_command_output_line(tx, context, line).await
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) async fn send_done_dual(
    tx: &mpsc::Sender<String>,
    context: &events::CommandContext,
    exit_code: i32,
    elapsed_ms: Option<u64>,
) {
    ws_send::send_done_dual(tx, context, exit_code, elapsed_ms).await
}

pub(super) async fn send_error_dual(
    tx: &mpsc::Sender<String>,
    context: &events::CommandContext,
    message: String,
    elapsed_ms: Option<u64>,
) {
    ws_send::send_error_dual(tx, context, message, elapsed_ms).await
}

async fn handle_sync_command(
    child: tokio::process::Child,
    context: &ExecCommandContext,
    tx: &mpsc::Sender<String>,
    start: Instant,
) {
    sync_mode::handle_sync_command(child, context, tx, start).await
}

pub(super) async fn handle_cancel(
    mode: &str,
    job_id: &str,
    tx: mpsc::Sender<String>,
    cfg: Arc<Config>,
) {
    cancel::handle_cancel(mode, job_id, tx, cfg).await
}

pub(super) async fn handle_command(
    mode: String,
    input: String,
    flags: serde_json::Value,
    tx: mpsc::Sender<String>,
    crawl_job_id: Arc<Mutex<Option<String>>>,
    cfg: Arc<Config>,
) {
    // All mode/input/flags checks below use `.as_str()` / `.as_ref()` on the
    // owned values.  These borrows live only within their enclosing expressions
    // and never cross an `.await` point, so the future remains `Send + 'static`.
    let context = ExecCommandContext {
        exec_id: format!("exec-{}", Uuid::new_v4()),
        mode: mode.clone(),
        input: input.clone(),
        flags: flags.clone(),
        cfg: cfg.clone(),
    };
    let ws_ctx = context.to_ws_ctx();

    if !ALLOWED_MODES.contains(&mode.as_str()) {
        send_error_dual(&tx, &ws_ctx, format!("unknown mode: {mode}"), None).await;
        return;
    }

    // Async modes (crawl, extract, embed) — fire-and-forget direct service dispatch:
    // enqueue the job and return immediately with the job ID.
    // No subprocess is spawned; no polling loop is run.
    if ASYNC_MODES.contains(&mode.as_str()) {
        ws_send::send_command_start(&tx, &context).await;
        async_mode::handle_async_command(context, tx, crawl_job_id).await;
        return;
    }

    // Sync direct modes (scrape, map, query, retrieve, ask, search, research,
    // stats, sources, domains, doctor, status, pulse_chat) — call services directly.
    if let Some(params) = sync_mode::classify_sync_direct(&mode, &input, &flags, cfg, &ws_ctx) {
        ws_send::send_command_start(&tx, &context).await;
        sync_mode::handle_sync_direct(params, tx, ws_ctx).await;
        return;
    }

    ws_send::send_command_start(&tx, &context).await;

    // Subprocess fallback.  Covers:
    // - github, reddit, youtube (ingest — !Send service functions, run to completion)
    // - suggest, screenshot, evaluate, sessions, dedupe, debug, refresh (not yet direct)
    // TODO: direct dispatch for remaining modes once !Send constraints are resolved
    let _ = ASYNC_SUBPROCESS_MODES; // used in routing comment above; suppress dead_code lint
    let exe = match resolve_exe() {
        Ok(p) => p,
        Err(e) => {
            send_error_dual(&tx, &ws_ctx, format!("cannot find axon binary: {e}"), None).await;
            return;
        }
    };
    let args = args::build_args(&mode, &input, &flags);
    let start = Instant::now();
    let child = Command::new(&exe)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    let child = match child {
        Ok(c) => c,
        Err(e) => {
            send_error_dual(
                &tx,
                &ws_ctx,
                format!("spawn failed: {e} (exe: {})", exe.display()),
                None,
            )
            .await;
            return;
        }
    };
    handle_sync_command(child, &context, &tx, start).await;
}
