use crate::crates::core::config::Config;
use crate::crates::jobs::crawl;
use crate::crates::services::events::{LogLevel, ServiceEvent, emit};
use crate::crates::services::types::{CrawlJobResult, CrawlStartResult};
use std::error::Error;
use tokio::sync::mpsc;
use uuid::Uuid;

// --- Pure mapping helpers (no I/O, testable without live services) ---

pub fn map_crawl_start_result(job_ids: Vec<String>) -> CrawlStartResult {
    CrawlStartResult { job_ids }
}

pub fn map_crawl_job_result(payload: serde_json::Value) -> CrawlJobResult {
    CrawlJobResult { payload }
}

// --- Service functions ---

/// Enqueue one or more crawl jobs and return their job IDs immediately.
/// Fire-and-forget: jobs are inserted into the queue and this function returns
/// without waiting for the crawl to complete.
pub async fn crawl_start(
    cfg: &Config,
    urls: &[String],
    tx: Option<mpsc::Sender<ServiceEvent>>,
) -> Result<CrawlStartResult, Box<dyn Error>> {
    if urls.is_empty() {
        return Err("crawl_start: no URLs provided".into());
    }

    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!("enqueueing crawl jobs for {} URL(s)", urls.len()),
        },
    );

    let url_refs: Vec<&str> = urls.iter().map(String::as_str).collect();
    let jobs = crawl::start_crawl_jobs_batch(cfg, &url_refs).await?;

    let job_ids: Vec<String> = jobs.into_iter().map(|(_, id)| id.to_string()).collect();

    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!("enqueued {} crawl job(s)", job_ids.len()),
        },
    );

    Ok(map_crawl_start_result(job_ids))
}

/// Look up the current state of a crawl job by its UUID.
pub async fn crawl_status(cfg: &Config, job_id: Uuid) -> Result<CrawlJobResult, Box<dyn Error>> {
    let job = crawl::get_job(cfg, job_id).await?;
    let payload = serde_json::to_value(job)?;
    Ok(map_crawl_job_result(payload))
}
