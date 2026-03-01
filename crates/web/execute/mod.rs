//! Command execution bridge for `axon serve`.
//!
//! This module validates frontend requests, launches `axon` subprocesses,
//! streams command output to WebSocket clients, and orchestrates async job
//! polling/cancel flows.
pub(crate) mod events;
pub(crate) mod files;
mod polling;
#[cfg(test)]
#[path = "tests/ws_event_v2_tests.rs"]
mod ws_event_v2_tests;

pub(crate) use files::handle_read_file;

use serde_json::json;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;

use events::{
    CommandDonePayload, CommandErrorPayload, JobCancelResponsePayload, WsEventV2,
    serialize_v2_event,
};
use files::send_scrape_file;
use polling::poll_async_job;

#[derive(Debug, Clone)]
struct ExecCommandContext {
    exec_id: String,
    mode: String,
    input: String,
}

impl ExecCommandContext {
    fn to_ws_ctx(&self) -> events::CommandContext {
        events::CommandContext {
            exec_id: self.exec_id.clone(),
            mode: self.mode.clone(),
            input: self.input.clone(),
        }
    }
}

fn cancel_ok_from_output(parsed: Option<&serde_json::Value>, status_success: bool) -> bool {
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

fn is_valid_cancel_job_id(job_id: &str) -> bool {
    Uuid::parse_str(job_id).is_ok()
}

async fn send_command_start(tx: &mpsc::Sender<String>, context: &ExecCommandContext) {
    if let Some(v2) = serialize_v2_event(WsEventV2::CommandStart {
        ctx: context.to_ws_ctx(),
    }) {
        let _ = tx.send(v2).await;
    }
}

async fn send_command_output_json(
    tx: &mpsc::Sender<String>,
    context: &events::CommandContext,
    data: serde_json::Value,
) {
    if let Some(v2) = serialize_v2_event(WsEventV2::CommandOutputJson {
        ctx: context.clone(),
        data,
    }) {
        let _ = tx.send(v2).await;
    }
}

async fn send_command_output_line(
    tx: &mpsc::Sender<String>,
    context: &events::CommandContext,
    line: String,
) {
    if let Some(v2) = serialize_v2_event(WsEventV2::CommandOutputLine {
        ctx: context.clone(),
        line,
    }) {
        let _ = tx.send(v2).await;
    }
}

pub(super) async fn send_done_dual(
    tx: &mpsc::Sender<String>,
    context: &events::CommandContext,
    exit_code: i32,
    elapsed_ms: Option<u64>,
) {
    if let Some(v2) = serialize_v2_event(WsEventV2::CommandDone {
        ctx: context.clone(),
        payload: CommandDonePayload {
            exit_code,
            elapsed_ms,
        },
    }) {
        let _ = tx.send(v2).await;
    }
}

pub(super) async fn send_error_dual(
    tx: &mpsc::Sender<String>,
    context: &events::CommandContext,
    message: String,
    elapsed_ms: Option<u64>,
) {
    if let Some(v2) = serialize_v2_event(WsEventV2::CommandError {
        ctx: context.clone(),
        payload: CommandErrorPayload {
            message,
            elapsed_ms,
        },
    }) {
        let _ = tx.send(v2).await;
    }
}

/// Known command modes — only these are allowed to prevent injection.
const ALLOWED_MODES: &[&str] = &[
    "scrape",
    "crawl",
    "map",
    "extract",
    "search",
    "research",
    "embed",
    "debug",
    "doctor",
    "query",
    "retrieve",
    "ask",
    "evaluate",
    "suggest",
    "sources",
    "domains",
    "stats",
    "status",
    "dedupe",
    "github",
    "reddit",
    "youtube",
    "sessions",
    "screenshot",
];

/// Known flag names — only these are passed through to the subprocess.
/// Infrastructure secrets/URLs are intentionally excluded (pg-url, redis-url,
/// amqp-url, qdrant-url, tei-url, openai-base-url, openai-api-key, openai-model).
const ALLOWED_FLAGS: &[(&str, &str)] = &[
    ("max_pages", "--max-pages"),
    ("max_depth", "--max-depth"),
    ("limit", "--limit"),
    ("collection", "--collection"),
    ("format", "--format"),
    ("render_mode", "--render-mode"),
    ("include_subdomains", "--include-subdomains"),
    ("discover_sitemaps", "--discover-sitemaps"),
    ("sitemap_since_days", "--sitemap-since-days"),
    ("embed", "--embed"),
    ("diagnostics", "--diagnostics"),
    ("yes", "--yes"),
    ("wait", "--wait"),
    ("research_depth", "--research-depth"),
    ("search_time_range", "--search-time-range"),
    ("sort", "--sort"),
    ("time", "--time"),
    ("max_posts", "--max-posts"),
    ("min_score", "--min-score"),
    ("scrape_links", "--scrape-links"),
    ("include_source", "--include-source"),
    ("claude", "--claude"),
    ("codex", "--codex"),
    ("gemini", "--gemini"),
    ("project", "--project"),
    ("output_dir", "--output-dir"),
    ("delay_ms", "--delay-ms"),
    ("request_timeout_ms", "--request-timeout-ms"),
    ("performance_profile", "--performance-profile"),
    ("batch_concurrency", "--batch-concurrency"),
    ("depth", "--depth"),
];

/// Modes that enqueue async jobs — never force `--wait true`.
/// We capture the job ID from stdout and poll for completion.
const ASYNC_MODES: &[&str] = &["crawl", "extract", "embed", "github", "reddit", "youtube"];

/// Modes where `--json` must NOT be injected (they don't support it).
const NO_JSON_MODES: &[&str] = &["search", "research"];

/// Strip ANSI escape sequences from a string.
fn strip_ansi(s: &str) -> String {
    console::strip_ansi_codes(s).into_owned()
}

/// Resolve the axon binary path.
/// Uses `AXON_BIN` env var if set, then tries known local build locations,
/// then falls back to `axon` on PATH.
fn resolve_exe() -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("AXON_BIN") {
        if !p.is_empty() {
            let path = PathBuf::from(&p);
            if path.exists() {
                return Ok(path);
            }
            // AXON_BIN is set but the path doesn't exist — fall through to
            // auto-discovery so container images with the binary on PATH still work.
        }
    }

    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(current) = std::env::current_exe() {
        candidates.push(current.clone());
        if let Some(bin_dir) = current.parent() {
            candidates.push(bin_dir.join("axon"));
            if let Some(target_dir) = bin_dir.parent() {
                candidates.push(target_dir.join("debug").join("axon"));
                candidates.push(target_dir.join("release").join("axon"));
            }
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("target").join("debug").join("axon"));
        candidates.push(cwd.join("target").join("release").join("axon"));
        candidates.push(cwd.join("scripts").join("axon"));
    }

    let mut seen = HashSet::new();
    for candidate in candidates {
        if seen.insert(candidate.clone()) && candidate.exists() {
            return Ok(candidate);
        }
    }

    // PATH fallback. This keeps the web server functional even if the
    // original process binary was deleted after startup.
    Ok(PathBuf::from("axon"))
}

/// Build the argument list from mode, input, and flags.
/// Returns the args vector with `--json` injected where appropriate.
/// For async modes, strips any `--wait` flag the client may have sent
/// (the server controls wait semantics via fire-and-forget + poll).
fn build_args(mode: &str, input: &str, flags: &serde_json::Value) -> Vec<String> {
    let is_async = ASYNC_MODES.contains(&mode);
    let mut args: Vec<String> = vec![mode.to_string()];

    // Input goes as positional arguments.  For job sub-commands like
    // "cancel <id>" or "status <id>" the frontend sends a single string
    // that must be split into separate positional args.  For everything
    // else (URLs, query text) the whole string is one arg.
    let trimmed = input.trim();
    if !trimmed.is_empty() {
        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        let is_job_subcmd = matches!(
            parts[0],
            "cancel" | "status" | "errors" | "list" | "cleanup" | "clear" | "worker" | "recover"
        );
        if is_job_subcmd {
            for part in parts {
                let p = part.trim();
                if !p.is_empty() {
                    args.push(p.to_string());
                }
            }
        } else {
            args.push(trimmed.to_string());
        }
    }

    // Inject --json for all modes except search/research (which don't support it).
    // Async modes need it to parse the job ID; sync modes need it for structured stdout.
    if !NO_JSON_MODES.contains(&mode) {
        args.push("--json".to_string());
    }

    // Scrape: disable embed so the web UI does not depend on TEI being reachable.
    // The scrape result is delivered as JSON via stdout; embedding is a background concern.
    if mode == "scrape" {
        args.push("--embed".to_string());
        args.push("false".to_string());
    }

    // Whitelist-based flag mapping
    if let Some(obj) = flags.as_object() {
        for (json_key, cli_flag) in ALLOWED_FLAGS {
            // S1: strip --wait from async modes — the server controls wait
            // semantics via fire-and-forget + poll, so a client-supplied
            // --wait true would break the async flow.
            if is_async && *json_key == "wait" {
                continue;
            }
            if let Some(val) = obj.get(*json_key) {
                match val {
                    serde_json::Value::Bool(true) => {
                        args.push(cli_flag.to_string());
                    }
                    serde_json::Value::Bool(false) => {
                        args.push(cli_flag.to_string());
                        args.push("false".to_string());
                    }
                    serde_json::Value::Number(n) => {
                        args.push(cli_flag.to_string());
                        args.push(n.to_string());
                    }
                    serde_json::Value::String(s) if !s.is_empty() => {
                        args.push(cli_flag.to_string());
                        args.push(s.clone());
                    }
                    _ => {}
                }
            }
        }
    }

    args
}

/// Execute a CLI command as a subprocess, streaming stderr as log lines.
/// Async modes (crawl, embed, extract, etc.) enqueue the job and poll for
/// completion. Synchronous modes block until the subprocess exits.
pub(super) async fn handle_command(
    mode: &str,
    input: &str,
    flags: &serde_json::Value,
    tx: mpsc::Sender<String>,
    crawl_job_id: Arc<Mutex<Option<String>>>,
) {
    let mut context = ExecCommandContext {
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

    let args = build_args(&context.mode, &context.input, flags);
    let start = Instant::now();

    // Notify the frontend what command is about to run so it can activate
    // the appropriate renderer before any output arrives.
    send_command_start(&tx, &context).await;

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
        handle_async_command(child, &mut context, &tx, crawl_job_id, start).await;
    } else {
        handle_sync_command(child, &context, &tx, start).await;
    }
}

/// Handle an async-mode command: capture the job ID from stdout, stream stderr
/// as log lines, then poll for job completion.
async fn handle_async_command(
    mut child: tokio::process::Child,
    context: &mut ExecCommandContext,
    tx: &mpsc::Sender<String>,
    crawl_job_id: Arc<Mutex<Option<String>>>,
    start: Instant,
) {
    let ws_ctx = context.to_ws_ctx();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stderr_tx = tx.clone();

    // Capture stdout lines to extract the job ID JSON
    let stdout_capture = tokio::spawn(async move {
        let stdout = stdout?;
        let mut lines = BufReader::new(stdout).lines();
        let mut job_id: Option<String> = None;
        while let Ok(Some(line)) = lines.next_line().await {
            let clean = line.trim().to_string();
            if clean.is_empty() {
                continue;
            }
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&clean) {
                if let Some(id) = parsed.get("job_id").and_then(|v| v.as_str()) {
                    job_id = Some(id.to_string());
                }
            }
        }
        job_id
    });

    // Stream stderr as log lines
    let stderr_task = tokio::spawn(async move {
        let Some(stderr) = stderr else { return };
        let mut lines = BufReader::new(stderr).lines();
        let mut last_stderr = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            let clean = strip_ansi(&line);
            if clean.trim().is_empty() {
                continue;
            }
            if clean == last_stderr {
                continue;
            }
            last_stderr.clone_from(&clean);
            if stderr_tx
                .send(json!({"type": "log", "line": clean}).to_string())
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let _ = tokio::join!(stderr_task);
    let _ = child.wait().await;

    let job_id = stdout_capture.await.ok().flatten();

    if let Some(id) = job_id {
        // Store job ID for cancel support
        *crawl_job_id.lock().await = Some(id.clone());

        let _ = tx
            .send(
                json!({"type": "log", "line": format!("[web] {} job enqueued: {id}", context.mode)})
                    .to_string(),
            )
            .await;

        // Poll the job for progress/completion
        let mode_str = context.mode.clone();
        let input_str = context.input.trim().to_string();
        poll_async_job(&id, &mode_str, &input_str, &ws_ctx, tx, start).await;

        // Clear job ID after completion
        *crawl_job_id.lock().await = None;
    } else {
        let elapsed = start.elapsed().as_millis() as u64;
        send_error_dual(
            tx,
            &ws_ctx,
            format!("failed to capture {} job ID from subprocess", context.mode),
            Some(elapsed),
        )
        .await;
    }
}

/// Handle a synchronous command: stream stdout as structured messages,
/// stream stderr as log lines, wait for exit, and send done/error.
async fn handle_sync_command(
    mut child: tokio::process::Child,
    context: &ExecCommandContext,
    tx: &mpsc::Sender<String>,
    start: Instant,
) {
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_tx = tx.clone();
    let stderr_tx = tx.clone();
    let is_screenshot = context.mode == "screenshot";
    let ws_ctx = context.to_ws_ctx();
    let stdout_ctx = ws_ctx.clone();

    let stdout_task = tokio::spawn(async move {
        let Some(stdout) = stdout else {
            return Vec::new();
        };
        let mut lines = BufReader::new(stdout).lines();
        // Collect screenshot JSON objects (have `path` + `size_bytes` + `url`)
        // so we can send a screenshot_files message from deterministic data
        // instead of relying on a fragile filesystem timestamp scan.
        let mut screenshot_jsons: Vec<serde_json::Value> = Vec::new();
        // Accumulate full stdout so pretty-printed JSON objects can be parsed
        // after the stream ends (line-by-line parsing misses multiline JSON).
        let mut stdout_accum = String::new();
        let mut saw_json_line = false;

        while let Ok(Some(line)) = lines.next_line().await {
            let clean = strip_ansi(&line);
            if clean.trim().is_empty() {
                continue;
            }
            if !stdout_accum.is_empty() {
                stdout_accum.push('\n');
            }
            stdout_accum.push_str(&clean);
            // Try to parse as JSON for structured rendering; fall back to raw text.
            // Only treat objects and arrays as meaningful structured output — primitive
            // JSON values (strings, numbers, bools) are likely array elements from
            // pretty-printed multiline JSON and should not suppress the end-of-stream
            // full-document recovery pass that correctly parses the whole blob.
            match serde_json::from_str::<serde_json::Value>(&clean) {
                Ok(parsed) if parsed.is_object() || parsed.is_array() => {
                    saw_json_line = true;
                    if is_screenshot {
                        screenshot_jsons.push(parsed.clone());
                    }
                    send_command_output_json(&stdout_tx, &stdout_ctx, parsed).await;
                }
                Ok(_) | Err(_) => {
                    send_command_output_line(&stdout_tx, &stdout_ctx, clean).await;
                }
            }
        }

        // Final recovery pass for multiline pretty JSON output.
        // If the full stdout parses, emit a single structured payload so the
        // frontend can render rich components instead of raw JSON text.
        if !saw_json_line
            && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(stdout_accum.trim())
        {
            send_command_output_json(&stdout_tx, &stdout_ctx, parsed).await;
        }

        screenshot_jsons
    });

    let stderr_task = tokio::spawn(async move {
        let Some(stderr) = stderr else { return };
        let mut lines = BufReader::new(stderr).lines();
        let mut last_stderr = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            let clean = strip_ansi(&line);
            if clean.trim().is_empty() {
                continue;
            }
            if clean == last_stderr {
                continue;
            }
            last_stderr.clone_from(&clean);
            if stderr_tx
                .send(json!({"type": "log", "line": clean}).to_string())
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let (stdout_result, _) = tokio::join!(stdout_task, stderr_task);
    let screenshot_jsons = stdout_result.unwrap_or_default();

    let status = child.wait().await;
    let elapsed = start.elapsed().as_millis() as u64;

    match status {
        Ok(exit) => {
            let code = exit.code().unwrap_or(-1);
            if code == 0 {
                if context.mode == "scrape" {
                    send_scrape_file(tx, &ws_ctx).await;
                }
                if context.mode == "screenshot" {
                    // Build screenshot_files from captured stdout JSON
                    // (deterministic — no filesystem timestamp scan)
                    files::send_screenshot_files_from_json(&screenshot_jsons, tx, &ws_ctx).await;
                }
                send_done_dual(tx, &ws_ctx, code, Some(elapsed)).await;
            } else {
                send_error_dual(tx, &ws_ctx, format!("exit code {code}"), Some(elapsed)).await;
            }
        }
        Err(e) => {
            send_error_dual(tx, &ws_ctx, format!("wait failed: {e}"), None).await;
        }
    }
}

/// Cancel a running async job by spawning `axon <mode> cancel <id>`.
/// Falls back to `crawl` if no mode is specified (legacy callers).
pub(super) async fn handle_cancel(mode: &str, job_id: &str, tx: mpsc::Sender<String>) {
    let cancel_mode = if mode.is_empty() { "crawl" } else { mode };
    let ws_ctx = events::CommandContext {
        exec_id: format!("exec-{}", Uuid::new_v4()),
        mode: cancel_mode.to_string(),
        input: job_id.to_string(),
    };
    if !is_valid_cancel_job_id(job_id) {
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
    let exe = match resolve_exe() {
        Ok(p) => p,
        Err(e) => {
            send_error_dual(&tx, &ws_ctx, format!("cannot find axon binary: {e}"), None).await;
            return;
        }
    };

    let output = Command::new(&exe)
        .args([cancel_mode, "cancel", job_id, "--json"])
        .output()
        .await;

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let parsed = serde_json::from_str::<serde_json::Value>(stdout.trim()).ok();
            let ok = cancel_ok_from_output(parsed.as_ref(), out.status.success());
            let message = parsed
                .as_ref()
                .and_then(|v| v.get("message"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or_else(|| {
                    let trimmed = stderr.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                });

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

            if !ok {
                send_error_dual(
                    &tx,
                    &ws_ctx,
                    format!(
                        "cancel failed{}",
                        out.status
                            .code()
                            .map(|code| format!(": exit code {code}"))
                            .unwrap_or_default()
                    ),
                    None,
                )
                .await;
            } else {
                // Use exit code 0 for a successful cancel — the UI treats non-zero
                // codes as failures, so 130 (SIGINT) would incorrectly show the job
                // as failed even when the cancel completed successfully.
                send_done_dual(&tx, &ws_ctx, 0, None).await;
            }
        }
        Err(e) => {
            send_error_dual(&tx, &ws_ctx, format!("cancel failed: {e}"), None).await;
        }
    }
}
