//! Command execution bridge for `axon serve`.
//! Validates frontend requests, runs `axon` subprocesses, and streams output.
mod args;
mod async_mode;
mod cancel;
mod constants;
mod context;
pub(crate) mod events;
mod exe;
pub(crate) mod files;
mod polling;
mod sync_mode;
mod ws_send;

#[cfg(test)]
#[path = "tests/ws_event_v2_tests.rs"]
mod ws_event_v2_tests;

#[cfg(test)]
#[path = "tests/ws_protocol_tests.rs"]
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
    mode: &str,
    input: &str,
    flags: &serde_json::Value,
    tx: mpsc::Sender<String>,
    crawl_job_id: Arc<Mutex<Option<String>>>,
) {
    let context = ExecCommandContext {
        exec_id: format!("exec-{}", Uuid::new_v4()),
        mode: mode.to_string(),
        input: input.to_string(),
    };
    let ws_ctx = context.to_ws_ctx();

    if !ALLOWED_MODES.contains(&mode) {
        send_error_dual(&tx, &ws_ctx, format!("unknown mode: {mode}"), None).await;
        return;
    }

    let exe = match resolve_exe() {
        Ok(p) => p,
        Err(e) => {
            send_error_dual(&tx, &ws_ctx, format!("cannot find axon binary: {e}"), None).await;
            return;
        }
    };

    let args = args::build_args(&context.mode, &context.input, flags);
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

    if ASYNC_MODES.contains(&context.mode.as_str()) {
        async_mode::handle_async_command(child, context, &tx, crawl_job_id, start).await;
    } else {
        handle_sync_command(child, &context, &tx, start).await;
    }
}
