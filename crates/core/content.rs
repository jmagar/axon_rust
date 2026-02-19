use super::http::http_client;
use spider::url::Url;
use spider::website::Website;
use spider_transformations::transformation::content::{
    transform_content_input, ReturnFormat, TransformConfig, TransformInput,
};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;

pub fn build_transform_config() -> TransformConfig {
    TransformConfig {
        return_format: ReturnFormat::Markdown,
        readability: true,
        clean_html: true,
        main_content: true,
        filter_images: true,
        filter_svg: true,
    }
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
    transform_content_input(input, &build_transform_config())
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
    // Lowercase once upfront to avoid repeated allocations.
    let lower = html.to_ascii_lowercase();
    let marker = "name=\"description\"";
    let idx = lower.find(marker)?;
    let content_idx = lower[idx..].find("content=\"")? + idx + "content=\"".len();
    let rest = &html[content_idx..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
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
    let mut out = Vec::new();
    let mut pos = 0usize;
    while let Some(start_rel) = xml[pos..].find("<loc>") {
        let start = pos + start_rel + 5;
        if let Some(end_rel) = xml[start..].find("</loc>") {
            let end = start + end_rel;
            let value = xml[start..end].trim();
            if !value.is_empty() {
                out.push(value.to_string());
            }
            pos = end + 6;
        } else {
            break;
        }
    }
    out
}

#[derive(Debug, Clone, Default)]
pub struct ExtractionMetrics {
    pub deterministic_pages: usize,
    pub llm_fallback_pages: usize,
    pub llm_requests: usize,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd: f64,
}

#[derive(Debug, Clone)]
pub struct ExtractRun {
    pub start_url: String,
    pub pages_visited: usize,
    pub pages_with_data: usize,
    pub results: Vec<serde_json::Value>,
    pub metrics: ExtractionMetrics,
    pub parser_hits: HashMap<String, usize>,
}

#[derive(Debug, Clone, Default)]
pub struct PageExtraction {
    pub items: Vec<serde_json::Value>,
    pub parser_hits: Vec<String>,
}

pub trait DeterministicParser: Send + Sync {
    fn name(&self) -> &'static str;
    fn parse(&self, page_url: &str, html: &str) -> Vec<serde_json::Value>;
}

#[derive(Default)]
pub struct DeterministicExtractionEngine {
    parsers: Vec<Box<dyn DeterministicParser>>,
}

impl DeterministicExtractionEngine {
    pub fn with_default_parsers() -> Self {
        let mut engine = Self::default();
        engine.register_parser(Box::new(JsonLdParser));
        engine.register_parser(Box::new(OpenGraphParser));
        engine.register_parser(Box::new(HtmlTableParser));
        engine
    }

    pub fn register_parser(&mut self, parser: Box<dyn DeterministicParser>) {
        self.parsers.push(parser);
    }

    pub fn extract(&self, page_url: &str, html: &str) -> PageExtraction {
        let mut all_items = Vec::new();
        let mut parser_hits = Vec::new();
        let mut seen = HashSet::new();

        for parser in &self.parsers {
            let items = parser.parse(page_url, html);
            if !items.is_empty() {
                parser_hits.push(parser.name().to_string());
                for item in items {
                    if let Ok(key) = serde_json::to_string(&item) {
                        if seen.insert(key) {
                            all_items.push(item);
                        }
                    }
                }
            }
        }

        PageExtraction {
            items: all_items,
            parser_hits,
        }
    }
}

struct JsonLdParser;

impl DeterministicParser for JsonLdParser {
    fn name(&self) -> &'static str {
        "json-ld"
    }

    fn parse(&self, page_url: &str, html: &str) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        let mut pos = 0usize;

        while let Some(rel) = html[pos..].find("<script") {
            let script_start = pos + rel;
            let Some(tag_end_rel) = html[script_start..].find('>') else {
                break;
            };
            let tag_end = script_start + tag_end_rel;
            let tag = &html[script_start..=tag_end];
            let tag_lower = tag.to_ascii_lowercase();

            if !tag_lower.contains("application/ld+json") {
                pos = tag_end + 1;
                continue;
            }

            let Some(close_rel) = html[tag_end + 1..].find("</script>") else {
                break;
            };
            let close = tag_end + 1 + close_rel;
            let raw_json = html[tag_end + 1..close].trim();

            if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw_json) {
                flatten_results(&value, &mut out);
            }

            pos = close + "</script>".len();
        }

        if out.is_empty() {
            return out;
        }

        out.into_iter()
            .map(|mut item| {
                if let Some(obj) = item.as_object_mut() {
                    obj.entry("_source_url".to_string())
                        .or_insert(serde_json::Value::String(page_url.to_string()));
                    obj.entry("_parser".to_string())
                        .or_insert(serde_json::Value::String(self.name().to_string()));
                }
                item
            })
            .collect()
    }
}

struct OpenGraphParser;

impl DeterministicParser for OpenGraphParser {
    fn name(&self) -> &'static str {
        "open-graph"
    }

    fn parse(&self, page_url: &str, html: &str) -> Vec<serde_json::Value> {
        let lower = html.to_ascii_lowercase();
        let mut og_fields = serde_json::Map::new();
        let mut pos = 0usize;

        while let Some(rel) = lower[pos..].find("<meta") {
            let start = pos + rel;
            let Some(end_rel) = lower[start..].find('>') else {
                break;
            };
            let end = start + end_rel;
            let tag = &html[start..=end];
            let tag_lower = &lower[start..=end];

            if tag_lower.contains("property=\"og:") || tag_lower.contains("name=\"og:") {
                let property = extract_attr(tag, "property")
                    .or_else(|| extract_attr(tag, "name"))
                    .unwrap_or_default();
                let content = extract_attr(tag, "content").unwrap_or_default();
                if !property.is_empty() && !content.is_empty() {
                    og_fields.insert(property, serde_json::Value::String(content));
                }
            }
            pos = end + 1;
        }

        if og_fields.is_empty() {
            return Vec::new();
        }

        og_fields.insert(
            "_source_url".to_string(),
            serde_json::Value::String(page_url.to_string()),
        );
        og_fields.insert(
            "_parser".to_string(),
            serde_json::Value::String(self.name().to_string()),
        );

        vec![serde_json::Value::Object(og_fields)]
    }
}

struct HtmlTableParser;

impl DeterministicParser for HtmlTableParser {
    fn name(&self) -> &'static str {
        "html-table"
    }

    fn parse(&self, page_url: &str, html: &str) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        let mut pos = 0usize;

        while let Some(rel) = html[pos..].find("<table") {
            let table_start = pos + rel;
            let Some(table_end_rel) = html[table_start..].find("</table>") else {
                break;
            };
            let table_end = table_start + table_end_rel + "</table>".len();
            let table_html = &html[table_start..table_end];
            let row_count = table_html.matches("<tr").count();
            if row_count > 0 {
                out.push(serde_json::json!({
                    "_parser": self.name(),
                    "_source_url": page_url,
                    "rows": row_count,
                    "html_preview": table_html.chars().take(500).collect::<String>(),
                }));
            }
            pos = table_end;
        }

        out
    }
}

fn extract_attr(tag: &str, attr_name: &str) -> Option<String> {
    let patterns = [
        format!("{attr_name}=\""),
        format!("{}='", attr_name),
        format!("{} = \"", attr_name),
        format!("{} = '", attr_name),
    ];

    for pattern in patterns {
        if let Some(idx) = tag.to_ascii_lowercase().find(&pattern.to_ascii_lowercase()) {
            let start = idx + pattern.len();
            let rest = &tag[start..];
            let quote = rest.chars().next()?;
            let end = rest[1..].find(quote)? + 1;
            return Some(rest[1..end].trim().to_string());
        }
    }

    None
}

fn flatten_results(value: &serde_json::Value, out: &mut Vec<serde_json::Value>) {
    if let Some(arr) = value.get("results").and_then(|v| v.as_array()) {
        out.extend(arr.iter().cloned());
        return;
    }

    match value {
        serde_json::Value::Array(arr) => out.extend(arr.iter().cloned()),
        serde_json::Value::Object(_) => out.push(value.clone()),
        _ => {}
    }
}

#[derive(Debug, Clone)]
struct FallbackResponse {
    items: Vec<serde_json::Value>,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    estimated_cost_usd: f64,
}

async fn extract_items_fallback(
    client: &reqwest::Client,
    api_url: &str,
    openai_api_key: &str,
    openai_model: &str,
    prompt: &str,
    page_url: &str,
    html: &str,
) -> Result<FallbackResponse, Box<dyn Error>> {
    let trimmed_html: String = html.chars().take(20_000).collect();
    let response = client
        .post(api_url)
        .bearer_auth(openai_api_key)
        .json(&serde_json::json!({
            "model": openai_model,
            "messages": [
                {
                    "role": "system",
                    "content": format!(
                        "{} Return JSON with a top-level key \"results\" containing an array of extracted items.",
                        prompt
                    )
                },
                {
                    "role": "user",
                    "content": format!("URL: {}\n\nHTML:\n{}", page_url, trimmed_html)
                }
            ],
            "response_format": {"type": "json_object"},
            "temperature": 0.1
        }))
        .send()
        .await?
        .error_for_status()?;

    let body: serde_json::Value = response.json().await?;
    let usage = body.get("usage").cloned().unwrap_or_default();

    let prompt_tokens = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let completion_tokens = usage
        .get("completion_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(prompt_tokens + completion_tokens);

    let content = body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("{}");
    let parsed = serde_json::from_str::<serde_json::Value>(content).unwrap_or_default();
    let mut items = Vec::new();
    flatten_results(&parsed, &mut items);

    Ok(FallbackResponse {
        items,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        estimated_cost_usd: estimate_llm_cost_usd(openai_model, prompt_tokens, completion_tokens),
    })
}

fn estimate_llm_cost_usd(model: &str, prompt_tokens: u64, completion_tokens: u64) -> f64 {
    // Pricing map is best-effort and intended for operational visibility.
    let model_lc = model.to_ascii_lowercase();
    let (input_per_million, output_per_million) = if model_lc.contains("gpt-4o-mini") {
        (0.15_f64, 0.60_f64)
    } else if model_lc.contains("gpt-4o") {
        (2.50_f64, 10.00_f64)
    } else if model_lc.contains("gpt-4.1-mini") {
        (0.40_f64, 1.60_f64)
    } else if model_lc.contains("gpt-4.1") {
        (2.00_f64, 8.00_f64)
    } else {
        (0.0_f64, 0.0_f64)
    };

    ((prompt_tokens as f64 / 1_000_000.0) * input_per_million)
        + ((completion_tokens as f64 / 1_000_000.0) * output_per_million)
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

    let api_key = openai_api_key.to_string();
    let model = openai_model.to_string();
    let prompt_text = prompt.to_string();

    let mut website = Website::new(start_url)
        .with_limit(limit)
        .build()
        .map_err(|_| "build website")?;

    let mut rx = website.subscribe(16).ok_or("subscribe failed")?;
    let client = http_client()?.clone();
    let engine_for_task = Arc::clone(&engine);

    let collect = tokio::spawn(async move {
        let mut all_results: Vec<serde_json::Value> = vec![];
        let mut pages_with_data = 0usize;
        let mut metrics = ExtractionMetrics::default();
        let mut parser_hits: HashMap<String, usize> = HashMap::new();

        while let Ok(page) = rx.recv().await {
            let page_url = page.get_url().to_string();
            let html = page.get_html();
            if html.is_empty() {
                continue;
            }

            let deterministic = engine_for_task.extract(&page_url, &html);
            if !deterministic.items.is_empty() {
                metrics.deterministic_pages += 1;
                pages_with_data += 1;
                all_results.extend(deterministic.items);
                for hit in deterministic.parser_hits {
                    *parser_hits.entry(hit).or_insert(0) += 1;
                }
                continue;
            }

            if !has_fallback {
                continue;
            }

            metrics.llm_fallback_pages += 1;
            metrics.llm_requests += 1;
            if let Ok(fallback) = extract_items_fallback(
                &client,
                &api_url,
                &api_key,
                &model,
                &prompt_text,
                &page_url,
                &html,
            )
            .await
            {
                metrics.prompt_tokens += fallback.prompt_tokens;
                metrics.completion_tokens += fallback.completion_tokens;
                metrics.total_tokens += fallback.total_tokens;
                metrics.estimated_cost_usd += fallback.estimated_cost_usd;
                if !fallback.items.is_empty() {
                    pages_with_data += 1;
                    all_results.extend(fallback.items);
                }
            }
        }

        (all_results, pages_with_data, metrics, parser_hits)
    });

    website.crawl_raw().await;
    website.unsubscribe();

    let (results, pages_with_data, metrics, parser_hits) = collect.await?;
    let pages_visited: usize = website.get_all_links_visited().await.len();

    Ok(ExtractRun {
        start_url: start_url.to_string(),
        pages_visited,
        pages_with_data,
        results,
        metrics,
        parser_hits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_url_postgres() {
        let url = "postgresql://axon:secret123@localhost:5432/axon";
        let redacted = redact_url(url);
        assert!(!redacted.contains("secret123"));
        assert!(redacted.contains("***"));
    }

    #[test]
    fn test_redact_url_amqp() {
        let url = "amqp://guest:guest@localhost:5672";
        let redacted = redact_url(url);
        assert!(!redacted.contains("guest:guest"));
    }

    #[test]
    fn test_redact_url_no_credentials() {
        let url = "http://example.com/path";
        assert_eq!(redact_url(url), url);
    }

    #[test]
    fn test_redact_url_unparseable() {
        // Should not panic, should return sentinel
        let result = redact_url("not a url at all !!!@#$");
        assert_eq!(result, "***redacted***");
    }

    #[test]
    fn test_redact_url_username_only() {
        let url = "postgresql://admin@localhost:5432/db";
        let redacted = redact_url(url);
        assert!(!redacted.contains("admin@"));
        assert!(redacted.contains("***"));
    }

    #[test]
    fn test_redact_url_redis_with_password() {
        let url = "redis://:mypassword@localhost:6379";
        let redacted = redact_url(url);
        assert!(!redacted.contains("mypassword"));
    }

    #[test]
    fn test_default_engine_extracts_json_ld() {
        let html = r#"
            <html><head>
            <script type=\"application/ld+json\">{"@type":"Article","headline":"Hello"}</script>
            </head></html>
        "#;
        let engine = DeterministicExtractionEngine::with_default_parsers();
        let page = engine.extract("https://example.com", html);
        assert!(!page.items.is_empty());
        assert!(page.parser_hits.iter().any(|x| x == "json-ld"));
    }

    #[test]
    fn test_estimate_llm_cost_usd_zero_for_unknown_model() {
        let cost = estimate_llm_cost_usd("unknown-model", 10_000, 1_000);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_estimate_llm_cost_usd_known_model() {
        let cost = estimate_llm_cost_usd("gpt-4o-mini", 100_000, 20_000);
        assert!(cost > 0.0);
    }
}
