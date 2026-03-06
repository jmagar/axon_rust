use crate::crates::services::events::{ServiceEvent, emit};
use crate::crates::services::types::{
    AcpAdapterCommand, AcpBridgeEvent, AcpConfigOption, AcpConfigSelectValue,
    AcpPermissionRequestEvent, AcpPromptTurnRequest, AcpSessionProbeRequest, AcpSessionUpdateEvent,
    AcpSessionUpdateKind, AcpTurnResultEvent,
};
use agent_client_protocol::{
    Agent, Client, ClientSideConnection, ContentBlock, InitializeRequest, LoadSessionRequest,
    NewSessionRequest, PromptRequest, ProtocolVersion, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, SessionConfigKind,
    SessionConfigOption as SdkConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOptions, SessionId, SessionNotification, SessionUpdate,
    SetSessionConfigOptionRequest, StopReason,
};
use serde::Deserialize;
use std::error::Error;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio_util::compat::TokioAsyncReadCompatExt;
use tokio_util::compat::TokioAsyncWriteCompatExt;

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
        // ACP adapters manage their own LLM auth (OAuth, API keys).
        // Remove Axon's LLM proxy vars so they don't override the adapter's config.
        command.env_remove("OPENAI_BASE_URL");
        command.env_remove("OPENAI_API_KEY");
        command.env_remove("OPENAI_MODEL");
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
        let cwd = validate_session_cwd(cwd.as_ref())?;
        match req.session_id.as_deref().map(str::trim) {
            Some(session_id) if !session_id.is_empty() => Ok(AcpSessionSetupRequest::Load(
                LoadSessionRequest::new(SessionId::new(session_id), cwd),
            )),
            _ => Ok(AcpSessionSetupRequest::New(NewSessionRequest::new(cwd))),
        }
    }

    pub fn prepare_session_probe_setup(
        &self,
        req: &AcpSessionProbeRequest,
        cwd: impl AsRef<Path>,
    ) -> Result<AcpSessionSetupRequest, Box<dyn Error>> {
        self.validate_adapter()?;
        validate_probe_request(req)?;
        let cwd = validate_session_cwd(cwd.as_ref())?;
        match req.session_id.as_deref().map(str::trim) {
            Some(session_id) if !session_id.is_empty() => Ok(AcpSessionSetupRequest::Load(
                LoadSessionRequest::new(SessionId::new(session_id), cwd),
            )),
            _ => Ok(AcpSessionSetupRequest::New(NewSessionRequest::new(cwd))),
        }
    }

    pub async fn start_prompt_turn(
        &self,
        req: &AcpPromptTurnRequest,
        cwd: impl AsRef<Path>,
        tx: Option<mpsc::Sender<ServiceEvent>>,
    ) -> Result<(), Box<dyn Error>> {
        let initialize = self.prepare_initialize()?;
        let session_setup = self.prepare_session_setup(req, cwd)?;
        emit(
            &tx,
            ServiceEvent::Log {
                level: "info".to_string(),
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
            local.block_on(
                &rt,
                run_prompt_turn(adapter, initialize, session_setup, req_owned, tx),
            )
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
    ) -> Result<(), Box<dyn Error>> {
        let initialize = self.prepare_initialize()?;
        let session_setup = self.prepare_session_probe_setup(req, cwd)?;
        emit(
            &tx,
            ServiceEvent::Log {
                level: "info".to_string(),
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
            local.block_on(
                &rt,
                run_session_probe(adapter, initialize, session_setup, req_owned, tx),
            )
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

fn normalized_requested_model(model: Option<&str>) -> Option<String> {
    model
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "default")
        .map(ToString::to_string)
}

fn append_codex_model_override(
    adapter: &AcpAdapterCommand,
    requested_model: Option<&str>,
) -> AcpAdapterCommand {
    let Some(model) = normalized_requested_model(requested_model) else {
        return adapter.clone();
    };
    if !is_codex_adapter(adapter) {
        return adapter.clone();
    }
    let mut next = adapter.clone();
    next.args.push("-c".to_string());
    next.args.push(format!("model=\"{model}\""));
    next
}

fn codex_config_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".codex"))
}

fn read_codex_default_model() -> Option<String> {
    let config_path = codex_config_dir()?.join("config.toml");
    let raw = std::fs::read_to_string(config_path).ok()?;
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

fn read_codex_cached_model_options(current_model: Option<&str>) -> Option<Vec<AcpConfigOption>> {
    let cache_path = codex_config_dir()?.join("models_cache.json");
    let raw = std::fs::read_to_string(cache_path).ok()?;
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
        .or_else(read_codex_default_model)
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
    AcpSessionUpdateEvent {
        session_id: notification.session_id.0.to_string(),
        kind,
        text_delta,
        tool_call_id,
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
    match &notification.update {
        SessionUpdate::ConfigOptionUpdate(update) => ServiceEvent::AcpBridge {
            event: AcpBridgeEvent::ConfigOptionsUpdate(map_config_options(&update.config_options)),
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

async fn run_prompt_turn(
    adapter: AcpAdapterCommand,
    initialize: InitializeRequest,
    session_setup: AcpSessionSetupRequest,
    req: AcpPromptTurnRequest,
    tx: Option<mpsc::Sender<ServiceEvent>>,
) -> Result<(), String> {
    let adapter = append_codex_model_override(&adapter, req.model.as_deref());
    let codex_adapter = is_codex_adapter(&adapter);
    emit(
        &tx,
        ServiceEvent::Log {
            level: "info".to_string(),
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
                            level: "warn".to_string(),
                            message: format!("ACP adapter stderr: {trimmed}"),
                        },
                    );
                }
            }
        }
    });

    let runtime_state = Arc::new(Mutex::new(AcpRuntimeState::default()));
    let bridge = AcpBridgeClient {
        tx: tx.clone(),
        runtime_state: runtime_state.clone(),
    };

    emit(
        &tx,
        ServiceEvent::Log {
            level: "info".to_string(),
            message: "ACP runtime: transport ready, starting initialize".to_string(),
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
                    level: "info".to_string(),
                    message: "ACP runtime: IO task completed".to_string(),
                },
            ),
            Err(err) => emit(
                &io_tx,
                ServiceEvent::Log {
                    level: "warn".to_string(),
                    message: format!("ACP runtime: IO task failed: {err}"),
                },
            ),
        }
    });

    emit(
        &tx,
        ServiceEvent::Log {
            level: "info".to_string(),
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
            level: "info".to_string(),
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
                    level: "info".to_string(),
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
                    level: "info".to_string(),
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
                            level: "warn".to_string(),
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
    } else if codex_adapter
        && let Some(fallback_options) = read_codex_cached_model_options(req.model.as_deref())
    {
        emit(
            &tx,
            ServiceEvent::AcpBridge {
                event: AcpBridgeEvent::ConfigOptionsUpdate(fallback_options),
            },
        );
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
                        level: "info".to_string(),
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
                        level: "warn".to_string(),
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
            level: "info".to_string(),
            message: "ACP runtime: sending prompt turn".to_string(),
        },
    );
    let prompt_response = conn
        .prompt(PromptRequest::new(session_id.clone(), prompt_blocks))
        .await
        .map_err(|err| err.to_string())?;
    emit(
        &tx,
        ServiceEvent::Log {
            level: "info".to_string(),
            message: "ACP runtime: prompt turn completed".to_string(),
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

    let result_json = serde_json::json!({
        "text": assistant_text,
        "operations": [],
    });

    emit(
        &tx,
        ServiceEvent::AcpBridge {
            event: AcpBridgeEvent::TurnResult(AcpTurnResultEvent {
                session_id: settled_session_id,
                stop_reason: stop_reason_to_string(prompt_response.stop_reason),
                result: result_json.to_string(),
            }),
        },
    );

    let turn_result = Ok::<(), String>(());

    let _ = child.kill().await;
    let _ = child.wait().await;

    turn_result
}

async fn run_session_probe(
    adapter: AcpAdapterCommand,
    initialize: InitializeRequest,
    session_setup: AcpSessionSetupRequest,
    req: AcpSessionProbeRequest,
    tx: Option<mpsc::Sender<ServiceEvent>>,
) -> Result<(), String> {
    let adapter = append_codex_model_override(&adapter, req.model.as_deref());
    let codex_adapter = is_codex_adapter(&adapter);
    emit(
        &tx,
        ServiceEvent::Log {
            level: "info".to_string(),
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
                            level: "warn".to_string(),
                            message: format!("ACP adapter stderr: {trimmed}"),
                        },
                    );
                }
            }
        }
    });

    let runtime_state = Arc::new(Mutex::new(AcpRuntimeState::default()));
    let bridge = AcpBridgeClient {
        tx: tx.clone(),
        runtime_state: runtime_state.clone(),
    };

    emit(
        &tx,
        ServiceEvent::Log {
            level: "info".to_string(),
            message: "ACP runtime: transport ready, starting initialize".to_string(),
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
                    level: "info".to_string(),
                    message: "ACP runtime: IO task completed".to_string(),
                },
            ),
            Err(err) => emit(
                &io_tx,
                ServiceEvent::Log {
                    level: "warn".to_string(),
                    message: format!("ACP runtime: IO task failed: {err}"),
                },
            ),
        }
    });

    emit(
        &tx,
        ServiceEvent::Log {
            level: "info".to_string(),
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
            level: "info".to_string(),
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
                    level: "info".to_string(),
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
                    level: "info".to_string(),
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
                            level: "warn".to_string(),
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
    } else if codex_adapter
        && let Some(fallback_options) = read_codex_cached_model_options(req.model.as_deref())
    {
        emit(
            &tx,
            ServiceEvent::AcpBridge {
                event: AcpBridgeEvent::ConfigOptionsUpdate(fallback_options),
            },
        );
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

    let _ = child.kill().await;
    let _ = child.wait().await;

    Ok(())
}

#[derive(Debug, Default)]
struct AcpRuntimeState {
    session_id: Option<String>,
    assistant_text: String,
}

#[derive(Clone)]
struct AcpBridgeClient {
    tx: Option<mpsc::Sender<ServiceEvent>>,
    runtime_state: Arc<Mutex<AcpRuntimeState>>,
}

#[async_trait::async_trait(?Send)]
impl Client for AcpBridgeClient {
    async fn request_permission(
        &self,
        args: RequestPermissionRequest,
    ) -> agent_client_protocol::Result<RequestPermissionResponse> {
        emit(&self.tx, map_permission_request_event(&args));
        // Default-deny: emit the event for UI visibility but do not auto-approve.
        // Interactive approval must be wired before granting permission.
        Ok(RequestPermissionResponse::new(
            RequestPermissionOutcome::Cancelled,
        ))
    }

    async fn session_notification(
        &self,
        args: SessionNotification,
    ) -> agent_client_protocol::Result<()> {
        if let Some(text_delta) = extract_text_delta(&args.update)
            && matches!(
                map_session_update_kind(&args.update),
                AcpSessionUpdateKind::AssistantDelta
            )
        {
            let mut state = self.runtime_state.lock().map_err(|_| {
                agent_client_protocol::Error::internal_error()
                    .data("ACP runtime state lock poisoned")
            })?;
            state.assistant_text.push_str(&text_delta);
        }

        {
            let mut state = self.runtime_state.lock().map_err(|_| {
                agent_client_protocol::Error::internal_error()
                    .data("ACP runtime state lock poisoned")
            })?;
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
