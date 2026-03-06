use crate::crates::core::config::Config;
use crate::crates::services::events::{ServiceEvent, emit};
use crate::crates::services::types::{
    AskResult, EvaluateResult, Pagination, QueryResult, RetrieveOptions, RetrieveResult,
    SuggestResult,
};
use crate::crates::vector::ops::commands::ask::ask_payload;
use crate::crates::vector::ops::commands::discover_crawl_suggestions;
use crate::crates::vector::ops::commands::query_results;
use crate::crates::vector::ops::commands::run_evaluate_native;
use crate::crates::vector::ops::qdrant::retrieve_result;
use std::error::Error;
use tokio::sync::mpsc;

// ── Pure mapping helpers (unit-testable, no live services required) ──────────

pub fn map_query_results(results: Vec<serde_json::Value>) -> QueryResult {
    QueryResult { results }
}

pub fn map_retrieve_result(chunk_count: usize, content: String) -> RetrieveResult {
    let chunks = if chunk_count == 0 {
        Vec::new()
    } else {
        vec![serde_json::json!({
            "chunk_count": chunk_count,
            "content": content
        })]
    };
    RetrieveResult { chunks }
}

pub fn map_ask_payload(payload: serde_json::Value) -> AskResult {
    AskResult { payload }
}

pub fn map_evaluate_payload(payload: serde_json::Value) -> EvaluateResult {
    EvaluateResult { payload }
}

pub fn map_suggest_payload(payload: &serde_json::Value) -> Result<SuggestResult, Box<dyn Error>> {
    let suggestions = payload
        .get("suggestions")
        .and_then(serde_json::Value::as_array)
        .ok_or("missing suggestions array")?;
    let urls = suggestions
        .iter()
        .enumerate()
        .map(|(i, item)| {
            item.get("url")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
                .ok_or_else(|| format!("suggestions[{i}]: missing url").into())
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    Ok(SuggestResult { urls })
}

// ── Service functions (call-through wrappers) ────────────────────────────────

/// Semantic vector search.
pub async fn query(
    cfg: &Config,
    text: &str,
    opts: Pagination,
) -> Result<QueryResult, Box<dyn Error>> {
    let results = query_results(cfg, text, opts.limit.max(1), opts.offset).await?;
    Ok(map_query_results(results))
}

/// Retrieve stored document chunks for a URL.
pub async fn retrieve(
    cfg: &Config,
    url: &str,
    opts: RetrieveOptions,
) -> Result<RetrieveResult, Box<dyn Error>> {
    let (chunk_count, content) = retrieve_result(cfg, url, opts.max_points).await?;
    Ok(map_retrieve_result(chunk_count, content))
}

/// RAG ask: retrieve relevant context, then answer with LLM.
pub async fn ask(
    cfg: &Config,
    question: &str,
    tx: Option<mpsc::Sender<ServiceEvent>>,
) -> Result<AskResult, Box<dyn Error>> {
    emit(
        &tx,
        ServiceEvent::Log {
            level: "info".to_string(),
            message: format!(
                "starting ask: {}",
                question.chars().take(80).collect::<String>()
            ),
        },
    );
    let payload = ask_payload(cfg, question)
        .await
        .map_err(|e| -> Box<dyn Error> { e.into() })?;
    emit(
        &tx,
        ServiceEvent::Log {
            level: "info".to_string(),
            message: "ask complete".to_string(),
        },
    );
    Ok(map_ask_payload(payload))
}

/// RAG evaluate: run RAG and baseline answers, then judge with a second LLM call.
///
/// Note: `run_evaluate_native` writes its JSON output to stdout when
/// `cfg.json_output` is true. This wrapper calls the native function for its
/// side effects and returns a completed marker. Callers that need the structured
/// JSON payload should capture stdout or use `ask_payload` directly.
pub async fn evaluate(cfg: &Config, question: &str) -> Result<EvaluateResult, Box<dyn Error>> {
    let mut derived = cfg.clone();
    derived.query = Some(question.to_string());
    derived.positional = Vec::new();
    run_evaluate_native(&derived).await?;
    Ok(map_evaluate_payload(serde_json::json!({
        "question": question,
        "note": "output emitted to stdout; set cfg.json_output=true for structured JSON"
    })))
}

/// Suggest new URLs to crawl based on the current Qdrant index and an optional focus.
///
/// Returns the accepted suggestion URLs directly (no stdout side effects).
pub async fn suggest(cfg: &Config, focus: Option<&str>) -> Result<SuggestResult, Box<dyn Error>> {
    let mut derived = cfg.clone();
    derived.query = focus.map(ToString::to_string);
    derived.positional = Vec::new();
    let desired = derived.search_limit.clamp(1, 100);
    let focus_str = focus.unwrap_or_default().to_string();
    let pairs: Vec<(String, String)> =
        discover_crawl_suggestions(&derived, &focus_str, desired).await?;
    let urls = pairs.into_iter().map(|(url, _reason)| url).collect();
    Ok(SuggestResult { urls })
}
