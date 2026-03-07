use super::AxonMcpServer;
use super::common::{
    invalid_params, logged_internal_error, parse_job_id, parse_limit, parse_offset,
    parse_response_mode, respond_with_mode,
};
use crate::crates::core::http::validate_url;
use crate::crates::jobs::refresh::{
    RefreshScheduleCreate, cancel_refresh_job, cleanup_refresh_jobs, clear_refresh_jobs,
    create_refresh_schedule, delete_refresh_schedule, get_refresh_job, list_refresh_jobs,
    list_refresh_schedules, recover_stale_refresh_jobs, set_refresh_schedule_enabled,
    start_refresh_job,
};
use crate::crates::mcp::schema::{
    AxonToolResponse, RefreshRequest, RefreshSubaction, ResponseMode, StatusRequest,
};
use rmcp::ErrorData;

impl AxonMcpServer {
    pub(super) async fn handle_status(
        &self,
        _req: StatusRequest,
    ) -> Result<AxonToolResponse, ErrorData> {
        let (json, text) = crate::crates::cli::commands::status::status_full(self.cfg.as_ref())
            .await
            .map_err(|e| logged_internal_error("operation", e))?;

        Ok(AxonToolResponse::ok(
            "status",
            "status",
            serde_json::json!({
                "text": text,
                "json": json,
            }),
        ))
    }

    pub(super) async fn handle_refresh(
        &self,
        req: RefreshRequest,
    ) -> Result<AxonToolResponse, ErrorData> {
        let response_mode = parse_response_mode(req.response_mode);
        match req.subaction {
            RefreshSubaction::Start => {
                let urls = req
                    .urls
                    .or_else(|| req.url.map(|u| vec![u]))
                    .ok_or_else(|| invalid_params("urls or url is required for refresh.start"))?;
                if urls.is_empty() {
                    return Err(invalid_params("urls cannot be empty"));
                }
                let id = start_refresh_job(self.cfg.as_ref(), &urls)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "refresh",
                    "start",
                    serde_json::json!({ "job_id": id.to_string() }),
                ))
            }
            RefreshSubaction::Status => {
                let id = parse_job_id(req.job_id.as_ref())?;
                let job = get_refresh_job(self.cfg.as_ref(), id)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                respond_with_mode(
                    "refresh",
                    "status",
                    response_mode,
                    &format!("refresh-status-{id}"),
                    serde_json::json!({ "job": job }),
                )
            }
            RefreshSubaction::Cancel => {
                let id = parse_job_id(req.job_id.as_ref())?;
                let canceled = cancel_refresh_job(self.cfg.as_ref(), id)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "refresh",
                    "cancel",
                    serde_json::json!({ "job_id": id.to_string(), "canceled": canceled }),
                ))
            }
            RefreshSubaction::List => {
                let limit = parse_limit(req.limit, 20);
                let offset = parse_offset(req.offset);
                let jobs = list_refresh_jobs(self.cfg.as_ref(), limit, offset as i64)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                respond_with_mode(
                    "refresh",
                    "list",
                    response_mode,
                    "refresh-list",
                    serde_json::json!({ "jobs": jobs, "limit": limit, "offset": offset }),
                )
            }
            RefreshSubaction::Cleanup => {
                let deleted = cleanup_refresh_jobs(self.cfg.as_ref())
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "refresh",
                    "cleanup",
                    serde_json::json!({ "deleted": deleted }),
                ))
            }
            RefreshSubaction::Clear => {
                let deleted = clear_refresh_jobs(self.cfg.as_ref())
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "refresh",
                    "clear",
                    serde_json::json!({ "deleted": deleted }),
                ))
            }
            RefreshSubaction::Recover => {
                let recovered = recover_stale_refresh_jobs(self.cfg.as_ref())
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "refresh",
                    "recover",
                    serde_json::json!({ "recovered": recovered }),
                ))
            }
            RefreshSubaction::Schedule => self.handle_refresh_schedule(req, response_mode).await,
        }
    }

    async fn handle_refresh_schedule(
        &self,
        req: RefreshRequest,
        response_mode: ResponseMode,
    ) -> Result<AxonToolResponse, ErrorData> {
        let sub = req.schedule_subaction.as_deref().unwrap_or("list");
        match sub {
            "list" => {
                let limit = parse_limit(req.limit, 20);
                let schedules = list_refresh_schedules(self.cfg.as_ref(), limit)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                respond_with_mode(
                    "refresh",
                    "schedule",
                    response_mode,
                    "refresh-schedules",
                    serde_json::json!({ "schedules": schedules }),
                )
            }
            "create" => {
                let name = req.schedule_name.ok_or_else(|| {
                    invalid_params("schedule_name is required for schedule create")
                })?;
                let urls = req.urls.or_else(|| req.url.map(|u| vec![u]));
                let urls = urls.unwrap_or_default();
                if urls.is_empty() {
                    return Err(invalid_params(
                        "refresh schedule create requires at least one URL",
                    ));
                }
                for url in &urls {
                    validate_url(url).map_err(|e| invalid_params(e.to_string()))?;
                }
                let schedule = create_refresh_schedule(
                    self.cfg.as_ref(),
                    &RefreshScheduleCreate {
                        name,
                        seed_url: None,
                        urls: Some(urls),
                        every_seconds: 3600,
                        enabled: true,
                        next_run_at: chrono::Utc::now(),
                    },
                )
                .await
                .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "refresh",
                    "schedule",
                    serde_json::json!({ "created": schedule }),
                ))
            }
            "delete" => {
                let name = req.schedule_name.ok_or_else(|| {
                    invalid_params("schedule_name is required for schedule delete")
                })?;
                let deleted = delete_refresh_schedule(self.cfg.as_ref(), &name)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "refresh",
                    "schedule",
                    serde_json::json!({ "name": name, "deleted": deleted }),
                ))
            }
            "enable" | "disable" => {
                let name = req.schedule_name.ok_or_else(|| {
                    invalid_params("schedule_name is required for schedule enable/disable")
                })?;
                let enabled = sub == "enable";
                let updated = set_refresh_schedule_enabled(self.cfg.as_ref(), &name, enabled)
                    .await
                    .map_err(|e| logged_internal_error("operation", e))?;
                Ok(AxonToolResponse::ok(
                    "refresh",
                    "schedule",
                    serde_json::json!({ "name": name, "enabled": enabled, "updated": updated }),
                ))
            }
            other => Err(invalid_params(format!(
                "unknown schedule_subaction: {other}; expected list, create, delete, enable, disable"
            ))),
        }
    }
}
