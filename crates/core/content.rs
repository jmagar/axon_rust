mod deterministic;

#[cfg(test)]
mod tests;

pub use deterministic::{
    DeterministicExtractionEngine, DeterministicParser, ExtractRun, ExtractionMetrics,
    PageExtraction,
};

use super::http::{http_client, ssrf_blacklist_patterns, validate_url};
use super::logging::log_warn;
use deterministic::{extract_items_fallback, FallbackResponse};
use spider::url::Url;
use spider::website::Website;
use spider_transformations::transformation::content::{
    transform_content_input, ReturnFormat, TransformConfig, TransformInput,
};
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, LazyLock};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

static TRANSFORM_CONFIG: LazyLock<TransformConfig> = LazyLock::new(|| TransformConfig {
    return_format: ReturnFormat::Markdown,
    // Readability (Mozilla-style article scoring) discards documentation pages
    // that lack <article> structure — doc sites with sidebar + nested divs score
    // too low and get stripped to just the title. main_content=true already
    // extracts <main>/<article>/role=main structurally without the scoring penalty.
    readability: false,
    // clean_html uses [class*='ad'] which matches Tailwind `shadow-*` classes
    // (sh**ad**ow contains "ad"). This wipes all shadow-styled elements from
    // Tailwind CSS sites (react.dev, shadcn.com, etc.), leaving only the title.
    // html2md ignores script/style content natively, so clean_html buys nothing.
    clean_html: false,
    main_content: true,
    filter_images: true,
    filter_svg: true,
});
const FALLBACK_CONCURRENCY_LIMIT: usize = 4;

pub fn build_transform_config() -> &'static TransformConfig {
    &TRANSFORM_CONFIG
}

pub fn to_markdown(html: &str) -> String {
    let input = TransformInput {
        url: None,
        content: html.as_bytes(),
        screenshot_bytes: None,
        encoding: None,
        selector_config: None,
        ignore_tags: None,
    };
    transform_content_input(input, &TRANSFORM_CONFIG)
        .trim()
        .to_string()
}

/// Redact credentials from a URL, replacing username and password with `***`.
/// Returns `"***redacted***"` if the URL cannot be parsed.
pub fn redact_url(url: &str) -> String {
    match Url::parse(url) {
        Ok(mut parsed) => {
            if !parsed.username().is_empty() || parsed.password().is_some() {
                let _ = parsed.set_username("***");
                let _ = parsed.set_password(Some("***"));
            }
            parsed.to_string()
        }
        Err(_) => "***redacted***".to_string(),
    }
}

pub fn url_to_filename(url: &str, idx: u32) -> String {
    let parsed = Url::parse(url).ok();
    let host = parsed
        .as_ref()
        .and_then(|u| u.host_str())
        .unwrap_or("unknown-host");
    let path = parsed.as_ref().map(|u| u.path()).unwrap_or("/unknown-path");

    let stem_raw = format!("{host}{path}");
    let stem: String = stem_raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .take(80)
        .collect();

    format!("{:04}-{stem}.md", idx)
}

pub fn find_between<'a>(haystack: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let s = haystack.find(start)? + start.len();
    let e = haystack[s..].find(end)? + s;
    Some(haystack[s..e].trim())
}

pub fn extract_meta_description(html: &str) -> Option<String> {
    // Limit search to <head> (≤8 KB) to avoid cloning the full document.
    let head_end = html
        .find("</head>")
        .or_else(|| html.find("</HEAD>"))
        .unwrap_or(html.len().min(8192));
    // Use .get() instead of direct index to avoid a panic when head_end falls
    // on a UTF-8 multi-byte boundary (possible when the 8192-byte default is used).
    let head = html.get(..head_end).unwrap_or(html);
    let lower = head.to_ascii_lowercase();
    let marker = "name=\"description\"";
    let idx = lower.find(marker)?;
    let content_idx = lower[idx..].find("content=\"")? + idx + "content=\"".len();
    let rest = head.get(content_idx..)?;
    let end = rest.find('"')?;
    Some(rest.get(..end)?.to_string())
}

pub fn extract_links(html: &str, limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while let Some(rel) = html[pos..].find("href=\"") {
        let start = pos + rel + 6;
        let remain = &html[start..];
        let Some(end_rel) = remain.find('"') else {
            break;
        };
        let link = remain[..end_rel].trim();
        if (link.starts_with("http://") || link.starts_with("https://"))
            && !out.iter().any(|x| x == link)
        {
            out.push(link.to_string());
            if out.len() >= limit {
                break;
            }
        }
        pos = start + end_rel + 1;
    }
    out
}

pub fn extract_loc_values(xml: &str) -> Vec<String> {
    // Case-insensitive search without cloning the full document (which can be 1–5 MB).
    // The sitemap spec mandates lowercase, but real-world feeds sometimes use <LOC>.
    const OPEN: &[u8] = b"<loc>";
    const CLOSE: &[u8] = b"</loc>";
    let bytes = xml.as_bytes();
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while cursor + OPEN.len() <= bytes.len() {
        let Some(rel) = bytes[cursor..]
            .windows(OPEN.len())
            .position(|w| w.eq_ignore_ascii_case(OPEN))
        else {
            break;
        };
        let start_idx = cursor + rel + OPEN.len();
        let Some(end_rel) = bytes[start_idx..]
            .windows(CLOSE.len())
            .position(|w| w.eq_ignore_ascii_case(CLOSE))
        else {
            break;
        };
        let end_idx = start_idx + end_rel;
        let value = xml[start_idx..end_idx].trim();
        if !value.is_empty() {
            out.push(value.replace("&amp;", "&"));
        }
        cursor = end_idx + CLOSE.len();
    }
    out
}

pub fn normalize_prefix(prefix: &str) -> Option<String> {
    let trimmed = prefix.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return None;
    }
    let mut value = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };
    if value.len() > 1 && value.ends_with('/') {
        value.truncate(value.len() - 1);
    }
    Some(value)
}

pub fn is_excluded_url_path(url: &str, prefixes: &[String]) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    let path = parsed.path();
    prefixes.iter().any(|raw| {
        // Inline normalize_prefix logic without allocating — prefixes are pre-validated
        // at config time so the hot path is the common case (already has leading slash).
        let p = raw.trim().trim_end_matches('/');
        if p.is_empty() || p == "/" {
            return false;
        }
        // Common case: prefix already has leading slash (no allocation needed).
        if p.starts_with('/') {
            return path == p
                || (path.starts_with(p) && path.as_bytes().get(p.len()) == Some(&b'/'));
        }
        // Rare case: prefix lacks leading slash — compare with implicit "/".
        path == format!("/{p}")
            || path.starts_with('/')
                && path[1..].starts_with(p)
                && path.as_bytes().get(p.len() + 1) == Some(&b'/')
    })
}

pub fn canonicalize_url(url: &str) -> Option<String> {
    let mut parsed = Url::parse(url).ok()?;
    // Strip fragment
    parsed.set_fragment(None);
    // Strip default ports to prevent duplicate entries
    // (http://x:80/p and http://x/p must deduplicate)
    match (parsed.scheme(), parsed.port()) {
        ("http", Some(80)) | ("https", Some(443)) => {
            let _ = parsed.set_port(None);
        }
        _ => {}
    }
    // Strip trailing slashes from all paths (not just root)
    let path = parsed.path().to_string();
    if path.len() > 1 && path.ends_with('/') {
        parsed.set_path(path.trim_end_matches('/'));
    }
    Some(parsed.to_string())
}

pub fn extract_robots_sitemaps(robots_txt: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in robots_txt.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("sitemap") {
            continue;
        }
        let url = value.trim();
        if !url.is_empty() {
            out.push(url.to_string());
        }
    }
    out.sort();
    out.dedup();
    out
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
    mut rx: spider::tokio::sync::broadcast::Receiver<spider::page::Page>,
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
            Err(spider::tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                log_warn(&format!(
                    "broadcast receiver lagged, skipped {n} pages — consider increasing buffer"
                ));
                continue;
            }
            Err(spider::tokio::sync::broadcast::error::RecvError::Closed) => break,
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
    start_url: &str,
    prompt: &str,
    limit: u32,
    openai_base_url: &str,
    openai_api_key: &str,
    openai_model: &str,
    engine: Arc<DeterministicExtractionEngine>,
) -> Result<ExtractRun, Box<dyn Error>> {
    let api_url = format!("{}/chat/completions", openai_base_url.trim_end_matches('/'));
    let has_fallback = !openai_base_url.is_empty()
        && !openai_api_key.is_empty()
        && !openai_model.is_empty()
        && openai_base_url.starts_with("http");

    validate_url(start_url)?;
    let ssrf_patterns: Vec<spider::compact_str::CompactString> = ssrf_blacklist_patterns()
        .into_iter()
        .map(Into::into)
        .collect();
    let mut website = Website::new(start_url);
    website.with_limit(limit);
    website.with_blacklist_url(Some(ssrf_patterns));
    let mut website = website.build().map_err(|_| "build website")?;

    let rx = website.subscribe(16).ok_or("subscribe failed")?;
    let fallback_cfg = FallbackConfig {
        api_url,
        api_key: openai_api_key.to_string(),
        model: openai_model.to_string(),
        prompt_text: prompt.to_string(),
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
        start_url: start_url.to_string(),
        pages_visited,
        pages_with_data,
        results,
        metrics,
        parser_hits,
    })
}
