use super::AxonMcpServer;
use crate::crates::cli::commands::scrape::scrape_payload as cli_scrape_payload;
use crate::crates::core::config::{Config, RenderMode};
use crate::crates::mcp::schema::{
    AxonToolResponse, CrawlRequest, McpRenderMode, ResponseMode, SearchTimeRange,
};
use crate::crates::services::types::{
    MapOptions, Pagination, RetrieveOptions, SearchOptions, ServiceTimeRange,
};
use rmcp::ErrorData;
use sha2::{Digest, Sha256};
use spider_agent::TimeRange;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

pub(super) const MCP_TOOL_SCHEMA_URI: &str = "axon://schema/mcp-tool";

impl AxonMcpServer {
    pub(super) async fn scrape_payload(&self, url: &str) -> Result<serde_json::Value, ErrorData> {
        cli_scrape_payload(self.cfg.as_ref(), url)
            .await
            .map_err(|e| internal_error(e.to_string()))
    }

    pub(super) fn parse_viewport(
        viewport: Option<&str>,
        fallback_w: u32,
        fallback_h: u32,
    ) -> (u32, u32) {
        let Some(v) = viewport else {
            return (fallback_w, fallback_h);
        };
        let mut parts = v.split('x');
        let Some(w) = parts.next().and_then(|n| n.parse::<u32>().ok()) else {
            return (fallback_w, fallback_h);
        };
        let Some(h) = parts.next().and_then(|n| n.parse::<u32>().ok()) else {
            return (fallback_w, fallback_h);
        };
        if w == 0 || h == 0 {
            return (fallback_w, fallback_h);
        }
        (w, h)
    }
}

pub(super) fn invalid_params(msg: impl Into<String>) -> ErrorData {
    ErrorData::invalid_params(msg.into(), None)
}

pub(super) fn internal_error(msg: impl Into<String>) -> ErrorData {
    ErrorData::internal_error(msg.into(), None)
}

pub(super) fn parse_job_id(job_id: Option<&String>) -> Result<Uuid, ErrorData> {
    let raw = job_id.ok_or_else(|| invalid_params("job_id is required for this subaction"))?;
    Uuid::parse_str(raw).map_err(|e| invalid_params(format!("invalid job_id: {e}")))
}

pub(super) fn parse_limit(limit: Option<i64>, default: i64) -> i64 {
    limit.unwrap_or(default).clamp(1, 500)
}

pub(super) fn parse_limit_usize(limit: Option<usize>, default: usize, max: usize) -> usize {
    limit.unwrap_or(default).clamp(1, max)
}

pub(super) fn parse_offset(offset: Option<usize>) -> usize {
    offset.unwrap_or(0)
}

pub(super) fn parse_response_mode(mode: Option<ResponseMode>) -> ResponseMode {
    mode.unwrap_or(ResponseMode::Path)
}

pub(super) fn paginate_vec<T: Clone>(items: &[T], offset: usize, limit: usize) -> Vec<T> {
    items.iter().skip(offset).take(limit).cloned().collect()
}

pub(super) fn artifact_root() -> PathBuf {
    PathBuf::from(".cache/axon-mcp")
}

pub(super) fn ensure_artifact_root() -> Result<PathBuf, ErrorData> {
    let root = artifact_root();
    fs::create_dir_all(&root).map_err(|e| internal_error(e.to_string()))?;
    Ok(root)
}

pub(super) fn slugify(value: &str, max_len: usize) -> String {
    let mut out = String::with_capacity(value.len().min(max_len));
    let mut prev_dash = false;
    for ch in value.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
        if out.len() >= max_len {
            break;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "artifact".to_string()
    } else {
        trimmed
    }
}

pub(super) fn short_preview(text: &str, max_chars: usize) -> (String, bool) {
    if text.chars().count() <= max_chars {
        return (text.to_string(), false);
    }
    (text.chars().take(max_chars).collect::<String>(), true)
}

pub(super) fn line_count(text: &str) -> usize {
    text.lines().count()
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub(super) fn build_artifact_path(stem: &str, ext: &str) -> Result<PathBuf, ErrorData> {
    let root = ensure_artifact_root()?;
    Ok(root.join(format!("{stem}.{ext}")))
}

pub(super) fn write_text_artifact(stem: &str, text: &str) -> Result<serde_json::Value, ErrorData> {
    let path = build_artifact_path(stem, "json")?;
    fs::write(&path, text.as_bytes()).map_err(|e| internal_error(e.to_string()))?;
    let bytes = text.as_bytes();
    let (preview, preview_truncated) = short_preview(text, 600);
    Ok(serde_json::json!({
        "path": path,
        "bytes": bytes.len(),
        "line_count": line_count(text),
        "sha256": sha256_hex(bytes),
        "preview": preview,
        "preview_truncated": preview_truncated,
    }))
}

pub(super) fn validate_artifact_path(raw: &str) -> Result<PathBuf, ErrorData> {
    let root = ensure_artifact_root()?
        .canonicalize()
        .map_err(|e| internal_error(e.to_string()))?;
    let candidate = PathBuf::from(raw);
    let canonical = if candidate.is_absolute() {
        candidate
            .canonicalize()
            .map_err(|e| invalid_params(format!("artifact path not found: {e}")))?
    } else {
        let cwd = std::env::current_dir().map_err(|e| internal_error(e.to_string()))?;
        let from_cwd = cwd.join(&candidate);
        match from_cwd.canonicalize() {
            Ok(p) => p,
            Err(_) => root
                .join(&candidate)
                .canonicalize()
                .map_err(|e| invalid_params(format!("artifact path not found: {e}")))?,
        }
    };
    if !canonical.starts_with(&root) {
        return Err(invalid_params(
            "artifact path must be inside .cache/axon-mcp",
        ));
    }
    Ok(canonical)
}

pub(super) fn resolve_artifact_output_path(raw: &str) -> Result<PathBuf, ErrorData> {
    let candidate = PathBuf::from(raw);
    if candidate.as_os_str().is_empty() {
        return Err(invalid_params("output path cannot be empty"));
    }
    if candidate.is_absolute() {
        return Err(invalid_params(
            "output path must be relative to .cache/axon-mcp",
        ));
    }
    if candidate.components().any(|c| {
        matches!(
            c,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    }) {
        return Err(invalid_params(
            "output path cannot contain traversal components",
        ));
    }
    Ok(ensure_artifact_root()?.join(candidate))
}

pub(super) fn clip_inline_json(
    value: &serde_json::Value,
    max_chars: usize,
) -> (serde_json::Value, bool) {
    match serde_json::to_string(value) {
        Ok(raw) if raw.chars().count() <= max_chars => (value.clone(), false),
        Ok(raw) => {
            let clipped = raw.chars().take(max_chars).collect::<String>();
            (serde_json::json!({ "clipped_json": clipped }), true)
        }
        Err(_) => (
            serde_json::json!({ "clipped_json": "(serialization error)" }),
            true,
        ),
    }
}

pub(super) fn respond_with_mode(
    action: &str,
    subaction: &str,
    mode: ResponseMode,
    artifact_stem: &str,
    payload: serde_json::Value,
) -> Result<AxonToolResponse, ErrorData> {
    let text = serde_json::to_string(&payload).map_err(|e| internal_error(e.to_string()))?;
    let artifact = write_text_artifact(artifact_stem, &text)?;
    match mode {
        ResponseMode::Path => Ok(AxonToolResponse::ok(
            action,
            subaction,
            serde_json::json!({
                "response_mode": "path",
                "artifact": artifact,
                "status": "saved",
            }),
        )),
        ResponseMode::Inline => {
            let (inline, truncated) = clip_inline_json(&payload, 12_000);
            Ok(AxonToolResponse::ok(
                action,
                subaction,
                serde_json::json!({
                    "response_mode": "inline",
                    "inline": inline,
                    "truncated": truncated,
                    "artifact": artifact,
                }),
            ))
        }
        ResponseMode::Both => {
            let (inline, truncated) = clip_inline_json(&payload, 12_000);
            Ok(AxonToolResponse::ok(
                action,
                subaction,
                serde_json::json!({
                    "response_mode": "both",
                    "inline": inline,
                    "truncated": truncated,
                    "artifact": artifact,
                }),
            ))
        }
    }
}

pub(super) fn apply_crawl_overrides(cfg: &Config, req: &CrawlRequest) -> Config {
    let mut out = cfg.clone();
    if let Some(max_pages) = req.max_pages {
        out.max_pages = max_pages;
    }
    if let Some(max_depth) = req.max_depth {
        out.max_depth = max_depth;
    }
    if let Some(include_subdomains) = req.include_subdomains {
        out.include_subdomains = include_subdomains;
    }
    if let Some(respect_robots) = req.respect_robots {
        out.respect_robots = respect_robots;
    }
    if let Some(discover_sitemaps) = req.discover_sitemaps {
        out.discover_sitemaps = discover_sitemaps;
    }
    if let Some(sitemap_since_days) = req.sitemap_since_days {
        out.sitemap_since_days = sitemap_since_days;
    }
    if let Some(render_mode) = req.render_mode {
        out.render_mode = map_render_mode(render_mode);
    }
    if let Some(delay_ms) = req.delay_ms {
        out.delay_ms = delay_ms;
    }
    out
}

pub(super) fn map_render_mode(mode: McpRenderMode) -> RenderMode {
    match mode {
        McpRenderMode::Http => RenderMode::Http,
        McpRenderMode::Chrome => RenderMode::Chrome,
        McpRenderMode::AutoSwitch => RenderMode::AutoSwitch,
    }
}

pub(super) fn map_search_time_range(range: &SearchTimeRange) -> TimeRange {
    match range {
        SearchTimeRange::Day => TimeRange::Day,
        SearchTimeRange::Week => TimeRange::Week,
        SearchTimeRange::Month => TimeRange::Month,
        SearchTimeRange::Year => TimeRange::Year,
    }
}

/// Map MCP limit/offset params to service `Pagination`, clamping limit to [1, 500].
pub fn to_pagination(limit: Option<usize>, offset: Option<usize>) -> Pagination {
    Pagination {
        limit: limit.unwrap_or(10).clamp(1, 500),
        offset: offset.unwrap_or(0),
    }
}

/// Map MCP limit/offset params to service `MapOptions`, clamping limit to [1, 500].
pub fn to_map_options(limit: Option<usize>, offset: Option<usize>) -> MapOptions {
    MapOptions {
        limit: limit.unwrap_or(10).clamp(1, 500),
        offset: offset.unwrap_or(0),
    }
}

/// Map MCP `RetrieveOptions` (max_points field) to service `RetrieveOptions`.
pub fn to_retrieve_options(max_points: Option<usize>) -> RetrieveOptions {
    RetrieveOptions { max_points }
}

/// Map MCP `SearchTimeRange` enum to service `ServiceTimeRange`.
pub fn to_service_time_range(tr: SearchTimeRange) -> ServiceTimeRange {
    match tr {
        SearchTimeRange::Day => ServiceTimeRange::Day,
        SearchTimeRange::Week => ServiceTimeRange::Week,
        SearchTimeRange::Month => ServiceTimeRange::Month,
        SearchTimeRange::Year => ServiceTimeRange::Year,
    }
}

/// Map MCP search params to service `SearchOptions`.
pub fn to_search_options(
    limit: Option<usize>,
    offset: Option<usize>,
    time_range: Option<SearchTimeRange>,
) -> SearchOptions {
    SearchOptions {
        limit: limit.unwrap_or(10).clamp(1, 500),
        offset: offset.unwrap_or(0),
        time_range: time_range.map(to_service_time_range),
    }
}
