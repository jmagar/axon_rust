use crate::crates::core::config::Config;
use crate::crates::jobs::embed::start_embed_job;
use crate::crates::services::events::{LogLevel, ServiceEvent, emit};
use crate::crates::services::types::{EmbedJobResult, EmbedStartResult};
use std::error::Error;
use tokio::sync::mpsc;

// --- Pure mapping helpers (no I/O, testable without live services) ---

pub fn map_embed_start_result(job_id: String) -> EmbedStartResult {
    EmbedStartResult { job_id }
}

pub fn map_embed_job_result(payload: serde_json::Value) -> EmbedJobResult {
    EmbedJobResult { payload }
}

// --- Service functions ---

/// Enqueue an embed job for the input specified in cfg and return its job ID
/// immediately. The embed input is resolved from cfg.positional or cfg.output_dir
/// following the same logic as the CLI embed command.
pub async fn embed_start(
    cfg: &Config,
    tx: Option<mpsc::Sender<ServiceEvent>>,
) -> Result<EmbedStartResult, Box<dyn Error>> {
    let input = cfg.positional.first().cloned().unwrap_or_else(|| {
        cfg.output_dir
            .join("markdown")
            .to_string_lossy()
            .to_string()
    });

    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!("enqueueing embed job for input: {input}"),
        },
    );

    let job_id = start_embed_job(cfg, &input).await?;

    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!("enqueued embed job: {job_id}"),
        },
    );

    Ok(map_embed_start_result(job_id.to_string()))
}
