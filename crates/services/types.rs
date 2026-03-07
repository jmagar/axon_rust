#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pagination {
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetrieveOptions {
    pub max_points: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceTimeRange {
    Day,
    Week,
    Month,
    Year,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchOptions {
    pub limit: usize,
    pub offset: usize,
    pub time_range: Option<ServiceTimeRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapOptions {
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcesResult {
    pub count: usize,
    pub limit: usize,
    pub offset: usize,
    /// Indexed URLs paired with their chunk counts.
    pub urls: Vec<(String, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainFacet {
    pub domain: String,
    pub vectors: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainsResult {
    pub domains: Vec<DomainFacet>,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatsResult {
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DoctorResult {
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatusResult {
    pub payload: serde_json::Value,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DedupeResult {
    pub completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpAdapterCommand {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpPromptTurnRequest {
    pub session_id: Option<String>,
    pub prompt: Vec<String>,
    /// Model config option value to set after session setup (if agent supports it).
    pub model: Option<String>,
    /// MCP servers to pass through to the ACP agent session.
    pub mcp_servers: Vec<AcpMcpServerConfig>,
}

/// MCP server configuration passed through to ACP NewSessionRequest.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum AcpMcpServerConfig {
    Stdio {
        name: String,
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: Vec<(String, String)>,
    },
    Http {
        name: String,
        url: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpSessionProbeRequest {
    pub session_id: Option<String>,
    /// Optional model config option value to apply during probe.
    pub model: Option<String>,
}

/// A single selectable value within an ACP config option.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpConfigSelectValue {
    pub value: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// An ACP session config option (model selector, mode selector, etc.).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpConfigOption {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    pub current_value: String,
    pub options: Vec<AcpConfigSelectValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum AcpSessionUpdateKind {
    #[serde(rename = "user_delta")]
    UserDelta,
    #[serde(rename = "assistant_delta")]
    AssistantDelta,
    #[serde(rename = "thinking_content")]
    ThinkingDelta,
    #[serde(rename = "tool_use")]
    ToolCallStarted,
    #[serde(rename = "tool_use_update")]
    ToolCallUpdated,
    #[serde(rename = "status")]
    Plan,
    #[serde(rename = "status")]
    AvailableCommandsUpdate,
    #[serde(rename = "status")]
    CurrentModeUpdate,
    #[serde(rename = "status")]
    ConfigOptionUpdate,
    #[serde(rename = "status")]
    Unknown,
}

impl std::fmt::Display for AcpSessionUpdateKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserDelta => write!(f, "user_delta"),
            Self::AssistantDelta => write!(f, "assistant_delta"),
            Self::ThinkingDelta => write!(f, "thinking_content"),
            Self::ToolCallStarted => write!(f, "tool_use"),
            Self::ToolCallUpdated => write!(f, "tool_use_update"),
            Self::Plan => write!(f, "status"),
            Self::AvailableCommandsUpdate => write!(f, "status"),
            Self::CurrentModeUpdate => write!(f, "status"),
            Self::ConfigOptionUpdate => write!(f, "status"),
            Self::Unknown => write!(f, "status"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AcpSessionUpdateEvent {
    pub session_id: String,
    pub kind: AcpSessionUpdateKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_delta: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_status: Option<String>,
    /// Text content produced by the tool call (extracted from ToolCallContent::Content).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_content: Option<String>,
    /// Raw JSON input to the tool call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AcpPermissionRequestEvent {
    pub session_id: String,
    pub tool_call_id: String,
    /// Serialized as `"options"` on the wire to match existing frontend expectations.
    #[serde(rename = "options")]
    pub option_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AcpTurnResultEvent {
    pub session_id: String,
    pub stop_reason: String,
    pub result: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcpBridgeEvent {
    SessionUpdate(AcpSessionUpdateEvent),
    PermissionRequest(AcpPermissionRequestEvent),
    TurnResult(AcpTurnResultEvent),
    ConfigOptionsUpdate(Vec<AcpConfigOption>),
    PlanUpdate(AcpPlanUpdate),
    ModeUpdate(AcpModeUpdate),
    CommandsUpdate(AcpCommandsUpdate),
}

/// ACP plan forwarded from the agent's Plan session update.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpPlanEntry {
    pub content: String,
    pub priority: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpPlanUpdate {
    pub session_id: String,
    pub entries: Vec<AcpPlanEntry>,
}

/// Current mode changed notification from the ACP agent.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpModeUpdate {
    pub session_id: String,
    pub current_mode_id: String,
}

/// Available slash-commands changed notification from the ACP agent.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpAvailableCommand {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpCommandsUpdate {
    pub session_id: String,
    pub commands: Vec<AcpAvailableCommand>,
}

impl serde::Serialize for AcpBridgeEvent {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        match self {
            Self::SessionUpdate(update) => {
                // The wire type is derived from `kind` (e.g. "assistant_delta", "thinking_content").
                let event_type = update.kind.to_string();
                // "thinking_content" uses `content` as the text key; everything else uses `delta`.
                let text_key = if event_type == "thinking_content" {
                    "content"
                } else {
                    "delta"
                };
                let text_val = update.text_delta.clone().unwrap_or_default();

                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("type", &event_type)?;
                map.serialize_entry("session_id", &update.session_id)?;
                map.serialize_entry("tool_call_id", &update.tool_call_id)?;
                map.serialize_entry(text_key, &text_val)?;
                if let Some(ref name) = update.tool_name {
                    map.serialize_entry("tool_name", name)?;
                }
                if let Some(ref status) = update.tool_status {
                    map.serialize_entry("tool_status", status)?;
                }
                if let Some(ref content) = update.tool_content {
                    map.serialize_entry("tool_content", content)?;
                }
                if let Some(ref input) = update.tool_input {
                    map.serialize_entry("tool_input", input)?;
                }
                map.end()
            }
            Self::PermissionRequest(req) => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("type", "permission_request")?;
                map.serialize_entry("session_id", &req.session_id)?;
                map.serialize_entry("tool_call_id", &req.tool_call_id)?;
                map.serialize_entry("options", &req.option_ids)?;
                map.end()
            }
            Self::TurnResult(result) => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("type", "result")?;
                map.serialize_entry("session_id", &result.session_id)?;
                map.serialize_entry("stop_reason", &result.stop_reason)?;
                map.serialize_entry("result", &result.result)?;
                map.end()
            }
            Self::ConfigOptionsUpdate(options) => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("type", "config_options_update")?;
                map.serialize_entry("configOptions", options)?;
                map.end()
            }
            Self::PlanUpdate(plan) => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("type", "plan_update")?;
                map.serialize_entry("session_id", &plan.session_id)?;
                map.serialize_entry("entries", &plan.entries)?;
                map.end()
            }
            Self::ModeUpdate(mode) => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("type", "mode_update")?;
                map.serialize_entry("session_id", &mode.session_id)?;
                map.serialize_entry("currentModeId", &mode.current_mode_id)?;
                map.end()
            }
            Self::CommandsUpdate(cmds) => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("type", "commands_update")?;
                map.serialize_entry("session_id", &cmds.session_id)?;
                map.serialize_entry("commands", &cmds.commands)?;
                map.end()
            }
        }
    }
}

// Query / retrieve / ask / evaluate / suggest

#[derive(Debug, Clone, PartialEq)]
pub struct QueryResult {
    pub results: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrieveResult {
    pub chunks: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AskResult {
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvaluateResult {
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SuggestResult {
    pub urls: Vec<String>,
}

// Scrape / map / search / research

#[derive(Debug, Clone, PartialEq)]
pub struct ScrapeResult {
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapResult {
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub results: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResearchResult {
    pub payload: serde_json::Value,
}

// Lifecycle: crawl / embed / extract

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrawlStartResult {
    pub job_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CrawlJobResult {
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedStartResult {
    pub job_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmbedJobResult {
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractStartResult {
    pub job_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExtractJobResult {
    pub payload: serde_json::Value,
}

// Ingest / screenshot

#[derive(Debug, Clone, PartialEq)]
pub struct IngestResult {
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScreenshotResult {
    pub payload: serde_json::Value,
}
