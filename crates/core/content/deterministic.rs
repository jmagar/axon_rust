use html5gum::{Token, Tokenizer};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::hash::{Hash, Hasher};

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
        let mut seen_hashes: HashSet<u64> = HashSet::new();

        for parser in &self.parsers {
            let items = parser.parse(page_url, html);
            if !items.is_empty() {
                parser_hits.push(parser.name().to_string());
                for item in items {
                    if let Some(item_hash) = hash_json_value(&item) {
                        if seen_hashes.insert(item_hash) {
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

fn hash_json_value(value: &serde_json::Value) -> Option<u64> {
    let payload = serde_json::to_vec(value).ok()?;
    let mut hasher = DefaultHasher::new();
    payload.hash(&mut hasher);
    Some(hasher.finish())
}

struct JsonLdParser;

impl DeterministicParser for JsonLdParser {
    fn name(&self) -> &'static str {
        "json-ld"
    }

    fn parse(&self, page_url: &str, html: &str) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        let mut in_target_script = false;
        let mut current_json = String::new();

        for token in Tokenizer::new(html).infallible() {
            match token {
                Token::StartTag(tag) => {
                    if &tag.name[..] == b"script" {
                        if let Some(type_attr) = tag.attributes.get(&b"type"[..]) {
                            let type_str = String::from_utf8_lossy(type_attr).to_lowercase();
                            if type_str.contains("application/ld+json") {
                                in_target_script = true;
                                current_json.clear();
                            }
                        }
                    }
                }
                Token::String(s) => {
                    if in_target_script {
                        current_json.push_str(&String::from_utf8_lossy(&s));
                    }
                }
                Token::EndTag(tag) => {
                    if in_target_script && &tag.name[..] == b"script" {
                        in_target_script = false;
                        if let Ok(value) =
                            serde_json::from_str::<serde_json::Value>(current_json.trim())
                        {
                            flatten_results(&value, &mut out);
                        }
                    }
                }
                _ => {}
            }
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
        let mut og_fields = serde_json::Map::new();

        for token in Tokenizer::new(html).infallible() {
            if let Token::StartTag(tag) = token {
                if &tag.name[..] == b"meta" {
                    let mut property = None;
                    if let Some(prop) = tag.attributes.get(&b"property"[..]) {
                        property = Some(String::from_utf8_lossy(prop).into_owned());
                    } else if let Some(name) = tag.attributes.get(&b"name"[..]) {
                        property = Some(String::from_utf8_lossy(name).into_owned());
                    }

                    if let Some(prop) = property {
                        let prop_lower = prop.to_lowercase();
                        if prop_lower.starts_with("og:") {
                            if let Some(content_attr) = tag.attributes.get(&b"content"[..]) {
                                let content = String::from_utf8_lossy(content_attr).into_owned();
                                if !content.is_empty() {
                                    og_fields.insert(prop, serde_json::Value::String(content));
                                }
                            }
                        }
                    }
                }
            }
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
        let mut table_depth = 0;
        let mut row_count = 0;

        for token in Tokenizer::new(html).infallible() {
            match token {
                Token::StartTag(tag) => {
                    if &tag.name[..] == b"table" {
                        if table_depth == 0 {
                            row_count = 0;
                        }
                        table_depth += 1;
                    } else if &tag.name[..] == b"tr" && table_depth > 0 {
                        row_count += 1;
                    }
                }
                Token::EndTag(tag) => {
                    if &tag.name[..] == b"table" && table_depth > 0 {
                        table_depth -= 1;
                        if table_depth == 0 && row_count > 0 {
                            out.push(serde_json::json!({
                                "_parser": self.name(),
                                "_source_url": page_url,
                                "rows": row_count,
                            }));
                        }
                    }
                }
                _ => {}
            }
        }

        out
    }
}

pub(crate) fn flatten_results(value: &serde_json::Value, out: &mut Vec<serde_json::Value>) {
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
pub(crate) struct FallbackResponse {
    pub items: Vec<serde_json::Value>,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd: f64,
}

pub(crate) async fn extract_items_fallback(
    client: &reqwest::Client,
    api_url: &str,
    openai_api_key: &str,
    openai_model: &str,
    prompt: &str,
    page_url: &str,
    markdown: &str,
) -> Result<FallbackResponse, Box<dyn Error>> {
    let trimmed_markdown: String = markdown.chars().take(12_000).collect();
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
                    "content": format!("URL: {}\n\nContent (markdown):\n{}", page_url, trimmed_markdown)
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

pub(crate) fn estimate_llm_cost_usd(
    model: &str,
    prompt_tokens: u64,
    completion_tokens: u64,
) -> f64 {
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
