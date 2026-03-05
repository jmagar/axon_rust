use crate::crates::services::events::{ServiceEvent, emit};
use crate::crates::services::types::{
    AcpAdapterCommand, AcpBridgeEvent, AcpPermissionRequestEvent, AcpPromptTurnRequest,
    AcpSessionUpdateEvent, AcpSessionUpdateKind, AcpTurnResultEvent,
};
use agent_client_protocol::{
    Agent, Client, ClientSideConnection, ContentBlock, InitializeRequest, LoadSessionRequest,
    NewSessionRequest, PromptRequest, ProtocolVersion, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, SelectedPermissionOutcome, SessionId,
    SessionNotification, SessionUpdate, StopReason,
};
use std::error::Error;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

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

    pub fn spawn_adapter(&self) -> Result<std::process::Child, Box<dyn Error>> {
        self.validate_adapter()?;
        let mut command = std::process::Command::new(&self.adapter.program);
        command.args(&self.adapter.args);
        if let Some(cwd) = &self.adapter.cwd {
            command.current_dir(cwd);
        }
        command.stdin(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());
        let child = command.spawn()?;
        Ok(child)
    }

    pub fn prepare_initialize(&self) -> Result<InitializeRequest, Box<dyn Error>> {
        self.validate_adapter()?;
        Ok(InitializeRequest::new(ProtocolVersion::LATEST))
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
        let tx_owned = tx.clone();

        let join = tokio::task::spawn_blocking(move || {
            run_prompt_turn_blocking(adapter, initialize, session_setup, req_owned, tx_owned)
        })
        .await
        .map_err(|err| format!("failed to join ACP runtime worker: {err}"))?;

        join.map_err(|err| err.to_string())?;
        Ok(())
    }
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

pub fn map_session_notification_event(notification: &SessionNotification) -> ServiceEvent {
    ServiceEvent::AcpBridge {
        event: AcpBridgeEvent::SessionUpdate(map_session_notification(notification)),
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

fn run_prompt_turn_blocking(
    adapter: AcpAdapterCommand,
    initialize: InitializeRequest,
    session_setup: AcpSessionSetupRequest,
    req: AcpPromptTurnRequest,
    tx: Option<mpsc::Sender<ServiceEvent>>,
) -> Result<(), String> {
    use futures::io::AllowStdIo;
    use futures::task::LocalSpawnExt;

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
    let stderr_thread = std::thread::spawn(move || {
        for line in BufReader::new(child_stderr).lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            emit(
                &stderr_tx,
                ServiceEvent::Log {
                    level: "warn".to_string(),
                    message: format!("ACP adapter stderr: {line}"),
                },
            );
        }
    });

    let runtime_state = Arc::new(Mutex::new(AcpRuntimeState::default()));
    let bridge = AcpBridgeClient {
        tx: tx.clone(),
        runtime_state: runtime_state.clone(),
    };

    let mut pool = futures::executor::LocalPool::new();
    let spawner = pool.spawner();
    let (conn, io_task) = ClientSideConnection::new(
        bridge,
        AllowStdIo::new(child_stdin),
        AllowStdIo::new(child_stdout),
        move |task| {
            let _ = spawner.spawn_local(task);
        },
    );

    let io_spawner = pool.spawner();
    let _ = io_spawner.spawn_local(async move {
        let _ = io_task.await;
    });

    let turn_result = pool.run_until(async move {
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

        let session_id = match session_setup {
            AcpSessionSetupRequest::New(new_session) => {
                conn.new_session(new_session)
                    .await
                    .map_err(|err| err.to_string())?
                    .session_id
            }
            AcpSessionSetupRequest::Load(load_session) => {
                let session_id = load_session.session_id.clone();
                conn.load_session(load_session)
                    .await
                    .map_err(|err| err.to_string())?;
                session_id
            }
        };

        {
            let mut state = runtime_state
                .lock()
                .map_err(|_| "ACP runtime state lock poisoned".to_string())?;
            state.session_id = Some(session_id.0.to_string());
        }

        let prompt_blocks: Vec<ContentBlock> =
            req.prompt.into_iter().map(ContentBlock::from).collect();
        let prompt_response = conn
            .prompt(PromptRequest::new(session_id.clone(), prompt_blocks))
            .await
            .map_err(|err| err.to_string())?;

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

        Ok::<(), String>(())
    });

    if child.try_wait().map_err(|err| err.to_string())?.is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
    let _ = stderr_thread.join();

    turn_result
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
        if let Some(first_option) = args.options.first() {
            return Ok(RequestPermissionResponse::new(
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                    first_option.option_id.clone(),
                )),
            ));
        }
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
