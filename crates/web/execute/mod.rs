mod files;
mod polling;

pub(crate) use files::handle_read_file;

use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{Mutex, mpsc};

use files::{send_scrape_file, send_screenshot_files};
use polling::poll_async_job;

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
/// Uses `AXON_BIN` env var if set, otherwise `current_exe()`.
fn resolve_exe() -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("AXON_BIN") {
        let path = PathBuf::from(&p);
        if path.exists() {
            return Ok(path);
        }
        return Err(format!("AXON_BIN={p} does not exist"));
    }
    match std::env::current_exe() {
        Ok(p) => {
            if p.exists() {
                Ok(p)
            } else {
                Err(format!(
                    "current_exe() returned {} but file does not exist",
                    p.display()
                ))
            }
        }
        Err(e) => Err(format!("current_exe() failed: {e}")),
    }
}

/// Build the argument list from mode, input, and flags.
/// Returns the args vector with `--json` injected where appropriate.
/// For async modes, strips any `--wait` flag the client may have sent
/// (the server controls wait semantics via fire-and-forget + poll).
fn build_args(mode: &str, input: &str, flags: &serde_json::Value) -> Vec<String> {
    let is_async = ASYNC_MODES.contains(&mode);
    let mut args: Vec<String> = vec![mode.to_string()];

    // Input goes as a positional argument (URL, query text, etc.)
    let trimmed = input.trim();
    if !trimmed.is_empty() {
        args.push(trimmed.to_string());
    }

    // Inject --json for all modes except search/research (which don't support it).
    // Async modes need it to parse the job ID; sync modes need it for structured stdout.
    if !NO_JSON_MODES.contains(&mode) {
        args.push("--json".to_string());
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
    if !ALLOWED_MODES.contains(&mode) {
        let _ = tx
            .send(json!({"type": "error", "message": format!("unknown mode: {mode}")}).to_string())
            .await;
        return;
    }

    let exe = match resolve_exe() {
        Ok(p) => p,
        Err(e) => {
            let _ = tx
                .send(
                    json!({"type": "error", "message": format!("cannot find axon binary: {e}")})
                        .to_string(),
                )
                .await;
            return;
        }
    };

    let args = build_args(mode, input, flags);
    let start = Instant::now();

    // Notify the frontend what command is about to run so it can activate
    // the appropriate renderer before any output arrives.
    let _ = tx
        .send(json!({"type": "command_start", "mode": mode}).to_string())
        .await;

    let child = Command::new(&exe)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    let child = match child {
        Ok(c) => c,
        Err(e) => {
            let _ = tx
                .send(json!({"type": "error", "message": format!("spawn failed: {e} (exe: {})", exe.display())}).to_string())
                .await;
            return;
        }
    };

    if ASYNC_MODES.contains(&mode) {
        handle_async_command(child, mode, input.trim(), &tx, crawl_job_id, start).await;
    } else {
        handle_sync_command(child, mode, &tx, start).await;
    }
}

/// Handle an async-mode command: capture the job ID from stdout, stream stderr
/// as log lines, then poll for job completion.
async fn handle_async_command(
    mut child: tokio::process::Child,
    mode: &str,
    input: &str,
    tx: &mpsc::Sender<String>,
    crawl_job_id: Arc<Mutex<Option<String>>>,
    start: Instant,
) {
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

    if let Some(ref id) = job_id {
        // Store job ID for cancel support
        *crawl_job_id.lock().await = Some(id.clone());

        let _ = tx
            .send(
                json!({"type": "log", "line": format!("[web] {mode} job enqueued: {id}")})
                    .to_string(),
            )
            .await;

        // Poll the job for progress/completion
        let mode_str = mode.to_string();
        let input_str = input.to_string();
        poll_async_job(id, &mode_str, &input_str, tx, start).await;

        // Clear job ID after completion
        *crawl_job_id.lock().await = None;
    } else {
        let elapsed = start.elapsed().as_millis() as u64;
        let _ = tx
            .send(
                json!({"type": "error", "message": format!("failed to capture {mode} job ID from subprocess"), "elapsed_ms": elapsed})
                    .to_string(),
            )
            .await;
    }
}

/// Handle a synchronous command: stream stdout as structured messages,
/// stream stderr as log lines, wait for exit, and send done/error.
async fn handle_sync_command(
    mut child: tokio::process::Child,
    mode: &str,
    tx: &mpsc::Sender<String>,
    start: Instant,
) {
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_tx = tx.clone();
    let stderr_tx = tx.clone();

    let stdout_task = tokio::spawn(async move {
        let Some(stdout) = stdout else { return };
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let clean = strip_ansi(&line);
            if clean.trim().is_empty() {
                continue;
            }
            // Try to parse as JSON for structured rendering; fall back to raw text.
            let msg = match serde_json::from_str::<serde_json::Value>(&clean) {
                Ok(parsed) => json!({"type": "stdout_json", "data": parsed}).to_string(),
                Err(_) => json!({"type": "stdout_line", "line": clean}).to_string(),
            };
            if stdout_tx.send(msg).await.is_err() {
                break;
            }
        }
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

    let _ = tokio::join!(stdout_task, stderr_task);

    let status = child.wait().await;
    let elapsed = start.elapsed().as_millis() as u64;
    let mode_owned = mode.to_string();

    match status {
        Ok(exit) => {
            let code = exit.code().unwrap_or(-1);
            if code == 0 {
                if mode_owned == "scrape" {
                    send_scrape_file(tx).await;
                }
                if mode_owned == "screenshot" {
                    send_screenshot_files(tx).await;
                }
                let _ = tx
                    .send(
                        json!({"type": "done", "exit_code": code, "elapsed_ms": elapsed})
                            .to_string(),
                    )
                    .await;
            } else {
                let _ = tx
                    .send(
                        json!({"type": "error", "message": format!("exit code {code}"), "elapsed_ms": elapsed})
                            .to_string(),
                    )
                    .await;
            }
        }
        Err(e) => {
            let _ = tx
                .send(json!({"type": "error", "message": format!("wait failed: {e}")}).to_string())
                .await;
        }
    }
}

/// Cancel a running async job by spawning `axon <mode> cancel <id>`.
pub(super) async fn handle_cancel(job_id: &str, tx: mpsc::Sender<String>) {
    let exe = match resolve_exe() {
        Ok(p) => p,
        Err(e) => {
            let _ = tx
                .send(
                    json!({"type": "error", "message": format!("cannot find axon binary: {e}")})
                        .to_string(),
                )
                .await;
            return;
        }
    };

    // Try crawl cancel first (most common case from web UI)
    let output = Command::new(&exe)
        .args(["crawl", "cancel", job_id])
        .output()
        .await;

    match output {
        Ok(out) => {
            let _ = tx
                .send(
                    json!({"type": "done", "exit_code": out.status.code().unwrap_or(-1)})
                        .to_string(),
                )
                .await;
        }
        Err(e) => {
            let _ = tx
                .send(
                    json!({"type": "error", "message": format!("cancel failed: {e}")}).to_string(),
                )
                .await;
        }
    }
}
