use super::AxonMcpServer;
use super::common::{
    apply_crawl_overrides, invalid_params, logged_internal_error, parse_job_id, parse_limit,
    parse_offset, parse_response_mode, respond_with_mode,
};
use crate::crates::core::http::validate_url;
use crate::crates::jobs::crawl::{
    cancel_job, cleanup_jobs, clear_jobs, list_jobs, recover_stale_crawl_jobs,
};
use crate::crates::jobs::extract::{
    cancel_extract_job, cleanup_extract_jobs, clear_extract_jobs, get_extract_job,
    list_extract_jobs, recover_stale_extract_jobs, start_extract_job,
};
use crate::crates::mcp::schema::{
    AxonToolResponse, CrawlRequest, CrawlSubaction, ExtractRequest, ExtractSubaction,
};
use crate::crates::services::crawl as crawl_svc;
use rmcp::ErrorData;

impl AxonMcpServer {
    pub(super) async fn handle_crawl(
        &self,
        req: CrawlRequest,
    ) -> Result<AxonToolResponse, ErrorData> {
        let cfg = apply_crawl_overrides(self.cfg.as_ref(), &req);
        let response_mode = parse_response_mode(req.response_mode);
        match req.subaction {
            CrawlSubaction::Start => {
                let urls = req
                    .urls
                    .ok_or_else(|| invalid_params("urls is required for crawl.start"))?;
                if urls.is_empty() {
                    return Err(invalid_params("urls cannot be empty"));
                }
                for url in &urls {
                    validate_url(url).map_err(|e| invalid_params(e.to_string()))?;
                }
                let result = crawl_svc::crawl_start(&cfg, &urls, None)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "crawl",
                    "start",
                    serde_json::json!({
                        "job_ids": result.job_ids
                    }),
                ))
            }
            CrawlSubaction::Status => {
                let id = parse_job_id(req.job_id.as_ref())?;
                let result = crawl_svc::crawl_status(&cfg, id)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "crawl",
                    "status",
                    serde_json::json!({ "job": result.payload }),
                ))
            }
            CrawlSubaction::Cancel => {
                let id = parse_job_id(req.job_id.as_ref())?;
                let canceled = cancel_job(&cfg, id)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "crawl",
                    "cancel",
                    serde_json::json!({ "job_id": id.to_string(), "canceled": canceled }),
                ))
            }
            CrawlSubaction::List => {
                let limit = parse_limit(req.limit, 20);
                let offset = parse_offset(req.offset);
                let jobs = list_jobs(&cfg, limit, offset as i64)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                respond_with_mode(
                    "crawl",
                    "list",
                    response_mode,
                    "crawl-list",
                    serde_json::json!({ "jobs": jobs, "limit": limit, "offset": offset }),
                )
            }
            CrawlSubaction::Cleanup => {
                let deleted = cleanup_jobs(&cfg)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "crawl",
                    "cleanup",
                    serde_json::json!({ "deleted": deleted }),
                ))
            }
            CrawlSubaction::Clear => {
                let deleted = clear_jobs(&cfg)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "crawl",
                    "clear",
                    serde_json::json!({ "deleted": deleted }),
                ))
            }
            CrawlSubaction::Recover => {
                let recovered = recover_stale_crawl_jobs(&cfg)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "crawl",
                    "recover",
                    serde_json::json!({ "recovered": recovered }),
                ))
            }
        }
    }

    pub(super) async fn handle_extract(
        &self,
        req: ExtractRequest,
    ) -> Result<AxonToolResponse, ErrorData> {
        let response_mode = parse_response_mode(req.response_mode);
        match req.subaction {
            ExtractSubaction::Start => {
                let urls = req
                    .urls
                    .ok_or_else(|| invalid_params("urls is required for extract.start"))?;
                if urls.is_empty() {
                    return Err(invalid_params("urls cannot be empty"));
                }
                for url in &urls {
                    validate_url(url).map_err(|e| invalid_params(e.to_string()))?;
                }
                let id = start_extract_job(self.cfg.as_ref(), &urls, req.prompt)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "extract",
                    "start",
                    serde_json::json!({ "job_id": id.to_string() }),
                ))
            }
            ExtractSubaction::Status => {
                let id = parse_job_id(req.job_id.as_ref())?;
                let job = get_extract_job(self.cfg.as_ref(), id)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                respond_with_mode(
                    "extract",
                    "status",
                    response_mode,
                    &format!("extract-status-{id}"),
                    serde_json::json!({ "job": job }),
                )
            }
            ExtractSubaction::Cancel => {
                let id = parse_job_id(req.job_id.as_ref())?;
                let canceled = cancel_extract_job(self.cfg.as_ref(), id)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "extract",
                    "cancel",
                    serde_json::json!({ "job_id": id.to_string(), "canceled": canceled }),
                ))
            }
            ExtractSubaction::List => {
                let limit = parse_limit(req.limit, 20);
                let offset = parse_offset(req.offset);
                let jobs = list_extract_jobs(self.cfg.as_ref(), limit, offset as i64)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                respond_with_mode(
                    "extract",
                    "list",
                    response_mode,
                    "extract-list",
                    serde_json::json!({ "jobs": jobs, "limit": limit, "offset": offset }),
                )
            }
            ExtractSubaction::Cleanup => {
                let deleted = cleanup_extract_jobs(self.cfg.as_ref())
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "extract",
                    "cleanup",
                    serde_json::json!({ "deleted": deleted }),
                ))
            }
            ExtractSubaction::Clear => {
                let deleted = clear_extract_jobs(self.cfg.as_ref())
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "extract",
                    "clear",
                    serde_json::json!({ "deleted": deleted }),
                ))
            }
            ExtractSubaction::Recover => {
                let recovered = recover_stale_extract_jobs(self.cfg.as_ref())
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "extract",
                    "recover",
                    serde_json::json!({ "recovered": recovered }),
                ))
            }
        }
    }
}
