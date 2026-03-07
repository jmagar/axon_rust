use crate::crates::core::config::Config;
use crate::crates::core::http::{http_client, normalize_url};
use crate::crates::core::ui::{muted, primary};
use crate::crates::vector::ops::{input, qdrant};
use spider::url::Url;
use std::collections::HashSet;
use std::error::Error;

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

fn suggestion_score(url: &str) -> i32 {
    let parsed = Url::parse(url).ok();
    let Some(parsed) = parsed else {
        return 0;
    };
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    let path = parsed.path().to_ascii_lowercase();
    let full = format!("{host}{path}");
    let mut score = 0i32;

    let high_value = [
        "docs",
        "reference",
        "api",
        "guide",
        "manual",
        "changelog",
        "release",
        "help",
        "kb",
    ];
    if high_value.iter().any(|k| full.contains(k)) {
        score += 4;
    }
    if path == "/" || path.is_empty() {
        score += 1;
    }
    let depth = path.split('/').filter(|s| !s.is_empty()).count();
    if (1..=4).contains(&depth) {
        score += 2;
    }
    if parsed.query().is_some() {
        score -= 2;
    }
    let low_value = [
        "privacy", "terms", "careers", "press", "blog", "news", "about",
    ];
    if low_value.iter().any(|k| path.contains(k)) {
        score -= 3;
    }
    let binary_suffixes = [".zip", ".gz", ".tar", ".exe", ".dmg"];
    if binary_suffixes.iter().any(|s| path.ends_with(s)) {
        score -= 6;
    }
    score
}

fn host_of(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default()
}

struct SuggestPromptContext {
    desired: usize,
    indexed_urls: Vec<String>,
    indexed_lookup: HashSet<String>,
    ranked_base_urls: Vec<(String, usize)>,
    focus: String,
    base_context: String,
    existing_url_context: String,
}

fn suggestion_focus(cfg: &Config) -> String {
    super::resolve_query_text(cfg).unwrap_or_default()
}

async fn build_suggest_prompt_context(
    cfg: &Config,
    focus: &str,
    desired: usize,
) -> Result<SuggestPromptContext, Box<dyn Error>> {
    let base_url_context_limit =
        qdrant::env_usize_clamped("AXON_SUGGEST_BASE_URL_LIMIT", 250, 10, 5_000);
    let existing_url_context_limit =
        qdrant::env_usize_clamped("AXON_SUGGEST_EXISTING_URL_LIMIT", 500, 0, 5_000);
    let index_dedup_limit =
        qdrant::env_usize_clamped("AXON_SUGGEST_INDEX_LIMIT", 50_000, 100, 500_000);

    // Fetch indexed URLs for duplicate filtering (capped to avoid full-collection scan).
    let (indexed_urls, mut ranked_base_urls) = spider::tokio::try_join!(
        qdrant::qdrant_indexed_urls(cfg, Some(index_dedup_limit)),
        qdrant::qdrant_domain_facets(cfg, base_url_context_limit),
    )?;

    if indexed_urls.is_empty() {
        return Err("No indexed URLs found in Qdrant collection; run crawl/scrape first".into());
    }

    // Build lookup set: stored URLs are already normalised, so only slash variants needed.
    let mut indexed_lookup = HashSet::with_capacity(indexed_urls.len() * 2);
    for indexed in &indexed_urls {
        let without_slash = indexed.trim_end_matches('/');
        indexed_lookup.insert(without_slash.to_string());
        indexed_lookup.insert(format!("{without_slash}/"));
    }

    // Domain facets come back alphabetically sorted; re-sort by page count descending.
    ranked_base_urls.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let base_context = ranked_base_urls
        .iter()
        .map(|(domain, pages)| format!("{domain} (pages={pages})"))
        .collect::<Vec<_>>()
        .join("\n");
    let existing_url_context = indexed_urls
        .iter()
        .take(existing_url_context_limit)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    Ok(SuggestPromptContext {
        desired,
        indexed_urls,
        indexed_lookup,
        ranked_base_urls,
        focus: focus.to_string(),
        base_context,
        existing_url_context,
    })
}

fn build_suggest_user_prompt(ctx: &SuggestPromptContext) -> String {
    let user_prompt = format!(
        "You are helping expand a documentation crawl set.\n\
Return STRICT JSON only in this shape:\n\
{{\"suggestions\":[{{\"url\":\"https://...\",\"reason\":\"...\"}}]}}\n\n\
Rules:\n\
- Provide exactly {} suggestions.\n\
- Suggest docs/reference/changelog/API/help URLs likely to complement the indexed base URLs.\n\
- Do not suggest any URL from ALREADY_INDEXED_URLS.\n\
- Prefer URLs likely to be crawl entrypoints or high-value docs pages.\n\
- Use only absolute http/https URLs.\n\n\
Focus (optional): {}\n\n\
INDEXED_BASE_URLS_WITH_PAGE_COUNTS:\n{}\n\n\
ALREADY_INDEXED_URLS:\n{}",
        ctx.desired, ctx.focus, ctx.base_context, ctx.existing_url_context
    );
    user_prompt
}

async fn request_suggestions_from_llm(
    cfg: &Config,
    user_prompt: &str,
) -> Result<String, Box<dyn Error>> {
    let client = http_client()?;
    let req = super::streaming::build_openai_chat_request(client, cfg).json(&serde_json::json!({
        "model": cfg.openai_model,
        "messages": [
            {"role": "system", "content": "You propose complementary documentation crawl targets. Output JSON only."},
            {"role": "user", "content": user_prompt}
        ],
        "temperature": 0.2,
    }));

    let response = req.send().await?.error_for_status()?;
    let json: serde_json::Value = response.json().await?;
    Ok(json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string())
}

fn filter_new_suggestions(
    content: &str,
    indexed_lookup: &HashSet<String>,
    desired: usize,
) -> (Vec<Suggestion>, Vec<String>) {
    let parsed = parse_suggestions_from_llm(content);
    let mut accepted = Vec::new();
    let mut rejected_existing = Vec::new();
    let mut accepted_seen = HashSet::new();

    for suggestion in parsed {
        if already_indexed(&suggestion.url, indexed_lookup) {
            rejected_existing.push(suggestion.url);
            continue;
        }
        if accepted_seen.insert(suggestion.url.clone()) {
            accepted.push(suggestion);
        }
    }

    accepted.sort_by(|a, b| {
        suggestion_score(&b.url)
            .cmp(&suggestion_score(&a.url))
            .then_with(|| a.url.cmp(&b.url))
    });
    let mut diversified = Vec::new();
    let mut used_hosts = HashSet::new();
    for suggestion in &accepted {
        let host = host_of(&suggestion.url);
        if host.is_empty() {
            continue;
        }
        if used_hosts.insert(host) {
            diversified.push(suggestion.clone());
            if diversified.len() >= desired {
                break;
            }
        }
    }
    if diversified.len() < desired {
        let mut seen_urls = diversified
            .iter()
            .map(|s| s.url.clone())
            .collect::<HashSet<_>>();
        for suggestion in &accepted {
            if seen_urls.insert(suggestion.url.clone()) {
                diversified.push(suggestion.clone());
            }
            if diversified.len() >= desired {
                break;
            }
        }
    }
    (diversified, rejected_existing)
}

fn emit_suggest_output(
    cfg: &Config,
    ctx: &SuggestPromptContext,
    accepted: &[Suggestion],
    rejected_existing: &[String],
    content: &str,
) -> Result<(), Box<dyn Error>> {
    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "collection": cfg.collection,
                "requested": ctx.desired,
                "indexed_urls_count": ctx.indexed_urls.len(),
                "indexed_base_urls_count": ctx.ranked_base_urls.len(),
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
        ctx.desired,
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
            muted(
                "No new URLs survived filtering. Retry with a different focus or higher model temperature."
            )
        );
    }
    Ok(())
}

pub async fn run_suggest_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if cfg.openai_base_url.trim().is_empty() || cfg.openai_model.trim().is_empty() {
        return Err("OPENAI_BASE_URL and OPENAI_MODEL required for suggest".into());
    }
    let desired = cfg.search_limit.clamp(1, 100);
    let focus = suggestion_focus(cfg);
    let (accepted, rejected_existing, content, ctx) =
        discover_suggestions_with_context(cfg, &focus, desired).await?;
    emit_suggest_output(cfg, &ctx, &accepted, &rejected_existing, &content)
}

async fn discover_suggestions_with_context(
    cfg: &Config,
    focus: &str,
    desired: usize,
) -> Result<(Vec<Suggestion>, Vec<String>, String, SuggestPromptContext), Box<dyn Error>> {
    let ctx = build_suggest_prompt_context(cfg, focus, desired).await?;
    let user_prompt = build_suggest_user_prompt(&ctx);
    let content = request_suggestions_from_llm(cfg, &user_prompt).await?;
    let (accepted, rejected_existing) =
        filter_new_suggestions(&content, &ctx.indexed_lookup, desired);
    Ok((accepted, rejected_existing, content, ctx))
}

pub async fn discover_crawl_suggestions(
    cfg: &Config,
    focus: &str,
    desired: usize,
) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let desired = desired.clamp(1, 100);
    let (accepted, _, _, _) = discover_suggestions_with_context(cfg, focus, desired).await?;
    Ok(accepted
        .into_iter()
        .map(|s| (s.url, s.reason))
        .collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::{already_indexed, filter_new_suggestions, parse_suggestions_from_llm};
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

    #[test]
    fn filter_prefers_high_value_urls_and_diversifies_hosts() {
        let mut indexed = HashSet::new();
        indexed.insert("https://docs.a.com/old".to_string());
        let content = r#"{
          "suggestions": [
            {"url":"https://a.com/privacy","reason":"low value"},
            {"url":"https://docs.a.com/reference/api","reason":"high value"},
            {"url":"https://docs.b.com/guide","reason":"high value"},
            {"url":"https://a.com/news","reason":"low value"}
          ]
        }"#;
        let (accepted, _rejected) = filter_new_suggestions(content, &indexed, 2);
        assert_eq!(accepted.len(), 2);
        assert_eq!(accepted[0].url, "https://docs.a.com/reference/api");
        assert_eq!(accepted[1].url, "https://docs.b.com/guide");
    }
}
