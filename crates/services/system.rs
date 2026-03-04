use crate::crates::cli::commands::doctor::build_doctor_report;
use crate::crates::cli::commands::status::status_full;
use crate::crates::core::config::Config;
use crate::crates::services::events::{ServiceEvent, emit};
use crate::crates::services::types::{
    DedupeResult, DoctorResult, DomainFacet, DomainsResult, Pagination, SourcesResult, StatsResult,
    StatusResult,
};
use crate::crates::vector::ops::qdrant::{domains_payload, run_dedupe_native, sources_payload};
use crate::crates::vector::ops::stats::stats_payload;
use std::error::Error;
use tokio::sync::mpsc;

pub fn map_sources_payload(payload: &serde_json::Value) -> Result<SourcesResult, Box<dyn Error>> {
    let count = payload
        .get("count")
        .and_then(serde_json::Value::as_u64)
        .ok_or("missing count")? as usize;
    let limit = payload
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .ok_or("missing limit")? as usize;
    let offset = payload
        .get("offset")
        .and_then(serde_json::Value::as_u64)
        .ok_or("missing offset")? as usize;
    let urls = payload
        .get("urls")
        .and_then(serde_json::Value::as_array)
        .ok_or("missing urls")?
        .iter()
        .filter_map(|item| {
            let url = item.get("url")?.as_str()?.to_string();
            let chunks = item.get("chunks")?.as_u64()? as usize;
            Some((url, chunks))
        })
        .collect::<Vec<_>>();

    Ok(SourcesResult {
        count,
        limit,
        offset,
        urls,
    })
}

pub fn map_domains_payload(payload: &serde_json::Value) -> Result<DomainsResult, Box<dyn Error>> {
    let limit = payload
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .ok_or("missing limit")? as usize;
    let offset = payload
        .get("offset")
        .and_then(serde_json::Value::as_u64)
        .ok_or("missing offset")? as usize;

    let domains = payload
        .get("domains")
        .and_then(serde_json::Value::as_array)
        .ok_or("missing domains")?
        .iter()
        .filter_map(|item| {
            Some(DomainFacet {
                domain: item.get("domain")?.as_str()?.to_string(),
                vectors: item.get("vectors")?.as_u64()? as usize,
            })
        })
        .collect::<Vec<_>>();

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
    map_sources_payload(&payload)
}

pub async fn domains(
    cfg: &Config,
    pagination: Pagination,
) -> Result<DomainsResult, Box<dyn Error>> {
    let payload = domains_payload(cfg, pagination.limit, pagination.offset).await?;
    map_domains_payload(&payload)
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
            level: "info".to_string(),
            message: "starting dedupe".to_string(),
        },
    );
    run_dedupe_native(cfg).await?;
    emit(
        &tx,
        ServiceEvent::Log {
            level: "info".to_string(),
            message: "completed dedupe".to_string(),
        },
    );
    Ok(DedupeResult { completed: true })
}
