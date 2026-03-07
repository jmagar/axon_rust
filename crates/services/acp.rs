use crate::crates::services::events::{LogLevel, ServiceEvent, emit};
use crate::crates::services::types::{
    AcpAdapterCommand, AcpAvailableCommand, AcpBridgeEvent, AcpCommandsUpdate, AcpConfigOption,
    AcpConfigSelectValue, AcpMcpServerConfig, AcpModeUpdate, AcpPermissionRequestEvent,
    AcpPlanEntry, AcpPlanUpdate, AcpPromptTurnRequest, AcpSessionProbeRequest,
    AcpSessionUpdateEvent, AcpSessionUpdateKind, AcpTurnResultEvent,
};
use agent_client_protocol::{
    Agent, Client, ClientSideConnection, ContentBlock, EnvVariable, InitializeRequest,
    LoadSessionRequest, McpServer, McpServerHttp, McpServerStdio, NewSessionRequest,
    PermissionOptionKind, PromptRequest, ProtocolVersion, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, SelectedPermissionOutcome,
    SessionConfigKind, SessionConfigOption as SdkConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOptions, SessionId, SessionNotification, SessionUpdate,
    SetSessionConfigOptionRequest, StopReason, ToolCallContent,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Shared map of pending permission responses keyed by `tool_call_id`.
///
/// When a permission request arrives from the ACP agent, the bridge inserts a
/// oneshot sender here. The WS handler (or auto-approve logic) sends the chosen
/// `option_id` through it. Uses `std::sync::Mutex` because the ACP runtime
/// runs on a `current_thread` tokio runtime inside `spawn_blocking`.
pub type PermissionResponderMap = Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<String>>>>;
use tokio_util::compat::TokioAsyncReadCompatExt;
use tokio_util::compat::TokioAsyncWriteCompatExt;

const ACP_ADAPTER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// Minimal ACP client scaffold for the services layer.
///
/// This is intentionally narrow for Phase A: process lifecycle + typed event
/// mapping. The live ACP handshake/turn lifecycle is added in follow-up tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpClientScaffold {
    adapter: AcpAdapterCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcpSessionSetupRequest {
    New(NewSessionRequest),
    Load(LoadSessionRequest),
}

impl AcpClientScaffold {
    #[must_use]
    pub fn new(adapter: AcpAdapterCommand) -> Self {
        Self { adapter }
    }

    #[must_use]
    pub fn adapter(&self) -> &AcpAdapterCommand {
        &self.adapter
    }

    pub fn validate_adapter(&self) -> Result<(), Box<dyn Error>> {
        validate_adapter_command(&self.adapter)
    }

    pub fn spawn_adapter(&self) -> Result<tokio::process::Child, Box<dyn Error>> {
        self.validate_adapter()?;
        let mut command = tokio::process::Command::new(&self.adapter.program);
        command.args(&self.adapter.args);
        if let Some(cwd) = &self.adapter.cwd {
            command.current_dir(cwd);
        }
        // Clear all inherited env vars, then allowlist only what adapters need.
        // OPENAI_* vars are intentionally excluded — they point at Axon's local LLM
        // proxy, not at OpenAI. Adapters (Claude CLI, Codex) use their own OAuth /
        // stored API keys for authentication.
        // CLAUDECODE is excluded to prevent nested-session detection.
        command.env_clear();
        for key in &[
            "PATH",
            "HOME",
            "USER",
            "SHELL",
            "TERM",
            "LANG",
            "ANTHROPIC_API_KEY",
            "CLAUDE_CODE_USE_BEDROCK",
            "CLAUDE_CODE_USE_VERTEX",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_CACHE_HOME",
            // Gemini CLI auth and config
            "GEMINI_API_KEY",
            "GOOGLE_API_KEY",
            "GOOGLE_CLOUD_PROJECT",
            "GOOGLE_CLOUD_LOCATION",
            "GOOGLE_APPLICATION_CREDENTIALS",
            "GEMINI_CLI_HOME",
            "GEMINI_FORCE_FILE_STORAGE",
        ] {
            if let Ok(val) = std::env::var(key) {
                command.env(key, val);
            }
        }
        command.stdin(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());
        let child = command.spawn()?;
        Ok(child)
    }

    pub fn prepare_initialize(&self) -> Result<InitializeRequest, Box<dyn Error>> {
        self.validate_adapter()?;
        Ok(InitializeRequest::new(ProtocolVersion::LATEST).client_info(
            agent_client_protocol::Implementation::new("axon", env!("CARGO_PKG_VERSION")),
        ))
    }

    pub fn prepare_session_setup(
        &self,
        req: &AcpPromptTurnRequest,
        cwd: impl AsRef<Path>,
    ) -> Result<AcpSessionSetupRequest, Box<dyn Error>> {
        self.validate_adapter()?;
        validate_prompt_turn_request(req)?;
        build_session_setup(req.session_id.as_deref(), cwd, &req.mcp_servers)
    }

    pub fn prepare_session_probe_setup(
        &self,
        req: &AcpSessionProbeRequest,
        cwd: impl AsRef<Path>,
    ) -> Result<AcpSessionSetupRequest, Box<dyn Error>> {
        self.validate_adapter()?;
        validate_probe_request(req)?;
        build_session_setup(req.session_id.as_deref(), cwd, &[])
    }

    pub async fn start_prompt_turn(
        &self,
        req: &AcpPromptTurnRequest,
        cwd: impl AsRef<Path>,
        tx: Option<mpsc::Sender<ServiceEvent>>,
        permission_responders: PermissionResponderMap,
    ) -> Result<(), Box<dyn Error>> {
        let initialize = self.prepare_initialize()?;
        let session_setup = self.prepare_session_setup(req, cwd)?;
        emit(
            &tx,
            ServiceEvent::Log {
                level: LogLevel::Info,
                message: format!(
                    "ACP scaffold accepted prompt turn (session_id={})",
                    req.session_id.as_deref().unwrap_or("<new>")
                ),
            },
        );

        let adapter = self.adapter.clone();
        let req_owned = req.clone();

        // ACP SDK futures are !Send (uses ?Send traits), so we run on a
        // dedicated thread with its own tokio runtime + LocalSet.
        // Unlike the previous AllowStdIo approach, we use tokio::process
        // for non-blocking I/O that returns Pending instead of blocking.
        let join = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|err| format!("failed to create ACP tokio runtime: {err}"))?;
            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, async {
                match tokio::time::timeout(
                    ACP_ADAPTER_TIMEOUT,
                    run_prompt_turn(
                        adapter,
                        initialize,
                        session_setup,
                        req_owned,
                        tx,
                        permission_responders,
                    ),
                )
                .await
                {
                    Ok(result) => result,
                    Err(_) => Err("ACP adapter timed out after 5 minutes".into()),
                }
            })
        })
        .await
        .map_err(|err| format!("failed to join ACP runtime worker: {err}"))?;

        join.map_err(|err| err.to_string())?;
        Ok(())
    }

    pub async fn start_session_probe(
        &self,
        req: &AcpSessionProbeRequest,
        cwd: impl AsRef<Path>,
        tx: Option<mpsc::Sender<ServiceEvent>>,
        permission_responders: PermissionResponderMap,
    ) -> Result<(), Box<dyn Error>> {
        let initialize = self.prepare_initialize()?;
        let session_setup = self.prepare_session_probe_setup(req, cwd)?;
        emit(
            &tx,
            ServiceEvent::Log {
                level: LogLevel::Info,
                message: format!(
                    "ACP scaffold accepted session probe (session_id={})",
                    req.session_id.as_deref().unwrap_or("<new>")
                ),
            },
        );

        let adapter = self.adapter.clone();
        let req_owned = req.clone();

        let join = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|err| format!("failed to create ACP tokio runtime: {err}"))?;
            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, async {
                match tokio::time::timeout(
                    ACP_ADAPTER_TIMEOUT,
                    run_session_probe(
                        adapter,
                        initialize,
                        session_setup,
                        req_owned,
                        tx,
                        permission_responders,
                    ),
                )
                .await
                {
                    Ok(result) => result,
                    Err(_) => Err("ACP adapter timed out after 5 minutes".into()),
                }
            })
        })
        .await
        .map_err(|err| format!("failed to join ACP runtime worker: {err}"))?;

        join.map_err(|err| err.to_string())?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct CodexModelsCache {
    models: Vec<CodexCachedModel>,
}

#[derive(Debug, Deserialize)]
struct CodexCachedModel {
    slug: String,
    display_name: Option<String>,
    description: Option<String>,
}

fn is_codex_adapter(adapter: &AcpAdapterCommand) -> bool {
    Path::new(&adapter.program)
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.contains("codex"))
}

fn is_gemini_adapter(adapter: &AcpAdapterCommand) -> bool {
    Path::new(&adapter.program)
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.contains("gemini"))
}

fn normalized_requested_model(model: Option<&str>) -> Option<String> {
    model
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "default")
        .map(ToString::to_string)
}

fn validate_model_string(model: &str) -> Result<(), Box<dyn Error>> {
    if model.is_empty() {
        return Err("model string is empty".into());
    }
    // Allow only alphanumeric, hyphens, underscores, dots, forward slashes, colons, spaces
    if !model
        .chars()
        .all(|c| c.is_alphanumeric() || "-_./: ".contains(c))
    {
        return Err(format!("model string contains invalid characters: {}", model).into());
    }
    Ok(())
}

fn append_codex_model_override(
    adapter: &AcpAdapterCommand,
    requested_model: Option<&str>,
) -> Result<AcpAdapterCommand, Box<dyn Error>> {
    let Some(model) = normalized_requested_model(requested_model) else {
        return Ok(adapter.clone());
    };
    if !is_codex_adapter(adapter) {
        return Ok(adapter.clone());
    }
    validate_model_string(&model)?;
    let mut next = adapter.clone();
    next.args.push("-c".to_string());
    next.args.push(format!("model=\"{model}\""));
    Ok(next)
}

fn append_gemini_model_override(
    adapter: &AcpAdapterCommand,
    requested_model: Option<&str>,
) -> Result<AcpAdapterCommand, Box<dyn Error>> {
    let Some(model) = normalized_requested_model(requested_model) else {
        return Ok(adapter.clone());
    };
    if !is_gemini_adapter(adapter) {
        return Ok(adapter.clone());
    }
    validate_model_string(&model)?;
    let mut next = adapter.clone();
    next.args.push("--model".to_string());
    next.args.push(model);
    Ok(next)
}

fn codex_config_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".codex"))
}

async fn read_codex_default_model() -> Option<String> {
    let config_path = codex_config_dir()?.join("config.toml");
    let raw = tokio::fs::read_to_string(config_path).await.ok()?;
    raw.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with("model") {
            return None;
        }
        let (_, value) = trimmed.split_once('=')?;
        let model = value.trim().trim_matches('"');
        if model.is_empty() {
            None
        } else {
            Some(model.to_string())
        }
    })
}

fn gemini_config_dir() -> Option<PathBuf> {
    std::env::var_os("GEMINI_CLI_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".gemini")))
}

async fn read_gemini_default_model() -> Option<String> {
    let config_path = gemini_config_dir()?.join("settings.json");
    let raw = tokio::fs::read_to_string(config_path).await.ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    parsed
        .pointer("/model/name")
        .or_else(|| parsed.get("selectedModel"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

async fn read_gemini_cached_model_options(
    current_model: Option<&str>,
) -> Option<Vec<AcpConfigOption>> {
    let model = current_model
        .and_then(|v| normalized_requested_model(Some(v)))
        .or(read_gemini_default_model().await)?;
    Some(vec![AcpConfigOption {
        id: "model".to_string(),
        name: "Model".to_string(),
        description: Some("Gemini model".to_string()),
        category: Some("model".to_string()),
        current_value: model.clone(),
        options: vec![AcpConfigSelectValue {
            value: model.clone(),
            name: model,
            description: None,
        }],
    }])
}

async fn read_codex_cached_model_options(
    current_model: Option<&str>,
) -> Option<Vec<AcpConfigOption>> {
    let cache_path = codex_config_dir()?.join("models_cache.json");
    let raw = tokio::fs::read_to_string(cache_path).await.ok()?;
    let cache: CodexModelsCache = serde_json::from_str(&raw).ok()?;
    if cache.models.is_empty() {
        return None;
    }
    let options = cache
        .models
        .into_iter()
        .map(|model| AcpConfigSelectValue {
            value: model.slug.clone(),
            name: model.display_name.unwrap_or_else(|| model.slug.clone()),
            description: model.description,
        })
        .collect::<Vec<_>>();
    let selected = current_model
        .and_then(|value| normalized_requested_model(Some(value)))
        .or(read_codex_default_model().await)
        .or_else(|| options.first().map(|option| option.value.clone()))?;
    Some(vec![AcpConfigOption {
        id: "model".to_string(),
        name: "Model".to_string(),
        description: Some("Codex cached model choices".to_string()),
        category: Some("model".to_string()),
        current_value: selected,
        options,
    }])
}

pub fn validate_adapter_command(adapter: &AcpAdapterCommand) -> Result<(), Box<dyn Error>> {
    if adapter.program.trim().is_empty() {
        return Err("ACP adapter command cannot be empty".into());
    }
    Ok(())
}

pub fn validate_prompt_turn_request(req: &AcpPromptTurnRequest) -> Result<(), Box<dyn Error>> {
    if req.prompt.is_empty() {
        return Err("ACP prompt turn requires at least one prompt block".into());
    }
    if req
        .session_id
        .as_deref()
        .is_some_and(|session_id| session_id.trim().is_empty())
    {
        return Err("ACP session_id cannot be blank when provided".into());
    }
    Ok(())
}

pub fn validate_probe_request(req: &AcpSessionProbeRequest) -> Result<(), Box<dyn Error>> {
    if req
        .session_id
        .as_deref()
        .is_some_and(|session_id| session_id.trim().is_empty())
    {
        return Err("ACP session_id cannot be blank when provided".into());
    }
    Ok(())
}

pub fn validate_session_cwd(cwd: &Path) -> Result<PathBuf, Box<dyn Error>> {
    if !cwd.is_absolute() {
        return Err("ACP session cwd must be an absolute path".into());
    }
    Ok(cwd.to_path_buf())
}

fn convert_mcp_servers(configs: &[AcpMcpServerConfig]) -> Vec<McpServer> {
    configs
        .iter()
        .map(|cfg| match cfg {
            AcpMcpServerConfig::Stdio {
                name,
                command,
                args,
                env,
            } => {
                let mut server = McpServerStdio::new(name.clone(), command.clone());
                if !args.is_empty() {
                    server = server.args(args.clone());
                }
                if !env.is_empty() {
                    server = server.env(
                        env.iter()
                            .map(|(k, v)| EnvVariable::new(k.clone(), v.clone()))
                            .collect(),
                    );
                }
                McpServer::Stdio(server)
            }
            AcpMcpServerConfig::Http { name, url } => {
                McpServer::Http(McpServerHttp::new(name.clone(), url.clone()))
            }
        })
        .collect()
}

fn build_session_setup(
    session_id: Option<&str>,
    cwd: impl AsRef<Path>,
    mcp_servers: &[AcpMcpServerConfig],
) -> Result<AcpSessionSetupRequest, Box<dyn Error>> {
    let cwd = validate_session_cwd(cwd.as_ref())?;
    let sdk_mcp_servers = convert_mcp_servers(mcp_servers);
    match session_id.map(str::trim) {
        Some(sid) if !sid.is_empty() => {
            let mut req = LoadSessionRequest::new(SessionId::new(sid), cwd);
            if !sdk_mcp_servers.is_empty() {
                req = req.mcp_servers(sdk_mcp_servers);
            }
            Ok(AcpSessionSetupRequest::Load(req))
        }
        _ => {
            let mut req = NewSessionRequest::new(cwd);
            if !sdk_mcp_servers.is_empty() {
                req = req.mcp_servers(sdk_mcp_servers);
            }
            Ok(AcpSessionSetupRequest::New(req))
        }
    }
}

pub fn map_session_update_kind(update: &SessionUpdate) -> AcpSessionUpdateKind {
    match update {
        SessionUpdate::UserMessageChunk(_) => AcpSessionUpdateKind::UserDelta,
        SessionUpdate::AgentMessageChunk(_) => AcpSessionUpdateKind::AssistantDelta,
        SessionUpdate::AgentThoughtChunk(_) => AcpSessionUpdateKind::ThinkingDelta,
        SessionUpdate::ToolCall(_) => AcpSessionUpdateKind::ToolCallStarted,
        SessionUpdate::ToolCallUpdate(_) => AcpSessionUpdateKind::ToolCallUpdated,
        SessionUpdate::Plan(_) => AcpSessionUpdateKind::Plan,
        SessionUpdate::AvailableCommandsUpdate(_) => AcpSessionUpdateKind::AvailableCommandsUpdate,
        SessionUpdate::CurrentModeUpdate(_) => AcpSessionUpdateKind::CurrentModeUpdate,
        SessionUpdate::ConfigOptionUpdate(_) => AcpSessionUpdateKind::ConfigOptionUpdate,
        _ => AcpSessionUpdateKind::Unknown,
    }
}

pub fn map_session_notification(notification: &SessionNotification) -> AcpSessionUpdateEvent {
    let kind = map_session_update_kind(&notification.update);
    let text_delta = extract_text_delta(&notification.update);
    let tool_call_id = extract_tool_call_id(&notification.update);
    let (tool_name, tool_status) = extract_tool_details(&notification.update);
    let tool_content = extract_tool_content(&notification.update);
    let tool_input = extract_tool_input(&notification.update);
    AcpSessionUpdateEvent {
        session_id: notification.session_id.0.to_string(),
        kind,
        text_delta,
        tool_call_id,
        tool_name,
        tool_status,
        tool_content,
        tool_input,
    }
}

pub fn map_permission_request(req: &RequestPermissionRequest) -> AcpPermissionRequestEvent {
    let option_ids = req
        .options
        .iter()
        .map(|opt| opt.option_id.0.to_string())
        .collect::<Vec<_>>();
    AcpPermissionRequestEvent {
        session_id: req.session_id.0.to_string(),
        tool_call_id: req.tool_call.tool_call_id.0.to_string(),
        option_ids,
    }
}

pub fn map_permission_request_event(req: &RequestPermissionRequest) -> ServiceEvent {
    ServiceEvent::AcpBridge {
        event: AcpBridgeEvent::PermissionRequest(map_permission_request(req)),
    }
}

/// Convert ACP SDK config options into our service-layer representation.
pub fn map_config_options(options: &[SdkConfigOption]) -> Vec<AcpConfigOption> {
    options
        .iter()
        .filter_map(|opt| {
            let select = match &opt.kind {
                SessionConfigKind::Select(select) => select,
                _ => return None,
            };
            let values = match &select.options {
                SessionConfigSelectOptions::Ungrouped(opts) => opts
                    .iter()
                    .map(|o| AcpConfigSelectValue {
                        value: o.value.0.to_string(),
                        name: o.name.clone(),
                        description: o.description.clone(),
                    })
                    .collect(),
                SessionConfigSelectOptions::Grouped(groups) => groups
                    .iter()
                    .flat_map(|g| &g.options)
                    .map(|o| AcpConfigSelectValue {
                        value: o.value.0.to_string(),
                        name: o.name.clone(),
                        description: o.description.clone(),
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let category = opt.category.as_ref().map(|c| match c {
                SessionConfigOptionCategory::Mode => "mode".to_string(),
                SessionConfigOptionCategory::Model => "model".to_string(),
                SessionConfigOptionCategory::ThoughtLevel => "thought_level".to_string(),
                SessionConfigOptionCategory::Other(s) => s.clone(),
                _ => "other".to_string(),
            });
            Some(AcpConfigOption {
                id: opt.id.0.to_string(),
                name: opt.name.clone(),
                description: opt.description.clone(),
                category,
                current_value: select.current_value.0.to_string(),
                options: values,
            })
        })
        .collect()
}

fn select_options_contains_value(options: &SessionConfigSelectOptions, requested: &str) -> bool {
    match options {
        SessionConfigSelectOptions::Ungrouped(values) => {
            values.iter().any(|v| v.value.0.as_ref() == requested)
        }
        SessionConfigSelectOptions::Grouped(groups) => groups
            .iter()
            .flat_map(|g| g.options.iter())
            .any(|v| v.value.0.as_ref() == requested),
        _ => false,
    }
}

pub fn map_session_notification_event(notification: &SessionNotification) -> ServiceEvent {
    let sid = notification.session_id.0.to_string();
    match &notification.update {
        SessionUpdate::ConfigOptionUpdate(update) => ServiceEvent::AcpBridge {
            event: AcpBridgeEvent::ConfigOptionsUpdate(map_config_options(&update.config_options)),
        },
        SessionUpdate::Plan(plan) => ServiceEvent::AcpBridge {
            event: AcpBridgeEvent::PlanUpdate(AcpPlanUpdate {
                session_id: sid,
                entries: plan
                    .entries
                    .iter()
                    .map(|e| AcpPlanEntry {
                        content: e.content.clone(),
                        priority: format!("{:?}", e.priority).to_lowercase(),
                        status: format!("{:?}", e.status).to_lowercase(),
                    })
                    .collect(),
            }),
        },
        SessionUpdate::CurrentModeUpdate(mode) => ServiceEvent::AcpBridge {
            event: AcpBridgeEvent::ModeUpdate(AcpModeUpdate {
                session_id: sid,
                current_mode_id: mode.current_mode_id.0.to_string(),
            }),
        },
        SessionUpdate::AvailableCommandsUpdate(cmds) => ServiceEvent::AcpBridge {
            event: AcpBridgeEvent::CommandsUpdate(AcpCommandsUpdate {
                session_id: sid,
                commands: cmds
                    .available_commands
                    .iter()
                    .map(|c| AcpAvailableCommand {
                        name: c.name.clone(),
                        description: Some(c.description.clone()),
                    })
                    .collect(),
            }),
        },
        _ => ServiceEvent::AcpBridge {
            event: AcpBridgeEvent::SessionUpdate(map_session_notification(notification)),
        },
    }
}

fn extract_tool_call_id(update: &SessionUpdate) -> Option<String> {
    match update {
        SessionUpdate::ToolCall(tool_call) => Some(tool_call.tool_call_id.0.to_string()),
        SessionUpdate::ToolCallUpdate(tool_call_update) => {
            Some(tool_call_update.tool_call_id.0.to_string())
        }
        _ => None,
    }
}

fn extract_tool_details(update: &SessionUpdate) -> (Option<String>, Option<String>) {
    match update {
        SessionUpdate::ToolCall(tool_call) => (
            Some(tool_call.title.clone()),
            Some(format!("{:?}", tool_call.status)),
        ),
        SessionUpdate::ToolCallUpdate(tool_call_update) => {
            let title = tool_call_update.fields.title.clone();
            let status = tool_call_update
                .fields
                .status
                .as_ref()
                .map(|s| format!("{s:?}"));
            (title, status)
        }
        _ => (None, None),
    }
}

fn extract_text_delta(update: &SessionUpdate) -> Option<String> {
    match update {
        SessionUpdate::UserMessageChunk(chunk)
        | SessionUpdate::AgentMessageChunk(chunk)
        | SessionUpdate::AgentThoughtChunk(chunk) => extract_content_text(&chunk.content),
        _ => None,
    }
}

fn extract_content_text(content: &ContentBlock) -> Option<String> {
    match content {
        ContentBlock::Text(text) => Some(text.text.clone()),
        _ => None,
    }
}

fn extract_tool_content(update: &SessionUpdate) -> Option<String> {
    match update {
        SessionUpdate::ToolCall(tc) => tc.content.iter().find_map(|c| match c {
            ToolCallContent::Content(content) => extract_content_text(&content.content),
            _ => None,
        }),
        SessionUpdate::ToolCallUpdate(tcu) => tcu.fields.content.as_ref().and_then(|contents| {
            contents.iter().find_map(|c| match c {
                ToolCallContent::Content(content) => extract_content_text(&content.content),
                _ => None,
            })
        }),
        _ => None,
    }
}

fn extract_tool_input(update: &SessionUpdate) -> Option<serde_json::Value> {
    match update {
        SessionUpdate::ToolCall(tc) => tc.raw_input.clone(),
        SessionUpdate::ToolCallUpdate(tcu) => tcu.fields.raw_input.clone(),
        _ => None,
    }
}

async fn run_prompt_turn(
    adapter: AcpAdapterCommand,
    initialize: InitializeRequest,
    session_setup: AcpSessionSetupRequest,
    req: AcpPromptTurnRequest,
    tx: Option<mpsc::Sender<ServiceEvent>>,
    permission_responders: PermissionResponderMap,
) -> Result<(), String> {
    let adapter = append_codex_model_override(&adapter, req.model.as_deref())
        .map_err(|err| format!("invalid model override: {err}"))?;
    let adapter = append_gemini_model_override(&adapter, req.model.as_deref())
        .map_err(|err| format!("invalid model override: {err}"))?;
    let codex_adapter = is_codex_adapter(&adapter);
    let gemini_adapter = is_gemini_adapter(&adapter);
    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: "ACP runtime: spawning adapter process".to_string(),
        },
    );
    let scaffold = AcpClientScaffold::new(adapter);
    let mut child = scaffold
        .spawn_adapter()
        .map_err(|err| format!("failed to spawn ACP adapter: {err}"))?;
    let child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| "ACP adapter stdin unavailable".to_string())?;
    let child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| "ACP adapter stdout unavailable".to_string())?;
    let child_stderr = child
        .stderr
        .take()
        .ok_or_else(|| "ACP adapter stderr unavailable".to_string())?;

    let stderr_tx = tx.clone();
    tokio::task::spawn_local(async move {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let mut reader = BufReader::new(child_stderr);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    emit(
                        &stderr_tx,
                        ServiceEvent::Log {
                            level: LogLevel::Warn,
                            message: format!("ACP adapter stderr: {trimmed}"),
                        },
                    );
                }
            }
        }
    });

    let runtime_state = Arc::new(Mutex::new(AcpRuntimeState::default()));
    let auto_approve = resolve_acp_auto_approve();
    let bridge = AcpBridgeClient {
        tx: tx.clone(),
        runtime_state: runtime_state.clone(),
        auto_approve,
        permission_responders: permission_responders.clone(),
    };

    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!(
                "ACP runtime: transport ready, starting initialize (auto_approve={auto_approve})"
            ),
        },
    );

    // Use tokio_util::compat to convert tokio::io::{AsyncRead,AsyncWrite} into
    // futures::io::{AsyncRead,AsyncWrite} that the ACP SDK expects.
    // This is the critical fix: tokio process I/O returns Pending properly
    // instead of blocking the thread like AllowStdIo did.
    let compat_stdin = child_stdin.compat_write();
    let compat_stdout = child_stdout.compat();

    let (conn, io_task) =
        ClientSideConnection::new(bridge, compat_stdin, compat_stdout, move |task| {
            tokio::task::spawn_local(task);
        });

    let io_tx = tx.clone();
    tokio::task::spawn_local(async move {
        match io_task.await {
            Ok(()) => emit(
                &io_tx,
                ServiceEvent::Log {
                    level: LogLevel::Info,
                    message: "ACP runtime: IO task completed".to_string(),
                },
            ),
            Err(err) => emit(
                &io_tx,
                ServiceEvent::Log {
                    level: LogLevel::Warn,
                    message: format!("ACP runtime: IO task failed: {err}"),
                },
            ),
        }
    });

    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: "ACP runtime: sending initialize request".to_string(),
        },
    );
    let initialize_response = conn
        .initialize(initialize)
        .await
        .map_err(|err| err.to_string())?;
    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!(
                "ACP initialized with protocol {}",
                initialize_response.protocol_version
            ),
        },
    );

    let (session_id, initial_config_options) = match session_setup {
        AcpSessionSetupRequest::New(new_session) => {
            emit(
                &tx,
                ServiceEvent::Log {
                    level: LogLevel::Info,
                    message: "ACP runtime: creating new session".to_string(),
                },
            );
            let response = conn
                .new_session(new_session)
                .await
                .map_err(|err| err.to_string())?;
            (response.session_id, response.config_options)
        }
        AcpSessionSetupRequest::Load(load_session) => {
            emit(
                &tx,
                ServiceEvent::Log {
                    level: LogLevel::Info,
                    message: "ACP runtime: loading existing session".to_string(),
                },
            );
            let requested_session_id = load_session.session_id.clone();
            let fallback_cwd = load_session.cwd.clone();
            match conn.load_session(load_session).await {
                Ok(response) => (requested_session_id, response.config_options),
                Err(err) => {
                    emit(
                        &tx,
                        ServiceEvent::Log {
                            level: LogLevel::Warn,
                            message: format!(
                                "ACP load_session failed, falling back to new session: {err}"
                            ),
                        },
                    );
                    let response = conn
                        .new_session(NewSessionRequest::new(fallback_cwd))
                        .await
                        .map_err(|e| e.to_string())?;
                    (response.session_id, response.config_options)
                }
            }
        }
    };

    let mapped_initial_options = initial_config_options
        .as_ref()
        .map(|config_options| map_config_options(config_options));
    if let Some(ref mapped) = mapped_initial_options
        && !mapped.is_empty()
    {
        emit(
            &tx,
            ServiceEvent::AcpBridge {
                event: AcpBridgeEvent::ConfigOptionsUpdate(mapped.clone()),
            },
        );
    } else if codex_adapter {
        if let Some(fallback_options) = read_codex_cached_model_options(req.model.as_deref()).await
        {
            emit(
                &tx,
                ServiceEvent::AcpBridge {
                    event: AcpBridgeEvent::ConfigOptionsUpdate(fallback_options),
                },
            );
        }
    } else if gemini_adapter {
        if let Some(fallback_options) = read_gemini_cached_model_options(req.model.as_deref()).await
        {
            emit(
                &tx,
                ServiceEvent::AcpBridge {
                    event: AcpBridgeEvent::ConfigOptionsUpdate(fallback_options),
                },
            );
        }
    }

    // If the caller requested a specific model and the agent advertises a model config option,
    // apply the setting before sending the prompt turn.
    if let Some(requested_model) = normalized_requested_model(req.model.as_deref())
        && let Some(ref config_options) = initial_config_options
    {
        let model_config = config_options.iter().find(|opt| {
            opt.category
                .as_ref()
                .is_some_and(|c| matches!(c, SessionConfigOptionCategory::Model))
        });

        if let Some(model_config) = model_config {
            let value_allowed = match &model_config.kind {
                SessionConfigKind::Select(select) => {
                    select_options_contains_value(&select.options, &requested_model)
                }
                _ => false,
            };
            if value_allowed {
                emit(
                    &tx,
                    ServiceEvent::Log {
                        level: LogLevel::Info,
                        message: format!("ACP runtime: setting model to {requested_model}"),
                    },
                );
                let set_response = conn
                    .set_session_config_option(SetSessionConfigOptionRequest::new(
                        session_id.clone(),
                        model_config.id.clone(),
                        requested_model.clone(),
                    ))
                    .await
                    .map_err(|err| format!("failed to set ACP model config: {err}"))?;
                let updated = map_config_options(&set_response.config_options);
                if !updated.is_empty() {
                    emit(
                        &tx,
                        ServiceEvent::AcpBridge {
                            event: AcpBridgeEvent::ConfigOptionsUpdate(updated),
                        },
                    );
                }
            } else {
                emit(
                    &tx,
                    ServiceEvent::Log {
                        level: LogLevel::Warn,
                        message: format!(
                            "ACP runtime: skipping unsupported model value '{requested_model}'"
                        ),
                    },
                );
            }
        }
    }

    {
        let mut state = runtime_state
            .lock()
            .map_err(|_| "ACP runtime state lock poisoned".to_string())?;
        state.session_id = Some(session_id.0.to_string());
    }

    let prompt_blocks: Vec<ContentBlock> = req.prompt.into_iter().map(ContentBlock::from).collect();
    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: "ACP runtime: sending prompt turn".to_string(),
        },
    );

    // Spawn a process exit watcher so we detect adapter crashes mid-session.
    let (exit_tx, exit_rx) = tokio::sync::oneshot::channel::<String>();
    tokio::task::spawn_local(async move {
        match child.wait().await {
            Ok(status) if !status.success() => {
                let _ = exit_tx.send(format!("ACP adapter exited with {status}"));
            }
            Err(err) => {
                let _ = exit_tx.send(format!("ACP adapter wait failed: {err}"));
            }
            Ok(_) => {
                let _ = exit_tx.send(String::new());
            }
        }
    });

    // Race the prompt against process exit.
    tokio::select! {
        prompt_result = conn.prompt(PromptRequest::new(session_id.clone(), prompt_blocks)) => {
            let prompt_response = prompt_result.map_err(|err| err.to_string())?;
            let stop_reason = prompt_response.stop_reason;
            let stop_reason_str = stop_reason_to_string(stop_reason);
            let log_level = match stop_reason {
                StopReason::EndTurn => LogLevel::Info,
                StopReason::MaxTokens | StopReason::Refusal | StopReason::Cancelled => LogLevel::Warn,
                _ => LogLevel::Info,
            };
            emit(
                &tx,
                ServiceEvent::Log {
                    level: log_level,
                    message: format!("ACP runtime: prompt turn completed (stop_reason={stop_reason_str})"),
                },
            );

            let (assistant_text, settled_session_id) = {
                let state = runtime_state
                    .lock()
                    .map_err(|_| "ACP runtime state lock poisoned".to_string())?;
                let session = state
                    .session_id
                    .clone()
                    .unwrap_or_else(|| session_id.0.to_string());
                (state.assistant_text.clone(), session)
            };

            emit(
                &tx,
                ServiceEvent::AcpBridge {
                    event: AcpBridgeEvent::TurnResult(AcpTurnResultEvent {
                        session_id: settled_session_id,
                        stop_reason: stop_reason_str,
                        result: assistant_text,
                    }),
                },
            );
        }
        exit_msg = exit_rx => {
            let msg = exit_msg.unwrap_or_else(|_| "exit channel dropped".to_string());
            if !msg.is_empty() {
                return Err(format!("ACP adapter crashed mid-session: {msg}"));
            }
            return Err("ACP adapter exited before prompt completed".to_string());
        }
    }

    Ok(())
}

async fn run_session_probe(
    adapter: AcpAdapterCommand,
    initialize: InitializeRequest,
    session_setup: AcpSessionSetupRequest,
    req: AcpSessionProbeRequest,
    tx: Option<mpsc::Sender<ServiceEvent>>,
    permission_responders: PermissionResponderMap,
) -> Result<(), String> {
    let adapter = append_codex_model_override(&adapter, req.model.as_deref())
        .map_err(|err| format!("invalid model override: {err}"))?;
    let adapter = append_gemini_model_override(&adapter, req.model.as_deref())
        .map_err(|err| format!("invalid model override: {err}"))?;
    let codex_adapter = is_codex_adapter(&adapter);
    let gemini_adapter = is_gemini_adapter(&adapter);
    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: "ACP runtime: spawning adapter process".to_string(),
        },
    );
    let scaffold = AcpClientScaffold::new(adapter);
    let mut child = scaffold
        .spawn_adapter()
        .map_err(|err| format!("failed to spawn ACP adapter: {err}"))?;
    let child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| "ACP adapter stdin unavailable".to_string())?;
    let child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| "ACP adapter stdout unavailable".to_string())?;
    let child_stderr = child
        .stderr
        .take()
        .ok_or_else(|| "ACP adapter stderr unavailable".to_string())?;

    let stderr_tx = tx.clone();
    tokio::task::spawn_local(async move {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let mut reader = BufReader::new(child_stderr);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    emit(
                        &stderr_tx,
                        ServiceEvent::Log {
                            level: LogLevel::Warn,
                            message: format!("ACP adapter stderr: {trimmed}"),
                        },
                    );
                }
            }
        }
    });

    // Spawn a process exit watcher so we detect adapter crashes mid-probe.
    let (exit_tx, exit_rx) = tokio::sync::oneshot::channel::<String>();
    tokio::task::spawn_local(async move {
        match child.wait().await {
            Ok(status) if !status.success() => {
                let _ = exit_tx.send(format!("ACP adapter exited with {status}"));
            }
            Err(err) => {
                let _ = exit_tx.send(format!("ACP adapter wait failed: {err}"));
            }
            Ok(_) => {
                let _ = exit_tx.send(String::new());
            }
        }
    });

    let runtime_state = Arc::new(Mutex::new(AcpRuntimeState::default()));
    let auto_approve = resolve_acp_auto_approve();
    let bridge = AcpBridgeClient {
        tx: tx.clone(),
        runtime_state: runtime_state.clone(),
        auto_approve,
        permission_responders: permission_responders.clone(),
    };

    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!(
                "ACP runtime: transport ready, starting initialize (auto_approve={auto_approve})"
            ),
        },
    );

    let compat_stdin = child_stdin.compat_write();
    let compat_stdout = child_stdout.compat();

    let (conn, io_task) =
        ClientSideConnection::new(bridge, compat_stdin, compat_stdout, move |task| {
            tokio::task::spawn_local(task);
        });

    let io_tx = tx.clone();
    tokio::task::spawn_local(async move {
        match io_task.await {
            Ok(()) => emit(
                &io_tx,
                ServiceEvent::Log {
                    level: LogLevel::Info,
                    message: "ACP runtime: IO task completed".to_string(),
                },
            ),
            Err(err) => emit(
                &io_tx,
                ServiceEvent::Log {
                    level: LogLevel::Warn,
                    message: format!("ACP runtime: IO task failed: {err}"),
                },
            ),
        }
    });

    // Run probe setup, racing against adapter process exit.
    let probe_work = async {
        emit(
            &tx,
            ServiceEvent::Log {
                level: LogLevel::Info,
                message: "ACP runtime: sending initialize request".to_string(),
            },
        );
        let initialize_response = conn
            .initialize(initialize)
            .await
            .map_err(|err| err.to_string())?;
        emit(
            &tx,
            ServiceEvent::Log {
                level: LogLevel::Info,
                message: format!(
                    "ACP initialized with protocol {}",
                    initialize_response.protocol_version
                ),
            },
        );

        let (session_id, initial_config_options) = match session_setup {
            AcpSessionSetupRequest::New(new_session) => {
                emit(
                    &tx,
                    ServiceEvent::Log {
                        level: LogLevel::Info,
                        message: "ACP runtime: creating new session".to_string(),
                    },
                );
                let response = conn
                    .new_session(new_session)
                    .await
                    .map_err(|err| err.to_string())?;
                (response.session_id, response.config_options)
            }
            AcpSessionSetupRequest::Load(load_session) => {
                emit(
                    &tx,
                    ServiceEvent::Log {
                        level: LogLevel::Info,
                        message: "ACP runtime: loading existing session".to_string(),
                    },
                );
                let requested_session_id = load_session.session_id.clone();
                let fallback_cwd = load_session.cwd.clone();
                match conn.load_session(load_session).await {
                    Ok(response) => (requested_session_id, response.config_options),
                    Err(err) => {
                        emit(
                            &tx,
                            ServiceEvent::Log {
                                level: LogLevel::Warn,
                                message: format!(
                                    "ACP load_session failed, falling back to new session: {err}"
                                ),
                            },
                        );
                        let response = conn
                            .new_session(NewSessionRequest::new(fallback_cwd))
                            .await
                            .map_err(|e| e.to_string())?;
                        (response.session_id, response.config_options)
                    }
                }
            }
        };

        let mapped_initial_options = initial_config_options
            .as_ref()
            .map(|config_options| map_config_options(config_options));
        if let Some(ref mapped) = mapped_initial_options
            && !mapped.is_empty()
        {
            emit(
                &tx,
                ServiceEvent::AcpBridge {
                    event: AcpBridgeEvent::ConfigOptionsUpdate(mapped.clone()),
                },
            );
        } else if codex_adapter {
            if let Some(fallback_options) =
                read_codex_cached_model_options(req.model.as_deref()).await
            {
                emit(
                    &tx,
                    ServiceEvent::AcpBridge {
                        event: AcpBridgeEvent::ConfigOptionsUpdate(fallback_options),
                    },
                );
            }
        } else if gemini_adapter {
            if let Some(fallback_options) =
                read_gemini_cached_model_options(req.model.as_deref()).await
            {
                emit(
                    &tx,
                    ServiceEvent::AcpBridge {
                        event: AcpBridgeEvent::ConfigOptionsUpdate(fallback_options),
                    },
                );
            }
        }

        if let Some(requested_model) = normalized_requested_model(req.model.as_deref())
            && let Some(ref config_options) = initial_config_options
        {
            let model_config = config_options.iter().find(|opt| {
                opt.category
                    .as_ref()
                    .is_some_and(|c| matches!(c, SessionConfigOptionCategory::Model))
            });

            if let Some(model_config) = model_config {
                let value_allowed = match &model_config.kind {
                    SessionConfigKind::Select(select) => {
                        select_options_contains_value(&select.options, &requested_model)
                    }
                    _ => false,
                };
                if value_allowed {
                    let set_response = conn
                        .set_session_config_option(SetSessionConfigOptionRequest::new(
                            session_id.clone(),
                            model_config.id.clone(),
                            requested_model,
                        ))
                        .await
                        .map_err(|err| format!("failed to set ACP model config: {err}"))?;
                    let updated = map_config_options(&set_response.config_options);
                    if !updated.is_empty() {
                        emit(
                            &tx,
                            ServiceEvent::AcpBridge {
                                event: AcpBridgeEvent::ConfigOptionsUpdate(updated),
                            },
                        );
                    }
                }
            }
        }

        Ok::<(), String>(())
    };

    tokio::select! {
        result = probe_work => { result?; }
        exit_msg = exit_rx => {
            let msg = exit_msg.unwrap_or_else(|_| "exit channel dropped".to_string());
            if !msg.is_empty() {
                return Err(format!("ACP adapter crashed mid-probe: {msg}"));
            }
            return Err("ACP adapter exited before probe completed".to_string());
        }
    }

    Ok(())
}

#[derive(Debug, Default)]
struct AcpRuntimeState {
    session_id: Option<String>,
    assistant_text: String,
}

/// Resolve whether ACP permissions should be auto-approved.
///
/// Returns `true` (auto-approve) unless `AXON_ACP_AUTO_APPROVE` is explicitly
/// set to `"false"`. Default is `true` for containerized deployments.
fn resolve_acp_auto_approve() -> bool {
    std::env::var("AXON_ACP_AUTO_APPROVE")
        .map(|v| v != "false")
        .unwrap_or(true)
}

/// Select the best auto-approve outcome from the permission request options.
fn auto_approve_outcome(
    args: &RequestPermissionRequest,
    tx: &Option<mpsc::Sender<ServiceEvent>>,
    tool_call_id: &str,
) -> RequestPermissionOutcome {
    let outcome = args
        .options
        .iter()
        .find(|opt| matches!(opt.kind, PermissionOptionKind::AllowAlways))
        .or_else(|| {
            args.options
                .iter()
                .find(|opt| matches!(opt.kind, PermissionOptionKind::AllowOnce))
        })
        .map(|opt| {
            RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                opt.option_id.clone(),
            ))
        })
        .unwrap_or(RequestPermissionOutcome::Cancelled);

    emit(
        tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!("ACP permission auto-approved for tool_call={tool_call_id}"),
        },
    );

    outcome
}

#[derive(Clone)]
struct AcpBridgeClient {
    tx: Option<mpsc::Sender<ServiceEvent>>,
    runtime_state: Arc<Mutex<AcpRuntimeState>>,
    /// When true, permissions are auto-approved without waiting for frontend.
    auto_approve: bool,
    /// Pending permission response channels keyed by tool_call_id.
    permission_responders: PermissionResponderMap,
}

#[async_trait::async_trait(?Send)]
impl Client for AcpBridgeClient {
    async fn request_permission(
        &self,
        args: RequestPermissionRequest,
    ) -> agent_client_protocol::Result<RequestPermissionResponse> {
        emit(&self.tx, map_permission_request_event(&args));

        let tool_call_id = args.tool_call.tool_call_id.0.to_string();

        if self.auto_approve {
            return Ok(RequestPermissionResponse::new(auto_approve_outcome(
                &args,
                &self.tx,
                &tool_call_id,
            )));
        }

        // Interactive mode: wait for the frontend to send a permission response.
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel::<String>();
        {
            let mut map = self.permission_responders.lock().map_err(|_| {
                agent_client_protocol::Error::internal_error()
                    .data("permission responder lock poisoned")
            })?;
            map.insert(tool_call_id.clone(), resp_tx);
        }

        emit(
            &self.tx,
            ServiceEvent::Log {
                level: LogLevel::Info,
                message: format!(
                    "ACP permission awaiting frontend response for tool_call={tool_call_id}"
                ),
            },
        );

        // Wait up to 60s for a response from the frontend.
        let outcome = match tokio::time::timeout(std::time::Duration::from_secs(60), resp_rx).await
        {
            Ok(Ok(option_id)) => {
                // Validate that the chosen option_id exists in the request.
                let matched = args
                    .options
                    .iter()
                    .find(|opt| *opt.option_id.0 == *option_id);
                match matched {
                    Some(opt) => {
                        emit(
                            &self.tx,
                            ServiceEvent::Log {
                                level: LogLevel::Info,
                                message: format!(
                                    "ACP permission resolved by frontend for tool_call={tool_call_id}: {option_id}"
                                ),
                            },
                        );
                        RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                            opt.option_id.clone(),
                        ))
                    }
                    None => {
                        emit(
                            &self.tx,
                            ServiceEvent::Log {
                                level: LogLevel::Warn,
                                message: format!(
                                    "ACP permission: frontend sent unknown option_id={option_id} for tool_call={tool_call_id}, cancelling"
                                ),
                            },
                        );
                        RequestPermissionOutcome::Cancelled
                    }
                }
            }
            Ok(Err(_)) => {
                // Sender dropped (WS disconnected) — fall back to auto-approve.
                emit(
                    &self.tx,
                    ServiceEvent::Log {
                        level: LogLevel::Warn,
                        message: format!(
                            "ACP permission: responder dropped for tool_call={tool_call_id}, falling back to auto-approve"
                        ),
                    },
                );
                auto_approve_outcome(&args, &self.tx, &tool_call_id)
            }
            Err(_) => {
                // Timeout — fall back to auto-approve so the session doesn't hang.
                emit(
                    &self.tx,
                    ServiceEvent::Log {
                        level: LogLevel::Warn,
                        message: format!(
                            "ACP permission: timeout waiting for frontend response for tool_call={tool_call_id}, falling back to auto-approve"
                        ),
                    },
                );
                // Clean up the map entry.
                if let Ok(mut map) = self.permission_responders.lock() {
                    map.remove(&tool_call_id);
                }
                auto_approve_outcome(&args, &self.tx, &tool_call_id)
            }
        };

        Ok(RequestPermissionResponse::new(outcome))
    }

    async fn session_notification(
        &self,
        args: SessionNotification,
    ) -> agent_client_protocol::Result<()> {
        {
            let mut state = self.runtime_state.lock().map_err(|_| {
                agent_client_protocol::Error::internal_error()
                    .data("ACP runtime state lock poisoned")
            })?;
            if let Some(text_delta) = extract_text_delta(&args.update)
                && matches!(
                    map_session_update_kind(&args.update),
                    AcpSessionUpdateKind::AssistantDelta
                )
            {
                state.assistant_text.push_str(&text_delta);
            }
            if state.session_id.is_none() {
                state.session_id = Some(args.session_id.0.to_string());
            }
        }

        emit(&self.tx, map_session_notification_event(&args));
        Ok(())
    }
}

fn stop_reason_to_string(reason: StopReason) -> String {
    match reason {
        StopReason::EndTurn => "end_turn",
        StopReason::MaxTokens => "max_tokens",
        StopReason::MaxTurnRequests => "max_turn_requests",
        StopReason::Refusal => "refusal",
        StopReason::Cancelled => "cancelled",
        _ => "unknown",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_adapter(program: &str) -> AcpAdapterCommand {
        AcpAdapterCommand {
            program: program.to_string(),
            args: vec!["--experimental-acp".to_string()],
            cwd: None,
        }
    }

    #[test]
    fn is_gemini_adapter_positive() {
        assert!(is_gemini_adapter(&make_adapter("gemini")));
        assert!(is_gemini_adapter(&make_adapter("/usr/local/bin/gemini")));
    }

    #[test]
    fn is_gemini_adapter_negative() {
        assert!(!is_gemini_adapter(&make_adapter("claude")));
        assert!(!is_gemini_adapter(&make_adapter("codex")));
    }

    #[test]
    fn append_gemini_model_override_adds_flag() {
        let adapter = make_adapter("gemini");
        let result = append_gemini_model_override(&adapter, Some("gemini-3-pro-preview")).unwrap();
        assert_eq!(
            result.args,
            vec!["--experimental-acp", "--model", "gemini-3-pro-preview"]
        );
    }

    #[test]
    fn append_gemini_model_override_skips_non_gemini() {
        let adapter = make_adapter("claude");
        let result = append_gemini_model_override(&adapter, Some("gemini-3-pro-preview")).unwrap();
        assert_eq!(result.args, vec!["--experimental-acp"]);
    }

    #[test]
    fn append_gemini_model_override_skips_default() {
        let adapter = make_adapter("gemini");
        let result = append_gemini_model_override(&adapter, Some("default")).unwrap();
        assert_eq!(result.args, vec!["--experimental-acp"]);
    }

    #[test]
    fn append_gemini_model_override_rejects_invalid_chars() {
        let adapter = make_adapter("gemini");
        let result = append_gemini_model_override(&adapter, Some("model;rm -rf"));
        assert!(result.is_err());
    }
}
