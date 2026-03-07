use crate::crates::cli::commands::research::research_payload;
use crate::crates::cli::commands::search::search_results;
use crate::crates::core::config::Config;
use crate::crates::services::events::{LogLevel, ServiceEvent, emit};
use crate::crates::services::types::{
    ResearchResult, SearchOptions, SearchResult, ServiceTimeRange,
};
use spider_agent::TimeRange;
use std::error::Error;
use tokio::sync::mpsc;

/// Convert a [`ServiceTimeRange`] to the `spider_agent` crate's [`TimeRange`].
///
/// Private — callers use the typed service options, not spider_agent types directly.
fn to_spider_time_range(tr: ServiceTimeRange) -> TimeRange {
    match tr {
        ServiceTimeRange::Day => TimeRange::Day,
        ServiceTimeRange::Week => TimeRange::Week,
        ServiceTimeRange::Month => TimeRange::Month,
        ServiceTimeRange::Year => TimeRange::Year,
    }
}

/// Map a `Vec<serde_json::Value>` of raw search items into a typed [`SearchResult`].
///
/// This is a pure function — no network required. Tests call it with JSON literals.
pub fn map_search_results(results: Vec<serde_json::Value>) -> SearchResult {
    SearchResult { results }
}

/// Map a raw JSON payload into a typed [`ResearchResult`].
///
/// This is a pure function — no network required. Tests call it with JSON literals.
pub fn map_research_payload(payload: serde_json::Value) -> ResearchResult {
    ResearchResult { payload }
}

/// Run a web search via Tavily and return a typed [`SearchResult`].
///
/// Delegates to [`search_results`] from the CLI commands layer. Emits log events
/// when a `tx` sender is provided.
pub async fn search(
    cfg: &Config,
    query: &str,
    opts: SearchOptions,
    tx: Option<mpsc::Sender<ServiceEvent>>,
) -> Result<SearchResult, Box<dyn Error>> {
    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!("starting search: {query}"),
        },
    );

    let time_range = opts.time_range.map(to_spider_time_range);
    let raw = search_results(cfg, query, opts.limit, opts.offset, time_range).await?;

    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!("search complete: {} results", raw.len()),
        },
    );

    Ok(map_search_results(raw))
}

/// Run a Tavily AI research query with LLM synthesis and return a typed [`ResearchResult`].
///
/// Delegates to [`research_payload`] from the CLI commands layer. Emits log events
/// when a `tx` sender is provided.
pub async fn research(
    cfg: &Config,
    query: &str,
    opts: SearchOptions,
    tx: Option<mpsc::Sender<ServiceEvent>>,
) -> Result<ResearchResult, Box<dyn Error>> {
    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: format!("starting research: {query}"),
        },
    );

    let time_range = opts.time_range.map(to_spider_time_range);
    let payload = research_payload(cfg, query, opts.limit, opts.offset, time_range).await?;

    emit(
        &tx,
        ServiceEvent::Log {
            level: LogLevel::Info,
            message: "research complete".to_string(),
        },
    );

    Ok(map_research_payload(payload))
}
