use crate::crates::core::config::{Config, ConfigOverrides};
use crate::crates::services::acp as acp_svc;
use crate::crates::services::events::ServiceEvent;
use crate::crates::services::map as map_svc;
use crate::crates::services::query as query_svc;
use crate::crates::services::scrape as scrape_svc;
use crate::crates::services::search as search_svc;
use crate::crates::services::system as system_svc;
use crate::crates::services::types::{
    AcpAdapterCommand, AcpPromptTurnRequest, AcpSessionProbeRequest, AskResult, DoctorResult,
    DomainsResult, MapOptions, MapResult, Pagination, QueryResult, ResearchResult, RetrieveOptions,
    RetrieveResult, ScrapeResult, SearchOptions, SearchResult, SourcesResult, StatsResult,
    StatusResult,
};
use serde_json::json;
use std::env;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

use super::context::ExecCommandContext;
use super::events::{CommandContext, WsEventV2, serialize_v2_event};
use super::exe::strip_ansi;
use super::files;
use super::ws_send::{send_command_output_line, send_done_dual, send_error_dual};

/// Modes dispatched directly through service functions (no subprocess).
///
/// This constant is the authoritative list consumed by tests and must stay in
/// sync with [`ServiceMode`].  At runtime `classify_sync_direct` uses
/// `ServiceMode::from_str` directly, so this constant is dead in non-test
/// builds — the `allow` below silences the warning intentionally.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) const DIRECT_SYNC_MODES: &[&str] = &[
    "scrape",
    "map",
    "query",
    "retrieve",
    "ask",
    "search",
    "research",
    "stats",
    "sources",
    "domains",
    "doctor",
    "status",
    "pulse_chat",
    "pulse_chat_probe",
];

/// Owned parameters extracted from the WS request before any `.await`.
///
/// All fields are owned so the containing future is `Send + 'static`.
/// Visibility is `pub(super)` so `execute.rs` can pass the opaque value from
/// `classify_sync_direct` to `handle_sync_direct` without inspecting its fields.
///
/// `cfg` is kept as `Arc<Config>` (not a plain `Config`) so that the
/// `call_*` service wrappers can clone the `Arc` into `async move` blocks
/// and borrow from the Arc-owned data without exposing a lifetime parameter
/// to `tokio::task::spawn`'s `Send + 'static` check.
pub(super) struct DirectParams {
    mode: ServiceMode,
    input: String,
    cfg: Arc<Config>,
    limit: usize,
    offset: usize,
    max_points: Option<usize>,
    agent: PulseChatAgent,
    session_id: Option<String>,
    model: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PulseChatAgent {
    Claude,
    Codex,
}

impl PulseChatAgent {
    fn from_flag(value: Option<&str>) -> Self {
        match value {
            Some(raw) if raw.eq_ignore_ascii_case("codex") => Self::Codex,
            _ => Self::Claude,
        }
    }
}

/// Classified service mode — replaces `mode: String` in `DirectParams` so the
/// async state machine never holds a `&str` borrow across `.await` points.
///
/// The `match mode.as_str()` scrutinee in `dispatch_service` would otherwise
/// create an `&str` borrow that the Rust async state machine includes in every
/// generated `Future::poll` state variant, causing an HRTB `Send` diagnostic
/// when the future is submitted to `tokio::task::spawn`.
///
/// By classifying the mode synchronously (before the first `.await`) we drop
/// the `&str` borrow before any suspension point, satisfying the
/// `Send + 'static` bound.
#[derive(Debug, Clone, Copy)]
enum ServiceMode {
    Scrape,
    Map,
    Query,
    Retrieve,
    Ask,
    Search,
    Research,
    Stats,
    Sources,
    Domains,
    Doctor,
    Status,
    PulseChat,
    PulseChatProbe,
}

impl ServiceMode {
    /// Classify a mode string.  Returns `None` for unknown modes.
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "scrape" => Some(Self::Scrape),
            "map" => Some(Self::Map),
            "query" => Some(Self::Query),
            "retrieve" => Some(Self::Retrieve),
            "ask" => Some(Self::Ask),
            "search" => Some(Self::Search),
            "research" => Some(Self::Research),
            "stats" => Some(Self::Stats),
            "sources" => Some(Self::Sources),
            "domains" => Some(Self::Domains),
            "doctor" => Some(Self::Doctor),
            "status" => Some(Self::Status),
            "pulse_chat" => Some(Self::PulseChat),
            "pulse_chat_probe" => Some(Self::PulseChatProbe),
            _ => None,
        }
    }
}

/// Extract a `usize` from a flags JSON value, falling back to `default`.
fn flag_usize(flags: &serde_json::Value, key: &str, default: usize) -> usize {
    flags
        .get(key)
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(default)
}

/// Extract an optional `usize` from a flags JSON value.
fn flag_opt_usize(flags: &serde_json::Value, key: &str) -> Option<usize> {
    flags.get(key).and_then(|v| v.as_u64()).map(|n| n as usize)
}

/// Delimiter for `AXON_ACP_ADAPTER_ARGS`.
///
/// The env var is parsed as a pipe-delimited list (e.g. `--flag|value|--stdio`).
/// Empty segments are ignored.
const ACP_ADAPTER_ARGS_DELIMITER: char = '|';

/// Parse pipe-delimited adapter args from `AXON_ACP_ADAPTER_ARGS`.
fn parse_acp_adapter_args(raw: &str) -> Vec<String> {
    raw.split(ACP_ADAPTER_ARGS_DELIMITER)
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn resolve_acp_adapter_command_from_values(
    cmd_value: Option<&str>,
    args_value: Option<&str>,
) -> Result<AcpAdapterCommand, String> {
    let program = cmd_value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "missing required env var AXON_ACP_ADAPTER_CMD for pulse_chat".to_string())?
        .to_string();

    let args = args_value.map(parse_acp_adapter_args).unwrap_or_default();

    Ok(AcpAdapterCommand {
        program,
        args,
        cwd: None,
    })
}

/// Resolve ACP adapter command and args for `pulse_chat`.
///
/// Values are parsed from `Config` fields sourced from environment parsing:
/// - `Config::acp_adapter_cmd` from `AXON_ACP_ADAPTER_CMD` (required)
/// - `Config::acp_adapter_args` from `AXON_ACP_ADAPTER_ARGS` (optional)
fn resolve_acp_adapter_command(
    cfg: &Config,
    agent: PulseChatAgent,
) -> Result<AcpAdapterCommand, String> {
    let (cmd_env_key, args_env_key) = match agent {
        PulseChatAgent::Claude => (
            "AXON_ACP_CLAUDE_ADAPTER_CMD",
            "AXON_ACP_CLAUDE_ADAPTER_ARGS",
        ),
        PulseChatAgent::Codex => ("AXON_ACP_CODEX_ADAPTER_CMD", "AXON_ACP_CODEX_ADAPTER_ARGS"),
    };

    let cmd_override = env::var(cmd_env_key).ok();
    let args_override = env::var(args_env_key).ok();

    resolve_acp_adapter_command_from_values(
        cmd_override.as_deref().or(cfg.acp_adapter_cmd.as_deref()),
        args_override.as_deref().or(cfg.acp_adapter_args.as_deref()),
    )
}

/// Build a per-request `Config` wrapped in `Arc` by applying collection + limit
/// overrides from flags.
///
/// The returned `Arc<Config>` is used in `DirectParams` so that `call_*` wrappers
/// can clone the `Arc` into `async move` blocks.  Borrows from Arc-owned data
/// (`&*cfg`) are confined to each wrapper's own state machine and do not generate
/// HRTB `for<'a> &'a Config: Send` constraints visible to `tokio::spawn`.
fn derive_cfg(context: &ExecCommandContext, flags: &serde_json::Value) -> Arc<Config> {
    let mut overrides = ConfigOverrides::default();

    if let Some(col) = flags
        .get("collection")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        overrides.collection = Some(col.to_string());
    }
    if let Some(limit) = flag_opt_usize(flags, "limit") {
        overrides.limit = Some(limit);
    }

    Arc::new(context.cfg.apply_overrides(&overrides))
}

/// Extract all parameters from `context` and `flags` into owned values before
/// any `.await`. This ensures the containing future is `Send + 'static`.
///
/// Returns `None` when `context.mode` is not a recognised `ServiceMode` —
/// callers should treat that as "not handled".
fn extract_params(context: &ExecCommandContext, flags: &serde_json::Value) -> Option<DirectParams> {
    // Classify the mode synchronously.  The `&str` borrow from `.as_str()` is
    // dropped at the end of this expression — it never escapes into any async
    // state machine.
    let mode = ServiceMode::from_str(context.mode.as_str())?;

    let cfg = derive_cfg(context, flags);
    let limit = flag_usize(flags, "limit", cfg.search_limit);
    let offset = flag_usize(flags, "offset", 0);
    let max_points = flag_opt_usize(flags, "limit");
    let agent = PulseChatAgent::from_flag(flags.get("agent").and_then(serde_json::Value::as_str));
    let session_id = flags
        .get("session_id")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);
    let model = flags
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);
    Some(DirectParams {
        mode,
        input: context.input.clone(),
        cfg,
        limit,
        offset,
        max_points,
        agent,
        session_id,
        model,
    })
}

/// Send a JSON output event, taking all parameters by owned value to avoid
/// holding borrows across `.await` points in the async state machine.
async fn send_json_owned(tx: mpsc::Sender<String>, ctx: CommandContext, data: serde_json::Value) {
    if let Some(v2) = serialize_v2_event(WsEventV2::CommandOutputJson { ctx, data }) {
        let _ = tx.send(v2).await;
    }
}

/// Send a `command.done` event, taking all parameters by owned value.
async fn send_done_owned(
    tx: mpsc::Sender<String>,
    ctx: CommandContext,
    exit_code: i32,
    elapsed_ms: Option<u64>,
) {
    use super::events::{CommandDonePayload, serialize_v2_event};
    if let Some(v2) = serialize_v2_event(WsEventV2::CommandDone {
        ctx,
        payload: CommandDonePayload {
            exit_code,
            elapsed_ms,
        },
    }) {
        let _ = tx.send(v2).await;
    }
}

/// Send a `command.error` event, taking all parameters by owned value.
async fn send_error_owned(
    tx: mpsc::Sender<String>,
    ctx: CommandContext,
    message: String,
    elapsed_ms: Option<u64>,
) {
    use super::events::{CommandErrorPayload, serialize_v2_event};
    if let Some(v2) = serialize_v2_event(WsEventV2::CommandError {
        ctx,
        payload: CommandErrorPayload {
            message,
            elapsed_ms,
        },
    }) {
        let _ = tx.send(v2).await;
    }
}

// ── Service call wrappers ─────────────────────────────────────────────────────
//
// Each wrapper returns `Pin<Box<dyn Future<Output=…> + Send + 'static>>`.
//
// Why boxing is required
// ──────────────────────
// Service functions take `&Config` and `&str` parameters.  When an `async fn`
// with such parameters is awaited inside a future submitted to
// `tokio::task::spawn`, rustc generates Higher-Ranked Trait Bound (HRTB)
// constraints of the form `for<'a> &'a Config: Send` and `for<'a> &'a str: Send`.
// These constraints are always true at runtime (`Config: Sync`, `str: Sync`),
// but rustc's current HRTB solver cannot prove them in this context
// (rust-lang/rust#96865) and emits "implementation of `Send` is not general
// enough".
//
// The fix: wrap each service call in `Box::pin(async move { … })`.
// • The `async move` block captures `cfg: Arc<Config>` and `input: String`
//   by value.  Both types are `'static`.
// • Inside the block, `&*cfg` and `input.as_str()` borrow data owned by the
//   closure itself — the lifetimes are fully determined and `'static`-adjacent.
// • `Box::pin` erases the concrete future type into `Pin<Box<dyn Future + Send
//   + 'static>>`.  Type erasure eliminates the lifetime parameters that trigger
//   the HRTB check.
// • The returned boxed future is `Send + 'static` by construction, satisfying
//   `tokio::task::spawn`.
//
// `Arc<Config>` (not `Config`) is used so `.clone()` inside each wrapper is a
// cheap reference-count bump, not a full struct copy.

fn call_scrape(
    cfg: Arc<Config>,
    url: String,
) -> Pin<Box<dyn Future<Output = Result<ScrapeResult, String>> + Send + 'static>> {
    Box::pin(async move {
        scrape_svc::scrape(&cfg, &url)
            .await
            .map_err(|e| e.to_string())
    })
}

fn call_map(
    cfg: Arc<Config>,
    url: String,
    opts: MapOptions,
) -> Pin<Box<dyn Future<Output = Result<MapResult, String>> + Send + 'static>> {
    Box::pin(async move {
        map_svc::discover(&cfg, &url, opts, None)
            .await
            .map_err(|e| e.to_string())
    })
}

fn call_query(
    cfg: Arc<Config>,
    text: String,
    pagination: Pagination,
) -> Pin<Box<dyn Future<Output = Result<QueryResult, String>> + Send + 'static>> {
    Box::pin(async move {
        query_svc::query(&cfg, &text, pagination)
            .await
            .map_err(|e| e.to_string())
    })
}

fn call_retrieve(
    cfg: Arc<Config>,
    url: String,
    opts: RetrieveOptions,
) -> Pin<Box<dyn Future<Output = Result<RetrieveResult, String>> + Send + 'static>> {
    Box::pin(async move {
        query_svc::retrieve(&cfg, &url, opts)
            .await
            .map_err(|e| e.to_string())
    })
}

fn call_ask(
    cfg: Arc<Config>,
    question: String,
) -> Pin<Box<dyn Future<Output = Result<AskResult, String>> + Send + 'static>> {
    Box::pin(async move {
        query_svc::ask(&cfg, &question, None)
            .await
            .map_err(|e| e.to_string())
    })
}

fn call_search(
    cfg: Arc<Config>,
    query: String,
    opts: SearchOptions,
) -> Pin<Box<dyn Future<Output = Result<SearchResult, String>> + Send + 'static>> {
    Box::pin(async move {
        search_svc::search(&cfg, &query, opts, None)
            .await
            .map_err(|e| e.to_string())
    })
}

fn call_research(
    cfg: Arc<Config>,
    query: String,
    opts: SearchOptions,
) -> Pin<Box<dyn Future<Output = Result<ResearchResult, String>> + Send + 'static>> {
    Box::pin(async move {
        search_svc::research(&cfg, &query, opts)
            .await
            .map_err(|e| e.to_string())
    })
}

fn call_stats(
    cfg: Arc<Config>,
) -> Pin<Box<dyn Future<Output = Result<StatsResult, String>> + Send + 'static>> {
    Box::pin(async move { system_svc::stats(&cfg).await.map_err(|e| e.to_string()) })
}

fn call_sources(
    cfg: Arc<Config>,
    pagination: Pagination,
) -> Pin<Box<dyn Future<Output = Result<SourcesResult, String>> + Send + 'static>> {
    Box::pin(async move {
        system_svc::sources(&cfg, pagination)
            .await
            .map_err(|e| e.to_string())
    })
}

fn call_domains(
    cfg: Arc<Config>,
    pagination: Pagination,
) -> Pin<Box<dyn Future<Output = Result<DomainsResult, String>> + Send + 'static>> {
    Box::pin(async move {
        system_svc::domains(&cfg, pagination)
            .await
            .map_err(|e| e.to_string())
    })
}

fn call_doctor(
    cfg: Arc<Config>,
) -> Pin<Box<dyn Future<Output = Result<DoctorResult, String>> + Send + 'static>> {
    Box::pin(async move { system_svc::doctor(&cfg).await.map_err(|e| e.to_string()) })
}

fn call_status(
    cfg: Arc<Config>,
) -> Pin<Box<dyn Future<Output = Result<StatusResult, String>> + Send + 'static>> {
    Box::pin(async move {
        system_svc::full_status(&cfg)
            .await
            .map_err(|e| e.to_string())
    })
}

/// Classify a mode string and extract all request parameters into owned values.
///
/// This is the **only** place in the direct-dispatch path where a `String` is
/// borrowed as `&str` — the call to `ServiceMode::from_str` and the
/// `extract_params` helpers.  By doing all classification here, in a plain
/// (non-async) function, no `&str` or `&ExecCommandContext` borrow ever enters
/// an async state machine, which is the root cause of Rust's HRTB `Send` false
/// positives when spawning with `tokio::task::spawn`.
///
/// Returns `None` when the mode is not a recognised `DIRECT_SYNC_MODES` entry.
pub(super) fn classify_sync_direct(
    mode: &str,
    input: &str,
    flags: &serde_json::Value,
    cfg: Arc<Config>,
    ws_ctx: &CommandContext,
) -> Option<DirectParams> {
    // Classify synchronously — borrow of `mode` is dropped after this call.
    ServiceMode::from_str(mode)?;

    // Build a synthetic context just to drive extract_params.
    let context = ExecCommandContext {
        exec_id: ws_ctx.exec_id.clone(),
        mode: mode.to_string(),
        input: input.to_string(),
        flags: flags.clone(),
        cfg,
    };
    extract_params(&context, flags)
}

/// Execute a pre-classified direct-dispatch request.
///
/// All parameters are fully owned — no `String → &str` conversion happens
/// inside this `async fn`, so the generated `Future` satisfies `Send + 'static`
/// and can be submitted to `tokio::task::spawn` without triggering Rust's HRTB
/// `Send` false positive.
pub(super) async fn handle_sync_direct(
    params: DirectParams,
    tx: mpsc::Sender<String>,
    ws_ctx: CommandContext,
) {
    let start = Instant::now();

    // dispatch_service takes full ownership — no borrows cross any .await.
    let svc_result = dispatch_service(params, tx.clone(), ws_ctx.clone()).await;
    let elapsed_ms = Some(start.elapsed().as_millis() as u64);

    match svc_result {
        Ok(()) => send_done_owned(tx, ws_ctx, 0, elapsed_ms).await,
        Err(msg) => send_error_owned(tx, ws_ctx, msg, elapsed_ms).await,
    }
}

/// Inner dispatch — all parameters are owned, resulting in a `Send + 'static` future.
///
/// Uses a `ServiceMode` enum rather than `match mode.as_str()`.  Matching on a
/// `String` via `.as_str()` creates a `&str` scrutinee borrow whose lifetime
/// spans the entire match block, including every `.await` suspension point
/// inside the arms.  The Rust async state machine includes that borrow in its
/// generated state variants, causing an HRTB `Send` diagnostic when the
/// resulting `Future` is passed to `tokio::task::spawn`.  Using a pre-classified
/// `Copy` enum eliminates the string borrow entirely.
///
/// Returns `Ok(())` on success or `Err(message)` on failure.
async fn dispatch_service(
    params: DirectParams,
    tx: mpsc::Sender<String>,
    ws_ctx: CommandContext,
) -> Result<(), String> {
    let DirectParams {
        mode,
        input,
        cfg,
        limit,
        offset,
        max_points,
        agent,
        session_id,
        model,
    } = params;

    // `mode` is a `Copy` enum — no string borrow anywhere in this async fn.
    match mode {
        ServiceMode::Scrape => {
            let result = call_scrape(cfg, input).await?;
            send_json_owned(tx.clone(), ws_ctx.clone(), result.payload).await;
            // Send the scraped markdown file (same as subprocess path).
            // Pass owned values — send_scrape_file takes ownership to avoid
            // borrows crossing .await points in the async state machine.
            files::send_scrape_file(tx, ws_ctx).await;
        }

        ServiceMode::Map => {
            let opts = MapOptions { limit, offset };
            let result = call_map(cfg, input, opts).await?;
            send_json_owned(tx, ws_ctx, result.payload).await;
        }

        ServiceMode::Query => {
            let pagination = Pagination { limit, offset };
            let result = call_query(cfg, input, pagination).await?;
            let data = json!({ "results": result.results });
            send_json_owned(tx, ws_ctx, data).await;
        }

        ServiceMode::Retrieve => {
            let opts = RetrieveOptions { max_points };
            let result = call_retrieve(cfg, input, opts).await?;
            let data = json!({ "chunks": result.chunks });
            send_json_owned(tx, ws_ctx, data).await;
        }

        ServiceMode::Ask => {
            let result = call_ask(cfg, input).await?;
            send_json_owned(tx, ws_ctx, result.payload).await;
        }

        ServiceMode::Search => {
            let opts = SearchOptions {
                limit,
                offset,
                time_range: None,
            };
            let result = call_search(cfg, input, opts).await?;
            let data = json!({ "results": result.results });
            send_json_owned(tx, ws_ctx, data).await;
        }

        ServiceMode::Research => {
            let opts = SearchOptions {
                limit,
                offset,
                time_range: None,
            };
            let result = call_research(cfg, input, opts).await?;
            send_json_owned(tx, ws_ctx, result.payload).await;
        }

        ServiceMode::Stats => {
            let result = call_stats(cfg).await?;
            send_json_owned(tx, ws_ctx, result.payload).await;
        }

        ServiceMode::Sources => {
            let pagination = Pagination { limit, offset };
            let result = call_sources(cfg, pagination).await?;
            let urls_json: Vec<serde_json::Value> = result
                .urls
                .into_iter()
                .map(|(u, c)| json!({"url": u, "chunks": c}))
                .collect();
            let data = json!({
                "count": result.count,
                "limit": result.limit,
                "offset": result.offset,
                "urls": urls_json,
            });
            send_json_owned(tx, ws_ctx, data).await;
        }

        ServiceMode::Domains => {
            let pagination = Pagination { limit, offset };
            let result = call_domains(cfg, pagination).await?;
            let domains_json: Vec<serde_json::Value> = result
                .domains
                .into_iter()
                .map(|d| json!({"domain": d.domain, "vectors": d.vectors}))
                .collect();
            let data = json!({
                "limit": result.limit,
                "offset": result.offset,
                "domains": domains_json,
            });
            send_json_owned(tx, ws_ctx, data).await;
        }

        ServiceMode::Doctor => {
            let result = call_doctor(cfg).await?;
            send_json_owned(tx, ws_ctx, result.payload).await;
        }

        ServiceMode::Status => {
            let result = call_status(cfg).await?;
            send_json_owned(tx, ws_ctx, result.payload).await;
        }

        ServiceMode::PulseChat => {
            let (event_tx, mut event_rx) = mpsc::channel::<ServiceEvent>(32);
            let adapter = resolve_acp_adapter_command(&cfg, agent)?;
            let scaffold = acp_svc::AcpClientScaffold::new(adapter);
            let req = AcpPromptTurnRequest {
                session_id,
                prompt: vec![input],
                model,
            };
            let cwd = env::current_dir().map_err(|e| e.to_string())?;
            let mut prompt_turn = tokio::spawn(async move {
                scaffold
                    .start_prompt_turn(&req, cwd, Some(event_tx))
                    .await
                    .map_err(|e| e.to_string())
            });

            loop {
                tokio::select! {
                    join_result = &mut prompt_turn => {
                        let run_result = join_result
                            .map_err(|e| format!("failed to join pulse_chat task: {e}"))?;
                        run_result?;
                        while let Ok(event) = event_rx.try_recv() {
                            match event {
                                ServiceEvent::Log { level, message } => {
                                    send_json_owned(
                                        tx.clone(),
                                        ws_ctx.clone(),
                                        json!({
                                            "type": "status",
                                            "level": level,
                                            "message": message,
                                        }),
                                    )
                                    .await;
                                }
                                ServiceEvent::AcpBridge { event } => {
                                    let payload = super::events::acp_bridge_event_payload(&event);
                                    send_json_owned(tx.clone(), ws_ctx.clone(), payload).await;
                                }
                            }
                        }
                        break;
                    }
                    maybe_event = event_rx.recv() => {
                        match maybe_event {
                            Some(ServiceEvent::Log { level, message }) => {
                                send_json_owned(
                                    tx.clone(),
                                    ws_ctx.clone(),
                                    json!({
                                        "type": "status",
                                        "level": level,
                                        "message": message,
                                    }),
                                )
                                .await;
                            }
                            Some(ServiceEvent::AcpBridge { event }) => {
                                let payload = super::events::acp_bridge_event_payload(&event);
                                send_json_owned(tx.clone(), ws_ctx.clone(), payload).await;
                            }
                            None => {
                                let run_result = (&mut prompt_turn)
                                    .await
                                    .map_err(|e| format!("failed to join pulse_chat task: {e}"))?;
                                run_result?;
                                break;
                            }
                        }
                    }
                }
            }
        }

        ServiceMode::PulseChatProbe => {
            let (event_tx, mut event_rx) = mpsc::channel::<ServiceEvent>(32);
            let adapter = resolve_acp_adapter_command(&cfg, agent)?;
            let scaffold = acp_svc::AcpClientScaffold::new(adapter);
            let req = AcpSessionProbeRequest { session_id, model };
            let cwd = env::current_dir().map_err(|e| e.to_string())?;
            let mut probe = tokio::spawn(async move {
                scaffold
                    .start_session_probe(&req, cwd, Some(event_tx))
                    .await
                    .map_err(|e| e.to_string())
            });

            loop {
                tokio::select! {
                    join_result = &mut probe => {
                        let run_result = join_result
                            .map_err(|e| format!("failed to join pulse_chat_probe task: {e}"))?;
                        run_result?;
                        while let Ok(event) = event_rx.try_recv() {
                            match event {
                                ServiceEvent::Log { level, message } => {
                                    send_json_owned(
                                        tx.clone(),
                                        ws_ctx.clone(),
                                        json!({
                                            "type": "status",
                                            "level": level,
                                            "message": message,
                                        }),
                                    )
                                    .await;
                                }
                                ServiceEvent::AcpBridge { event } => {
                                    let payload = super::events::acp_bridge_event_payload(&event);
                                    send_json_owned(tx.clone(), ws_ctx.clone(), payload).await;
                                }
                            }
                        }
                        break;
                    }
                    maybe_event = event_rx.recv() => {
                        match maybe_event {
                            Some(ServiceEvent::Log { level, message }) => {
                                send_json_owned(
                                    tx.clone(),
                                    ws_ctx.clone(),
                                    json!({
                                        "type": "status",
                                        "level": level,
                                        "message": message,
                                    }),
                                )
                                .await;
                            }
                            Some(ServiceEvent::AcpBridge { event }) => {
                                let payload = super::events::acp_bridge_event_payload(&event);
                                send_json_owned(tx.clone(), ws_ctx.clone(), payload).await;
                            }
                            None => {
                                let run_result = (&mut probe)
                                    .await
                                    .map_err(|e| format!("failed to join pulse_chat_probe task: {e}"))?;
                                run_result?;
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Subprocess-backed sync handler — used for modes not yet wired to direct dispatch.
///
/// Reads stdout/stderr from the child process and streams events to the WS
/// sender. Screenshot JSON payloads are accumulated and forwarded as
/// artifact entries after the process exits.
pub(super) async fn handle_sync_command(
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
        let mut screenshot_jsons: Vec<serde_json::Value> = Vec::new();
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
            match serde_json::from_str::<serde_json::Value>(&clean) {
                Ok(parsed) if parsed.is_object() || parsed.is_array() => {
                    saw_json_line = true;
                    if is_screenshot {
                        screenshot_jsons.push(parsed.clone());
                    }
                    send_json_owned(stdout_tx.clone(), stdout_ctx.clone(), parsed).await;
                }
                Ok(_) | Err(_) => {
                    send_command_output_line(&stdout_tx, &stdout_ctx, clean).await;
                }
            }
        }

        if !saw_json_line
            && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(stdout_accum.trim())
        {
            send_json_owned(stdout_tx, stdout_ctx, parsed).await;
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
                if context.mode == "screenshot" {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_sync_modes_not_in_async_modes() {
        use crate::crates::web::execute::constants::ASYNC_MODES;
        for mode in DIRECT_SYNC_MODES {
            assert!(
                !ASYNC_MODES.contains(mode),
                "mode '{mode}' must not be in both DIRECT_SYNC_MODES and ASYNC_MODES"
            );
        }
    }

    #[test]
    fn direct_sync_modes_all_in_allowed_modes() {
        use crate::crates::web::execute::constants::ALLOWED_MODES;
        for mode in DIRECT_SYNC_MODES {
            assert!(
                ALLOWED_MODES.contains(mode),
                "mode '{mode}' in DIRECT_SYNC_MODES must also be in ALLOWED_MODES"
            );
        }
    }

    #[test]
    fn flag_usize_returns_default_on_missing_key() {
        let flags = serde_json::json!({});
        assert_eq!(flag_usize(&flags, "limit", 10), 10);
    }

    #[test]
    fn flag_usize_returns_value_when_present() {
        let flags = serde_json::json!({"limit": 42});
        assert_eq!(flag_usize(&flags, "limit", 10), 42);
    }

    #[test]
    fn flag_opt_usize_returns_none_on_missing_key() {
        let flags = serde_json::json!({});
        assert_eq!(flag_opt_usize(&flags, "limit"), None);
    }

    #[test]
    fn flag_opt_usize_returns_some_when_present() {
        let flags = serde_json::json!({"limit": 7});
        assert_eq!(flag_opt_usize(&flags, "limit"), Some(7));
    }

    #[test]
    fn derive_cfg_applies_collection_override() {
        let base = Config::default();
        let context = ExecCommandContext {
            exec_id: "test".to_string(),
            mode: "query".to_string(),
            input: "test".to_string(),
            flags: serde_json::Value::Null,
            cfg: Arc::new(base),
        };
        let flags = serde_json::json!({"collection": "my_custom_col"});
        let cfg = derive_cfg(&context, &flags);
        assert_eq!(cfg.collection, "my_custom_col");
    }

    #[test]
    fn derive_cfg_ignores_empty_collection() {
        let base = Config::default();
        let original_collection = base.collection.clone();
        let context = ExecCommandContext {
            exec_id: "test".to_string(),
            mode: "query".to_string(),
            input: "test".to_string(),
            flags: serde_json::Value::Null,
            cfg: Arc::new(base),
        };
        let flags = serde_json::json!({"collection": ""});
        let cfg = derive_cfg(&context, &flags);
        assert_eq!(cfg.collection, original_collection);
    }

    #[test]
    fn extract_params_populates_limit_and_offset() {
        let base = Config::default();
        let context = ExecCommandContext {
            exec_id: "test".to_string(),
            mode: "query".to_string(),
            input: "rust async".to_string(),
            flags: serde_json::Value::Null,
            cfg: Arc::new(base),
        };
        let flags = serde_json::json!({"limit": 25, "offset": 5});
        let params = extract_params(&context, &flags).expect("query is a recognised mode");
        assert_eq!(params.limit, 25);
        assert_eq!(params.offset, 5);
        assert_eq!(params.input, "rust async");
        assert!(matches!(params.mode, ServiceMode::Query));
        assert_eq!(params.agent, PulseChatAgent::Claude);
        assert_eq!(params.session_id, None);
        assert_eq!(params.model, None);
    }

    #[test]
    fn extract_params_populates_session_id_for_pulse_chat() {
        let base = Config::default();
        let context = ExecCommandContext {
            exec_id: "test".to_string(),
            mode: "pulse_chat".to_string(),
            input: "hello".to_string(),
            flags: serde_json::Value::Null,
            cfg: Arc::new(base),
        };
        let flags = serde_json::json!({"session_id": "session-123"});
        let params = extract_params(&context, &flags).expect("pulse_chat is a recognised mode");
        assert_eq!(params.agent, PulseChatAgent::Claude);
        assert_eq!(params.session_id.as_deref(), Some("session-123"));
        assert_eq!(params.model, None);
    }

    #[test]
    fn extract_params_reads_codex_agent_for_pulse_chat() {
        let base = Config::default();
        let context = ExecCommandContext {
            exec_id: "test".to_string(),
            mode: "pulse_chat".to_string(),
            input: "hello".to_string(),
            flags: serde_json::Value::Null,
            cfg: Arc::new(base),
        };
        let flags = serde_json::json!({"agent": "codex"});
        let params = extract_params(&context, &flags).expect("pulse_chat is a recognised mode");
        assert_eq!(params.agent, PulseChatAgent::Codex);
        assert_eq!(params.model, None);
    }

    #[test]
    fn extract_params_reads_model_for_pulse_chat() {
        let base = Config::default();
        let context = ExecCommandContext {
            exec_id: "test".to_string(),
            mode: "pulse_chat".to_string(),
            input: "hello".to_string(),
            flags: serde_json::Value::Null,
            cfg: Arc::new(base),
        };
        let flags = serde_json::json!({"agent": "codex", "model": "o3"});
        let params = extract_params(&context, &flags).expect("pulse_chat is a recognised mode");
        assert_eq!(params.agent, PulseChatAgent::Codex);
        assert_eq!(params.model.as_deref(), Some("o3"));
    }

    #[test]
    fn extract_params_returns_none_for_unknown_mode() {
        let base = Config::default();
        let context = ExecCommandContext {
            exec_id: "test".to_string(),
            mode: "unknown_mode".to_string(),
            input: "some input".to_string(),
            flags: serde_json::Value::Null,
            cfg: Arc::new(base),
        };
        let flags = serde_json::json!({});
        assert!(extract_params(&context, &flags).is_none());
    }

    #[test]
    fn service_mode_from_str_roundtrip() {
        for mode in DIRECT_SYNC_MODES {
            assert!(
                ServiceMode::from_str(mode).is_some(),
                "ServiceMode::from_str(\"{mode}\") should return Some"
            );
        }
    }

    #[test]
    fn parse_acp_adapter_args_uses_pipe_delimiter_and_trims_segments() {
        let parsed = parse_acp_adapter_args(" --stdio | --model | gemini-3-flash-preview |  ");
        assert_eq!(parsed, vec!["--stdio", "--model", "gemini-3-flash-preview"]);
    }

    #[test]
    fn parse_acp_adapter_args_returns_empty_for_blank_input() {
        let parsed = parse_acp_adapter_args("   |   || ");
        assert!(parsed.is_empty());
    }

    #[test]
    fn resolve_acp_adapter_command_reads_required_cmd_and_optional_args() {
        let cmd = resolve_acp_adapter_command_from_values(
            Some("/usr/local/bin/acp-adapter-test"),
            Some("--stdio|--model|gpt-5-mini"),
        )
        .expect("env values should resolve");
        assert_eq!(cmd.program, "/usr/local/bin/acp-adapter-test");
        assert_eq!(cmd.args, vec!["--stdio", "--model", "gpt-5-mini"]);
        assert_eq!(cmd.cwd, None);
    }

    #[test]
    fn resolve_acp_adapter_command_requires_non_empty_cmd() {
        let err = resolve_acp_adapter_command_from_values(Some("   "), None)
            .expect_err("blank cmd should fail");
        assert!(
            err.contains("AXON_ACP_ADAPTER_CMD"),
            "error should mention missing/invalid env var: {err}"
        );
    }

    #[test]
    fn resolve_acp_adapter_command_requires_cmd_env_var() {
        let err = resolve_acp_adapter_command_from_values(None, None)
            .expect_err("missing cmd should fail");
        assert!(
            err.contains("AXON_ACP_ADAPTER_CMD"),
            "error should mention missing/invalid env var: {err}"
        );
    }
}
