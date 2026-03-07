use crate::crates::cli::commands::doctor::build_doctor_report;
use crate::crates::cli::commands::status::status_full;
use crate::crates::core::config::Config;
use crate::crates::services::events::{LogLevel, ServiceEvent, emit};
use crate::crates::services::types::{
    DedupeResult, DoctorResult, DomainFacet, DomainsResult, Pagination, SourcesResult, StatsResult,
    StatusResult,
};
use crate::crates::vector::ops::qdrant::{domains_payload, run_dedupe_native, sources_payload};
use crate::crates::vector::ops::stats::stats_payload;
use std::error::Error;
use std::fmt;
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct PayloadParseError(String);
impl fmt::Display for PayloadParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "payload parse error: {}", self.0)
    }
}
impl Error for PayloadParseError {}

pub fn map_sources_payload(
    payload: &serde_json::Value,
) -> Result<SourcesResult, PayloadParseError> {
    let count = payload
        .get("count")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| PayloadParseError("missing count".into()))? as usize;
    let limit = payload
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| PayloadParseError("missing limit".into()))? as usize;
    let offset = payload
        .get("offset")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| PayloadParseError("missing offset".into()))? as usize;
    let urls = payload
        .get("urls")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| PayloadParseError("missing urls".into()))?
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let url = item
                .get("url")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| PayloadParseError(format!("urls[{i}]: missing url")))?
                .to_string();
            let chunks = item
                .get("chunks")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| PayloadParseError(format!("urls[{i}]: missing chunks")))?
                as usize;
            Ok((url, chunks))
        })
        .collect::<Result<Vec<_>, PayloadParseError>>()?;

    Ok(SourcesResult {
        count,
        limit,
        offset,
        urls,
    })
}

pub fn map_domains_payload(
    payload: &serde_json::Value,
) -> Result<DomainsResult, PayloadParseError> {
    let limit = payload
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| PayloadParseError("missing limit".into()))? as usize;
    let offset = payload
        .get("offset")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| PayloadParseError("missing offset".into()))? as usize;

    let domains = payload
        .get("domains")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| PayloadParseError("missing domains".into()))?
        .iter()
        .enumerate()
        .map(|(i, item)| {
            Ok(DomainFacet {
                domain: item
                    .get("domain")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| PayloadParseError(format!("domains[{i}]: missing domain")))?
                    .to_string(),
                vectors: item
                    .get("vectors")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| PayloadParseError(format!("domains[{i}]: missing vectors")))?
                    as usize,
            })
        })
        .collect::<Result<Vec<_>, PayloadParseError>>()?;

    Ok(DomainsResult {
        domains,
        limit,
        offset,
    })
}

pub fn map_stats_payload(payload: serde_json::Value) -> StatsResult {
    StatsResult { payload }
}

pub fn map_doctor_payload(payload: serde_json::Value) -> DoctorResult {
    DoctorResult { payload }
}

pub async fn sources(
    cfg: &Config,
    pagination: Pagination,
) -> Result<SourcesResult, Box<dyn Error>> {
    let payload = sources_payload(cfg, pagination.limit, pagination.offset).await?;
    Ok(map_sources_payload(&payload)?)
}

pub async fn domains(
    cfg: &Config,
    pagination: Pagination,
) -> Result<DomainsResult, Box<dyn Error>> {
    let payload = domains_payload(cfg, pagination.limit, pagination.offset).await?;
    Ok(map_domains_payload(&payload)?)
}

pub async fn stats(cfg: &Config) -> Result<StatsResult, Box<dyn Error>> {
    let payload = stats_payload(cfg).await?;
    Ok(map_stats_payload(payload))
}

pub async fn doctor(cfg: &Config) -> Result<DoctorResult, Box<dyn Error>> {
    let payload = build_doctor_report(cfg).await?;
    Ok(map_doctor_payload(payload))
}

pub async fn full_status(cfg: &Config) -> Result<StatusResult, Box<dyn Error>> {
    let (payload, text) = status_full(cfg).await?;
    Ok(StatusResult { payload, text })
}

pub async fn dedupe(
    cfg: &Config,
    tx: Option<mpsc::Sender<ServiceEvent>>,
) -> Result<DedupeResult, Box<dyn Error>> {
    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: "starting dedupe".to_string(),
        },
    );
    if let Err(e) = run_dedupe_native(cfg).await {
        emit(
            &tx,
            ServiceEvent::Log {
                level: LogLevel::Error,
                message: format!("dedupe failed: {e}"),
            },
        );
        return Err(e);
    }
    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: "completed dedupe".to_string(),
        },
    );
    Ok(DedupeResult { completed: true })
}
