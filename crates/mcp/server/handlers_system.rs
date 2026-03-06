use super::AxonMcpServer;
use super::common::{
    MCP_TOOL_SCHEMA_URI, artifact_root, ensure_artifact_root, invalid_params, line_count,
    logged_internal_error, parse_limit_usize, parse_offset, parse_response_mode,
    resolve_artifact_output_path, respond_with_mode, sha256_hex, to_pagination,
    validate_artifact_path,
};
use crate::crates::cli::commands::screenshot::{
    spider_screenshot_with_options, url_to_screenshot_filename,
};
use crate::crates::core::http::{normalize_url, validate_url};
use crate::crates::mcp::schema::{
    ArtifactsRequest, ArtifactsSubaction, AxonToolResponse, DoctorRequest, DomainsRequest,
    HelpRequest, ScreenshotRequest, SourcesRequest, StatsRequest,
};
use crate::crates::services::system;
use rmcp::ErrorData;
use std::fs;

impl AxonMcpServer {
    pub(super) async fn handle_screenshot(
        &self,
        req: ScreenshotRequest,
    ) -> Result<AxonToolResponse, ErrorData> {
        let url = req
            .url
            .ok_or_else(|| invalid_params("url is required for screenshot"))?;
        let _response_mode = parse_response_mode(req.response_mode);
        let normalized = normalize_url(&url);
        validate_url(&normalized).map_err(|e| invalid_params(e.to_string()))?;

        let (width, height) = Self::parse_viewport(
            req.viewport.as_deref(),
            self.cfg.viewport_width,
            self.cfg.viewport_height,
        );
        let full_page = req.full_page.unwrap_or(self.cfg.screenshot_full_page);

        let bytes =
            spider_screenshot_with_options(&self.cfg, &normalized, width, height, full_page)
                .await
                .map_err(|e| logged_internal_error("operation", e))?;

        let path = if let Some(output) = req.output {
            resolve_artifact_output_path(&output)?
        } else {
            ensure_artifact_root()?
                .join("screenshots")
                .join(url_to_screenshot_filename(&normalized, 1))
        };
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| logged_internal_error("operation", e))?;
        }
        tokio::fs::write(&path, &bytes)
            .await
            .map_err(|e| logged_internal_error("operation", e))?;

        Ok(AxonToolResponse::ok(
            "screenshot",
            "screenshot",
            serde_json::json!({
                "url": normalized,
                "path": path,
                "size_bytes": bytes.len(),
                "full_page": full_page,
                "viewport": format!("{}x{}", width, height),
            }),
        ))
    }

    pub(super) async fn handle_artifacts(
        &self,
        req: ArtifactsRequest,
    ) -> Result<AxonToolResponse, ErrorData> {
        let path = req
            .path
            .as_deref()
            .ok_or_else(|| invalid_params("path is required for artifacts operations"))?;
        let path = validate_artifact_path(path)?;
        let text = fs::read_to_string(&path).map_err(|e| logged_internal_error("operation", e))?;

        match req.subaction {
            ArtifactsSubaction::Head => {
                let limit = parse_limit_usize(req.limit, 25, 500);
                let head = text.lines().take(limit).collect::<Vec<_>>().join("\n");
                Ok(AxonToolResponse::ok(
                    "artifacts",
                    "head",
                    serde_json::json!({
                        "path": path,
                        "limit": limit,
                        "line_count": line_count(&text),
                        "head": head,
                    }),
                ))
            }
            ArtifactsSubaction::Grep => {
                let pattern = req
                    .pattern
                    .as_deref()
                    .ok_or_else(|| invalid_params("pattern is required for artifacts.grep"))?;
                let limit = parse_limit_usize(req.limit, 25, 500);
                let offset = parse_offset(req.offset);
                let matches = text
                    .lines()
                    .enumerate()
                    .filter(|(_, line)| line.contains(pattern))
                    .skip(offset)
                    .take(limit)
                    .map(|(idx, line)| serde_json::json!({ "line": idx + 1, "text": line }))
                    .collect::<Vec<_>>();
                Ok(AxonToolResponse::ok(
                    "artifacts",
                    "grep",
                    serde_json::json!({
                        "path": path,
                        "pattern": pattern,
                        "limit": limit,
                        "offset": offset,
                        "matches": matches,
                    }),
                ))
            }
            ArtifactsSubaction::Wc => Ok(AxonToolResponse::ok(
                "artifacts",
                "wc",
                serde_json::json!({
                    "path": path,
                    "bytes": text.len(),
                    "lines": line_count(&text),
                    "sha256": sha256_hex(text.as_bytes()),
                }),
            )),
            ArtifactsSubaction::Read => {
                let limit = parse_limit_usize(req.limit, 2000, 20_000);
                let offset = parse_offset(req.offset);
                let content = text
                    .lines()
                    .skip(offset)
                    .take(limit)
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(AxonToolResponse::ok(
                    "artifacts",
                    "read",
                    serde_json::json!({
                        "path": path,
                        "offset": offset,
                        "limit": limit,
                        "content": content,
                    }),
                ))
            }
        }
    }

    pub(super) async fn handle_help(
        &self,
        req: HelpRequest,
    ) -> Result<AxonToolResponse, ErrorData> {
        respond_with_mode(
            "help",
            "help",
            parse_response_mode(req.response_mode),
            "help-actions",
            serde_json::json!({
                "tool": "axon",
                "actions": {
                    "status": [],
                    "help": [],
                    "scrape": ["scrape"],
                    "research": ["research"],
                    "ask": ["ask"],
                    "screenshot": ["screenshot"],
                    "crawl": ["start", "status", "cancel", "list", "cleanup", "clear", "recover"],
                    "extract": ["start", "status", "cancel", "list", "cleanup", "clear", "recover"],
                    "embed": ["start", "status", "cancel", "list", "cleanup", "clear", "recover"],
                    "ingest": ["start", "status", "cancel", "list", "cleanup", "clear", "recover"],
                    "refresh": ["start", "status", "cancel", "list", "cleanup", "clear", "recover", "schedule"],
                    "query": ["query"],
                    "retrieve": ["retrieve"],
                    "search": ["search"],
                    "map": ["map"],
                    "doctor": ["doctor"],
                    "domains": ["domains"],
                    "sources": ["sources"],
                    "stats": ["stats"],
                    "artifacts": ["head", "grep", "wc", "read"]
                },
                "resources": [
                    MCP_TOOL_SCHEMA_URI
                ],
                "defaults": {
                    "response_mode": "path",
                    "artifact_dir": artifact_root()
                }
            }),
        )
    }

    pub(super) async fn handle_doctor(
        &self,
        _req: DoctorRequest,
    ) -> Result<AxonToolResponse, ErrorData> {
        let result = system::doctor(self.cfg.as_ref())
            .await
            .map_err(|e| logged_internal_error("operation", e))?;
        Ok(AxonToolResponse::ok("doctor", "doctor", result.payload))
    }

    pub(super) async fn handle_domains(
        &self,
        req: DomainsRequest,
    ) -> Result<AxonToolResponse, ErrorData> {
        let pagination = to_pagination(req.limit.or(Some(25)), req.offset);
        let response_mode = parse_response_mode(req.response_mode);
        let result = system::domains(self.cfg.as_ref(), pagination)
            .await
            .map_err(|e| logged_internal_error("operation", e))?;
        let payload = serde_json::json!({
            "limit": result.limit,
            "offset": result.offset,
            "domains": result.domains.iter().map(|d| serde_json::json!({
                "domain": d.domain,
                "vectors": d.vectors,
            })).collect::<Vec<_>>(),
        });
        respond_with_mode("domains", "domains", response_mode, "domains", payload)
    }

    pub(super) async fn handle_sources(
        &self,
        req: SourcesRequest,
    ) -> Result<AxonToolResponse, ErrorData> {
        let pagination = to_pagination(req.limit.or(Some(25)), req.offset);
        let response_mode = parse_response_mode(req.response_mode);
        let result = system::sources(self.cfg.as_ref(), pagination)
            .await
            .map_err(|e| logged_internal_error("operation", e))?;
        let payload = serde_json::json!({
            "count": result.count,
            "limit": result.limit,
            "offset": result.offset,
            // Wire contract: urls is string[] (chunk counts are service-layer internal)
            "urls": result.urls.iter().map(|(url, _chunks)| url).collect::<Vec<_>>(),
        });
        respond_with_mode("sources", "sources", response_mode, "sources", payload)
    }

    pub(super) async fn handle_stats(
        &self,
        _req: StatsRequest,
    ) -> Result<AxonToolResponse, ErrorData> {
        let result = system::stats(self.cfg.as_ref())
            .await
            .map_err(|e| logged_internal_error("operation", e))?;
        Ok(AxonToolResponse::ok("stats", "stats", result.payload))
    }
}
