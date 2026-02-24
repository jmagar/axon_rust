use crate::crates::core::content::url_to_domain;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{Mutex, mpsc};

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

/// Resolve the output directory for reading crawl results.
/// Checks `AXON_WORKER_OUTPUT_DIR` first (host path to Docker bind mount),
/// then `AXON_OUTPUT_DIR`, then the default relative path.
fn output_dir() -> PathBuf {
    std::env::var("AXON_WORKER_OUTPUT_DIR")
        .or_else(|_| std::env::var("AXON_OUTPUT_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".cache/axon-rust/output"))
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

/// Find the most recently modified `.md` file in a directory.
async fn newest_md_file(dir: &Path) -> Option<PathBuf> {
    let mut entries = tokio::fs::read_dir(dir).await.ok()?;
    let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "md") {
            if let Ok(meta) = entry.metadata().await {
                if let Ok(modified) = meta.modified() {
                    if newest.as_ref().is_none_or(|(_, t)| modified > *t) {
                        newest = Some((path, modified));
                    }
                }
            }
        }
    }
    newest.map(|(p, _)| p)
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

    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            let _ = tx
                .send(json!({"type": "error", "message": format!("spawn failed: {e} (exe: {})", exe.display())}).to_string())
                .await;
            return;
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let is_async = ASYNC_MODES.contains(&mode);

    if is_async {
        // Async mode: capture stdout for job ID, stream stderr, then poll
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
            let input_str = trimmed.to_string();
            poll_async_job(id, &mode_str, &input_str, &tx, start).await;

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
    } else {
        // Synchronous mode: stream stdout as structured messages, stream stderr, wait for exit
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
                        send_scrape_file(&tx).await;
                    }
                    if mode_owned == "screenshot" {
                        send_screenshot_files(&tx).await;
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
                    .send(
                        json!({"type": "error", "message": format!("wait failed: {e}")})
                            .to_string(),
                    )
                    .await;
            }
        }
    }
}

/// Poll an async job for completion. For crawl jobs, also sends `crawl_progress`
/// messages with live page counts and reads the manifest on completion.
async fn poll_async_job(
    job_id: &str,
    mode: &str,
    input_url: &str,
    tx: &mpsc::Sender<String>,
    start: Instant,
) {
    // Status subcommand: `axon <mode> status <job_id> --json`
    // For ingest modes (github/reddit/youtube), the status command is the mode itself.
    let status_cmd = mode.to_string();

    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;

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
            let result = status_json.get("metrics").cloned().unwrap_or(json!({}));
            let _ = tx
                .send(
                    json!({
                        "type": "crawl_progress",
                        "job_id": job_id,
                        "status": status,
                        "pages_crawled": result.get("pages_crawled").and_then(|v| v.as_u64()).unwrap_or(0),
                        "pages_discovered": result.get("pages_discovered").and_then(|v| v.as_u64()).unwrap_or(0),
                        "md_created": result.get("md_created").and_then(|v| v.as_u64()).unwrap_or(0),
                        "thin_md": result.get("thin_md").and_then(|v| v.as_u64()).unwrap_or(0),
                        "phase": result.get("phase").and_then(|v| v.as_str()).unwrap_or("pending"),
                    })
                    .to_string(),
                )
                .await;
        }

        match status {
            "completed" => {
                // For crawl, read manifest and send file list.
                // The worker may run inside Docker with a different base path.
                // Extract the domain/uuid suffix from metrics.output_dir and
                // resolve it against the host-side output_dir().
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

/// Send a single scraped markdown file to the frontend.
async fn send_scrape_file(tx: &mpsc::Sender<String>) {
    let md_dir = output_dir().join("scrape-markdown");
    match newest_md_file(&md_dir).await {
        Some(path) => match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                let _ = tx
                    .send(
                        json!({
                            "type": "file_content",
                            "path": path.to_string_lossy(),
                            "content": content,
                        })
                        .to_string(),
                    )
                    .await;
            }
            Err(e) => {
                let _ = tx
                    .send(
                        json!({"type": "log", "line": format!("[web] read error: {e}")})
                            .to_string(),
                    )
                    .await;
            }
        },
        None => {
            let _ = tx
                .send(
                    json!({"type": "log", "line": format!("[web] no .md files found in {}", md_dir.display())})
                        .to_string(),
                )
                .await;
        }
    }
}

/// Send recently modified screenshot files to the frontend after a screenshot command.
/// Globs `output_dir/screenshots/*.png` and sends paths for any files modified in
/// the last 60 seconds (conservative window to catch the just-completed run).
async fn send_screenshot_files(tx: &mpsc::Sender<String>) {
    let screenshots_dir = output_dir().join("screenshots");
    let Ok(mut entries) = tokio::fs::read_dir(&screenshots_dir).await else {
        let _ = tx
            .send(
                json!({"type": "log", "line": format!("[web] no screenshots dir at {}", screenshots_dir.display())})
                    .to_string(),
            )
            .await;
        return;
    };

    let cutoff = std::time::SystemTime::now() - Duration::from_secs(60);
    let mut files = Vec::new();

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "png") {
            if let Ok(meta) = entry.metadata().await {
                if let Ok(modified) = meta.modified() {
                    if modified >= cutoff {
                        let name = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();
                        files.push(json!({
                            "path": path.to_string_lossy(),
                            "name": name,
                        }));
                    }
                }
            }
        }
    }

    if !files.is_empty() {
        let _ = tx
            .send(json!({"type": "screenshot_files", "files": files}).to_string())
            .await;
    }
}

/// Send the crawl manifest file list to the frontend from a job output directory.
async fn send_crawl_manifest(job_dir: &Path, tx: &mpsc::Sender<String>) {
    let manifest = job_dir.join("manifest.jsonl");

    // If the job-specific dir doesn't have a manifest yet, try `latest/`
    // (the reflink is updated after crawl completes, before marking done)
    let manifest = if tokio::fs::metadata(&manifest).await.is_ok() {
        manifest
    } else if let Some(parent) = job_dir.parent() {
        let latest = parent.join("latest").join("manifest.jsonl");
        if tokio::fs::metadata(&latest).await.is_ok() {
            latest
        } else {
            let _ = tx
                .send(
                    json!({"type": "log", "line": format!("[web] no manifest at {}", manifest.display())})
                        .to_string(),
                )
                .await;
            return;
        }
    } else {
        let _ = tx
            .send(json!({"type": "log", "line": "[web] no crawl manifest found"}).to_string())
            .await;
        return;
    };

    let base_dir = manifest
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    match tokio::fs::read_to_string(&manifest).await {
        Ok(raw) => {
            let mut files = Vec::new();
            for line in raw.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
                    continue;
                };
                let url = entry.get("url").and_then(|v| v.as_str()).unwrap_or("");
                let rel = entry
                    .get("relative_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let chars = entry
                    .get("markdown_chars")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if !rel.is_empty() {
                    files.push(json!({
                        "url": url,
                        "relative_path": rel,
                        "markdown_chars": chars,
                    }));
                }
            }

            let _ = tx
                .send(
                    json!({
                        "type": "crawl_files",
                        "files": files,
                        "output_dir": base_dir.to_string_lossy(),
                    })
                    .to_string(),
                )
                .await;

            // Auto-load the first file
            if let Some(first) = files.first() {
                if let Some(rel) = first.get("relative_path").and_then(|v| v.as_str()) {
                    let full = base_dir.join(rel);
                    if let Ok(content) = tokio::fs::read_to_string(&full).await {
                        let _ = tx
                            .send(
                                json!({
                                    "type": "file_content",
                                    "path": full.to_string_lossy(),
                                    "content": content,
                                })
                                .to_string(),
                            )
                            .await;
                    }
                }
            }
        }
        Err(e) => {
            let _ = tx
                .send(
                    json!({"type": "log", "line": format!("[web] manifest read error: {e}")})
                        .to_string(),
                )
                .await;
        }
    }
}

/// Read a file on demand from a crawl output directory.
/// Validates the path is within the base directory to prevent traversal attacks.
pub(super) async fn handle_read_file(
    relative_path: &str,
    base_dir: &Path,
    tx: mpsc::Sender<String>,
) {
    let full_path = base_dir.join(relative_path);
    let Ok(canonical_base) = tokio::fs::canonicalize(base_dir).await else {
        let _ = tx
            .send(json!({"type": "error", "message": "invalid base directory"}).to_string())
            .await;
        return;
    };
    let Ok(canonical_path) = tokio::fs::canonicalize(&full_path).await else {
        let _ = tx
            .send(json!({"type": "error", "message": "file not found"}).to_string())
            .await;
        return;
    };

    if !canonical_path.starts_with(&canonical_base) {
        let _ = tx
            .send(json!({"type": "error", "message": "path outside output directory"}).to_string())
            .await;
        return;
    }

    match tokio::fs::read_to_string(&canonical_path).await {
        Ok(content) => {
            let _ = tx
                .send(
                    json!({
                        "type": "file_content",
                        "path": canonical_path.to_string_lossy(),
                        "content": content,
                    })
                    .to_string(),
                )
                .await;
        }
        Err(e) => {
            let _ = tx
                .send(json!({"type": "error", "message": format!("read error: {e}")}).to_string())
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
