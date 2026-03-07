use crate::crates::core::config::Config;
use crate::crates::jobs::extract::start_extract_job;
use crate::crates::services::events::{LogLevel, ServiceEvent, emit};
use crate::crates::services::types::{ExtractJobResult, ExtractStartResult};
use std::error::Error;
use tokio::sync::mpsc;

// --- Pure mapping helpers (no I/O, testable without live services) ---

pub fn map_extract_start_result(job_id: String) -> ExtractStartResult {
    ExtractStartResult { job_id }
}

pub fn map_extract_job_result(payload: serde_json::Value) -> ExtractJobResult {
    ExtractJobResult { payload }
}

// --- Service functions ---

/// Enqueue an extract job for the given URLs and return its job ID immediately.
/// The extract prompt is read from cfg.query if present.
pub async fn extract_start(
    cfg: &Config,
    urls: &[String],
    tx: Option<mpsc::Sender<ServiceEvent>>,
) -> Result<ExtractStartResult, Box<dyn Error>> {
    if urls.is_empty() {
        return Err("extract_start requires at least one URL".into());
    }

    let prompt = cfg.query.clone();

    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!("enqueueing extract job for {} URL(s)", urls.len()),
        },
    );

    let job_id = start_extract_job(cfg, urls, prompt).await?;

    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!("enqueued extract job: {job_id}"),
        },
    );

    Ok(map_extract_start_result(job_id.to_string()))
}
