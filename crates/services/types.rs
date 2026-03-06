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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpSessionProbeRequest {
    pub session_id: Option<String>,
    /// Optional model config option value to apply during probe.
    pub model: Option<String>,
}

/// A single selectable value within an ACP config option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpConfigSelectValue {
    pub value: String,
    pub name: String,
    pub description: Option<String>,
}

/// An ACP session config option (model selector, mode selector, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpConfigOption {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub current_value: String,
    pub options: Vec<AcpConfigSelectValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcpSessionUpdateKind {
    UserDelta,
    AssistantDelta,
    ThinkingDelta,
    ToolCallStarted,
    ToolCallUpdated,
    Plan,
    AvailableCommandsUpdate,
    CurrentModeUpdate,
    ConfigOptionUpdate,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpSessionUpdateEvent {
    pub session_id: String,
    pub kind: AcpSessionUpdateKind,
    pub text_delta: Option<String>,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpPermissionRequestEvent {
    pub session_id: String,
    pub tool_call_id: String,
    pub option_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
