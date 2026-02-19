use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::http::{http_client, normalize_url};
use crate::axon_cli::crates::core::ui::{muted, primary};
use crate::axon_cli::crates::vector::ops_v2::{input, qdrant};
use spider::url::Url;
use std::collections::{HashMap, HashSet};
use std::env;
use std::error::Error;

fn env_usize_clamped(key: &str, default: usize, min: usize, max: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v >= min)
        .unwrap_or(default)
        .clamp(min, max)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Suggestion {
    url: String,
    reason: String,
}

fn parse_http_url(value: &str) -> Option<String> {
    let normalized = normalize_url(value.trim());
    let parsed = Url::parse(&normalized).ok()?;
    match parsed.scheme() {
        "http" | "https" => Some(parsed.to_string()),
        _ => None,
    }
}

fn parse_suggestions_from_llm(content: &str) -> Vec<Suggestion> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    let push =
        |out: &mut Vec<Suggestion>, seen: &mut HashSet<String>, url: String, reason: String| {
            if seen.insert(url.clone()) {
                out.push(Suggestion { url, reason });
            }
        };

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(content) {
        if let Some(items) = value.get("suggestions").and_then(|v| v.as_array()) {
            for item in items {
                if let Some(url) = item
                    .get("url")
                    .and_then(|v| v.as_str())
                    .and_then(parse_http_url)
                {
                    let reason = item
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Suggested by model")
                        .to_string();
                    push(&mut out, &mut seen, url, reason);
                } else if let Some(url) = item.as_str().and_then(parse_http_url) {
                    push(&mut out, &mut seen, url, "Suggested by model".to_string());
                }
            }
            return out;
        }
    }

    for token in content.split_whitespace() {
        let cleaned = token
            .trim_matches(|c: char| {
                matches!(
                    c,
                    '"' | '\'' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
                )
            })
            .trim_end_matches('.');
        if let Some(url) = parse_http_url(cleaned) {
            push(&mut out, &mut seen, url, "Suggested by model".to_string());
        }
    }

    out
}

fn already_indexed(url: &str, indexed_lookup: &HashSet<String>) -> bool {
    for variant in input::url_lookup_candidates(url) {
        if indexed_lookup.contains(&variant) {
            return true;
        }
    }
    false
}

pub async fn run_suggest_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if cfg.openai_base_url.trim().is_empty() || cfg.openai_model.trim().is_empty() {
        return Err("OPENAI_BASE_URL and OPENAI_MODEL required for suggest".into());
    }

    let desired = cfg.search_limit.clamp(1, 100);
    let base_url_context_limit = env_usize_clamped("AXON_SUGGEST_BASE_URL_LIMIT", 250, 10, 5_000);
    let existing_url_context_limit =
        env_usize_clamped("AXON_SUGGEST_EXISTING_URL_LIMIT", 500, 0, 5_000);

    let indexed_urls = qdrant::qdrant_indexed_urls(cfg).await?;
    if indexed_urls.is_empty() {
        return Err("No indexed URLs found in Qdrant collection; run crawl/scrape first".into());
    }

    let mut indexed_lookup = HashSet::new();
    for indexed in &indexed_urls {
        for variant in input::url_lookup_candidates(indexed) {
            indexed_lookup.insert(variant);
        }
    }

    let mut base_url_counts: HashMap<String, usize> = HashMap::new();
    for indexed in &indexed_urls {
        if let Some(base) = qdrant::base_url(indexed) {
            *base_url_counts.entry(base).or_insert(0) += 1;
        }
    }
    let mut ranked_base_urls: Vec<(String, usize)> = base_url_counts.into_iter().collect();
    ranked_base_urls.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let base_context = ranked_base_urls
        .iter()
        .take(base_url_context_limit)
        .map(|(base, pages)| format!("{base} (pages={pages})"))
        .collect::<Vec<_>>()
        .join("\n");
    let existing_url_context = indexed_urls
        .iter()
        .take(existing_url_context_limit)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");

    let focus = cfg
        .query
        .clone()
        .or_else(|| {
            if cfg.positional.is_empty() {
                None
            } else {
                Some(cfg.positional.join(" "))
            }
        })
        .unwrap_or_default();

    let user_prompt = format!(
        "You are helping expand a documentation crawl set.\n\
Return STRICT JSON only in this shape:\n\
{{\"suggestions\":[{{\"url\":\"https://...\",\"reason\":\"...\"}}]}}\n\n\
Rules:\n\
- Provide exactly {desired} suggestions.\n\
- Suggest docs/reference/changelog/API/help URLs likely to complement the indexed base URLs.\n\
- Do not suggest any URL from ALREADY_INDEXED_URLS.\n\
- Prefer URLs likely to be crawl entrypoints or high-value docs pages.\n\
- Use only absolute http/https URLs.\n\n\
Focus (optional): {focus}\n\n\
INDEXED_BASE_URLS_WITH_PAGE_COUNTS:\n{base_context}\n\n\
ALREADY_INDEXED_URLS:\n{existing_url_context}"
    );

    let client = http_client()?;
    let mut req = client
        .post(format!(
            "{}/chat/completions",
            cfg.openai_base_url.trim_end_matches('/')
        ))
        .json(&serde_json::json!({
            "model": cfg.openai_model,
            "messages": [
                {"role": "system", "content": "You propose complementary documentation crawl targets. Output JSON only."},
                {"role": "user", "content": user_prompt}
            ],
            "temperature": 0.2,
        }));

    if !cfg.openai_api_key.trim().is_empty() {
        req = req.bearer_auth(&cfg.openai_api_key);
    }

    let response = req.send().await?.error_for_status()?;
    let json: serde_json::Value = response.json().await?;
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("");

    let parsed = parse_suggestions_from_llm(content);
    let mut accepted = Vec::new();
    let mut rejected_existing = Vec::new();
    let mut accepted_seen = HashSet::new();

    for suggestion in parsed {
        if already_indexed(&suggestion.url, &indexed_lookup) {
            rejected_existing.push(suggestion.url);
            continue;
        }
        if accepted_seen.insert(suggestion.url.clone()) {
            accepted.push(suggestion);
        }
        if accepted.len() >= desired {
            break;
        }
    }

    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "collection": cfg.collection,
                "requested": desired,
                "indexed_urls_count": indexed_urls.len(),
                "indexed_base_urls_count": ranked_base_urls.len(),
                "suggestions": accepted.iter().map(|s| serde_json::json!({"url": s.url, "reason": s.reason})).collect::<Vec<_>>(),
                "rejected_existing": rejected_existing,
                "raw_model_output": content,
            }))?
        );
        return Ok(());
    }

    println!("{}", primary("Suggested Crawl Targets"));
    println!(
        "  {} requested={} accepted={} filtered_existing={}",
        muted("Summary:"),
        desired,
        accepted.len(),
        rejected_existing.len()
    );
    for (idx, suggestion) in accepted.iter().enumerate() {
        println!("  {}. {}", idx + 1, suggestion.url);
        println!("     {}", muted(&suggestion.reason));
    }
    if accepted.is_empty() {
        println!(
            "  {}",
            muted("No new URLs survived filtering. Retry with a different focus or higher model temperature.")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{already_indexed, parse_suggestions_from_llm};
    use std::collections::HashSet;

    #[test]
    fn parses_json_suggestions() {
        let input = r#"{
          "suggestions": [
            {"url":"https://docs.example.com/getting-started","reason":"Core onboarding guide"},
            {"url":"https://api.example.com/reference","reason":"API endpoint docs"}
          ]
        }"#;
        let parsed = parse_suggestions_from_llm(input);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].url, "https://docs.example.com/getting-started");
        assert_eq!(parsed[1].url, "https://api.example.com/reference");
    }

    #[test]
    fn parses_url_tokens_when_json_is_missing() {
        let input = "Try https://docs.rs/spider and https://doc.rust-lang.org/book/.";
        let parsed = parse_suggestions_from_llm(input);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].url, "https://docs.rs/spider");
        assert_eq!(parsed[1].url, "https://doc.rust-lang.org/book/");
    }

    #[test]
    fn rejects_already_indexed_url_variants() {
        let mut indexed = HashSet::new();
        indexed.insert("https://docs.example.com/guide".to_string());
        assert!(already_indexed("https://docs.example.com/guide/", &indexed));
        assert!(!already_indexed(
            "https://docs.example.com/changelog",
            &indexed
        ));
    }
}
