//! Command execution bridge for `axon serve`.
//! Validates frontend requests, runs `axon` subprocesses or calls services
//! directly, and streams output over WebSocket.
mod args;
mod async_mode;
mod cancel;
pub(crate) mod constants;
mod context;
pub(crate) mod events;
mod exe;
pub(crate) mod files;
pub mod overrides;
mod polling;
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

use crate::crates::core::config::Config;
use std::sync::Arc;
use std::time::Instant;
use tokio::process::Command;
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;

#[cfg(test)]
use constants::ALLOWED_FLAGS;
use constants::{ALLOWED_MODES, ASYNC_MODES};
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

pub(super) async fn handle_cancel(mode: &str, job_id: &str, tx: mpsc::Sender<String>) {
    cancel::handle_cancel(mode, job_id, tx).await
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
        cfg: cfg.clone(),
    };
    let ws_ctx = context.to_ws_ctx();

    if !ALLOWED_MODES.contains(&mode.as_str()) {
        send_error_dual(&tx, &ws_ctx, format!("unknown mode: {mode}"), None).await;
        return;
    }

    // Async modes (crawl, extract, embed, github, reddit, youtube) always go
    // through a subprocess so the worker can manage job lifecycle.
    if ASYNC_MODES.contains(&mode.as_str()) {
        let exe = match resolve_exe() {
            Ok(p) => p,
            Err(e) => {
                send_error_dual(&tx, &ws_ctx, format!("cannot find axon binary: {e}"), None).await;
                return;
            }
        };
        let args = args::build_args(&mode, &input, &flags);
        let start = Instant::now();
        ws_send::send_command_start(&tx, &context).await;
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
        async_mode::handle_async_command(child, context, &tx, crawl_job_id, start).await;
        return;
    }

    // Sync modes: try direct service dispatch first.  Modes not yet wired to a
    // service (suggest, screenshot, evaluate, sessions, dedupe, debug, refresh)
    // fall through to the subprocess path below.
    //
    // `classify_sync_direct` is a plain (non-async) function — it performs all
    // string → enum classification and parameter extraction synchronously before
    // any `.await`.  The resulting `DirectParams` value contains only owned
    // `Send + 'static` data (enum mode, owned strings, owned Config).
    //
    // `handle_sync_direct` is then awaited directly in the current task (no
    // extra `tokio::task::spawn`).  The borrow checker's HRTB `Send` check
    // for `tokio::spawn` requires `for<'a> &'a str: Send`, which rustc cannot
    // satisfy for borrows that live inside sub-futures.  Awaiting inline avoids
    // that spawn boundary entirely; the future runs on the same Tokio worker
    // that is already executing this `handle_command` invocation.
    if let Some(params) = sync_mode::classify_sync_direct(&mode, &input, &flags, cfg, &ws_ctx) {
        ws_send::send_command_start(&tx, &context).await;
        sync_mode::handle_sync_direct(params, tx, ws_ctx).await;
        return;
    }

    ws_send::send_command_start(&tx, &context).await;

    // Subprocess fallback for sync modes not yet directly dispatched.
    // TODO: direct dispatch — suggest, screenshot, evaluate, sessions, dedupe, debug, refresh
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
