use super::deterministic::{
    DeterministicExtractionEngine, ExtractRun, FallbackResponse, extract_items_fallback,
};
use super::{ExtractionMetrics, to_markdown};
use crate::crates::core::http::{http_client, ssrf_blacklist_patterns, validate_url};
use crate::crates::core::logging::log_warn;
use spider::website::Website;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const FALLBACK_CONCURRENCY_LIMIT: usize = 4;

/// Parse `"Key: Value"` header strings into a `HeaderMap`.
///
/// TODO: Extract a shared version of this into `crates/core/http.rs` and use
/// it here, in `scrape.rs`, and in `crawl/engine/runtime.rs` to eliminate the
/// duplicated parsing logic across all three call sites.
fn parse_custom_headers(raw_headers: &[String]) -> reqwest::header::HeaderMap {
    let mut map = reqwest::header::HeaderMap::new();
    for raw in raw_headers {
        if let Some((k, v)) = raw.split_once(": ") {
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                reqwest::header::HeaderValue::from_str(v),
            ) {
                map.insert(name, val);
            }
        }
    }
    map
}

/// Configuration bundle for `run_extract_with_engine`.
///
/// Replaces the previous 7-param function signature with a single struct,
/// making it easy to add new fields (e.g. `custom_headers`) without churn.
pub struct ExtractWebConfig {
    pub start_url: String,
    pub prompt: String,
    pub limit: u32,
    pub openai_base_url: String,
    pub openai_api_key: String,
    pub openai_model: String,
    /// Custom HTTP headers in `"Key: Value"` format, passed through to spider.
    pub custom_headers: Vec<String>,
}

struct FallbackConfig {
    api_url: String,
    api_key: String,
    model: String,
    prompt_text: String,
    has_fallback: bool,
}

struct PageCollectResult {
    results: Vec<serde_json::Value>,
    pages_visited: usize,
    pages_with_data: usize,
    metrics: ExtractionMetrics,
    parser_hits: HashMap<String, usize>,
}

async fn collect_page_results(
    mut rx: tokio::sync::broadcast::Receiver<spider::page::Page>,
    client: reqwest::Client,
    engine: Arc<DeterministicExtractionEngine>,
    cfg: FallbackConfig,
) -> PageCollectResult {
    let mut all_results: Vec<serde_json::Value> = vec![];
    let mut pages_visited = 0usize;
    let mut pages_with_data = 0usize;
    let mut metrics = ExtractionMetrics::default();
    let mut parser_hits: HashMap<String, usize> = HashMap::new();
    let fallback_limiter = Arc::new(Semaphore::new(FALLBACK_CONCURRENCY_LIMIT));
    let mut fallback_tasks: JoinSet<(String, Result<FallbackResponse, String>)> = JoinSet::new();

    loop {
        let page = match rx.recv().await {
            Ok(page) => page,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                log_warn(&format!(
                    "broadcast receiver lagged, skipped {n} pages — consider increasing buffer"
                ));
                continue;
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        };
        pages_visited += 1;
        let page_url = page.get_url().to_string();
        let html = page.get_html();
        if html.is_empty() {
            continue;
        }
        let deterministic = engine.extract(&page_url, &html);
        if !deterministic.items.is_empty() {
            metrics.deterministic_pages += 1;
            pages_with_data += 1;
            all_results.extend(deterministic.items);
            for hit in deterministic.parser_hits {
                *parser_hits.entry(hit).or_insert(0) += 1;
            }
            continue;
        }
        if !cfg.has_fallback {
            continue;
        }
        metrics.llm_fallback_pages += 1;
        metrics.llm_requests += 1;
        let api_url_c = cfg.api_url.clone();
        let api_key_c = cfg.api_key.clone();
        let model_c = cfg.model.clone();
        let prompt_c = cfg.prompt_text.clone();
        let client_c = client.clone();
        let limiter = Arc::clone(&fallback_limiter);
        let html_owned = html.to_string();
        fallback_tasks.spawn(async move {
            let _permit = match limiter.acquire_owned().await {
                Ok(p) => p,
                Err(_) => {
                    return (page_url, Err("fallback limiter closed".to_string()));
                }
            };
            let markdown = to_markdown(&html_owned);
            let res = extract_items_fallback(
                &client_c, &api_url_c, &api_key_c, &model_c, &prompt_c, &page_url, &markdown,
            )
            .await
            .map_err(|e| e.to_string());
            (page_url, res)
        });
        while let Some(joined) = fallback_tasks.try_join_next() {
            drain_fallback_result(joined, &mut pages_with_data, &mut all_results, &mut metrics);
        }
    }
    while let Some(joined) = fallback_tasks.join_next().await {
        drain_fallback_result(joined, &mut pages_with_data, &mut all_results, &mut metrics);
    }
    PageCollectResult {
        results: all_results,
        pages_visited,
        pages_with_data,
        metrics,
        parser_hits,
    }
}

fn drain_fallback_result(
    joined: Result<(String, Result<FallbackResponse, String>), tokio::task::JoinError>,
    pages_with_data: &mut usize,
    all_results: &mut Vec<serde_json::Value>,
    metrics: &mut ExtractionMetrics,
) {
    match joined {
        Ok((url, Ok(fallback))) => {
            metrics.prompt_tokens += fallback.prompt_tokens;
            metrics.completion_tokens += fallback.completion_tokens;
            metrics.total_tokens += fallback.total_tokens;
            metrics.estimated_cost_usd += fallback.estimated_cost_usd;
            let has_items = !fallback.items.is_empty();
            if has_items {
                *pages_with_data += 1;
                all_results.extend(fallback.items);
            }
            if !has_items {
                log_warn(&format!("fallback extraction produced no items for {url}"));
            }
        }
        Ok((url, Err(err))) => {
            log_warn(&format!("fallback extraction failed for {url}: {err}"));
        }
        Err(err) => {
            log_warn(&format!("fallback extraction task join error: {err}"));
        }
    }
}

pub async fn run_extract_with_engine(
    wcfg: ExtractWebConfig,
    engine: Arc<DeterministicExtractionEngine>,
) -> Result<ExtractRun, Box<dyn Error>> {
    let api_url = format!(
        "{}/chat/completions",
        wcfg.openai_base_url.trim_end_matches('/')
    );
    let has_fallback = !wcfg.openai_base_url.is_empty()
        && !wcfg.openai_api_key.is_empty()
        && !wcfg.openai_model.is_empty()
        && wcfg.openai_base_url.starts_with("http");

    validate_url(&wcfg.start_url)?;
    let ssrf_patterns: Vec<spider::compact_str::CompactString> = ssrf_blacklist_patterns()
        .iter()
        .copied()
        .map(Into::into)
        .collect();
    let mut website = Website::new(&wcfg.start_url);
    website.with_limit(wcfg.limit);
    website.with_blacklist_url(Some(ssrf_patterns));
    // Wire custom headers so `--header` applies to extract crawls too.
    // TODO: Extract a shared `parse_custom_headers(&[String]) -> HeaderMap` helper
    // in `crates/core/http.rs` and use it here, in `scrape.rs`, and in
    // `crawl/engine/runtime.rs` to eliminate this duplicated parsing logic.
    if !wcfg.custom_headers.is_empty() {
        let map = parse_custom_headers(&wcfg.custom_headers);
        if !map.is_empty() {
            website.with_headers(Some(map));
        }
    }
    let mut website = website.build().map_err(|_| "build website")?;

    let rx = website.subscribe(16).ok_or("subscribe failed")?;
    let fallback_cfg = FallbackConfig {
        api_url,
        api_key: wcfg.openai_api_key,
        model: wcfg.openai_model,
        prompt_text: wcfg.prompt,
        has_fallback,
    };
    let collect = tokio::spawn(collect_page_results(
        rx,
        http_client()?.clone(),
        Arc::clone(&engine),
        fallback_cfg,
    ));

    website.crawl_raw().await;
    website.unsubscribe();

    let PageCollectResult {
        results,
        pages_visited,
        pages_with_data,
        metrics,
        parser_hits,
    } = collect.await?;
    Ok(ExtractRun {
        start_url: wcfg.start_url,
        pages_visited,
        pages_with_data,
        results,
        metrics,
        parser_hits,
    })
}
