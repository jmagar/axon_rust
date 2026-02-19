use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::http::{http_client, normalize_url};
use crate::axon_cli::crates::core::ui::{muted, primary};
use crate::axon_cli::crates::vector::ops_v2::{input, qdrant, ranking, tei};
use futures_util::stream::{self, StreamExt};
use spider::url::Url;
use std::collections::{HashMap, HashSet};
use std::env;
use std::error::Error;
use std::io::Write;
use std::sync::Arc;

pub async fn run_query_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let query = cfg
        .query
        .clone()
        .or_else(|| {
            if cfg.positional.is_empty() {
                None
            } else {
                Some(cfg.positional.join(" "))
            }
        })
        .ok_or("query requires text")?;

    let mut query_vectors = tei::tei_embed(cfg, std::slice::from_ref(&query)).await?;
    if query_vectors.is_empty() {
        return Err("TEI returned no vector for query".into());
    }
    let vector = query_vectors.remove(0);
    let hits = qdrant::qdrant_search(cfg, &vector, cfg.search_limit.max(1)).await?;

    if !cfg.json_output {
        println!("{}", primary(&format!("Query Results for \"{query}\"")));
        println!("{} {}\n", muted("Showing"), hits.len());
    }

    for (i, h) in hits.iter().enumerate() {
        let score = h.score;
        let payload = &h.payload;
        let url = qdrant::payload_url_typed(payload);
        let snippet = qdrant::query_snippet(payload);
        if cfg.json_output {
            println!(
                "{}",
                serde_json::json!({"rank": i + 1, "score": score, "url": url, "snippet": snippet})
            );
        } else {
            println!(
                "  • {}. {} [{:.2}] {}",
                i + 1,
                crate::axon_cli::crates::core::ui::status_text("completed"),
                score,
                crate::axon_cli::crates::core::ui::accent(url)
            );
            println!("    {}", snippet);
        }
    }

    Ok(())
}

fn env_usize_clamped(key: &str, default: usize, min: usize, max: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v >= min)
        .unwrap_or(default)
        .clamp(min, max)
}

fn extract_sse_token(data: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(data).ok()?;
    value["choices"][0]["delta"]["content"]
        .as_str()
        .or_else(|| value["choices"][0]["message"]["content"].as_str())
        .or_else(|| value["choices"][0]["text"].as_str())
        .map(str::to_string)
}

fn process_sse_line(
    line: &str,
    answer: &mut String,
    print_tokens: bool,
    saw_stream_payload: &mut bool,
) -> Result<bool, Box<dyn Error>> {
    let trimmed = line.trim();
    if trimmed.is_empty() || !trimmed.starts_with("data: ") {
        return Ok(false);
    }
    let data = trimmed.trim_start_matches("data: ").trim();
    if data.is_empty() {
        return Ok(false);
    }
    if data == "[DONE]" {
        return Ok(true);
    }

    if let Some(token) = extract_sse_token(data) {
        *saw_stream_payload = true;
        answer.push_str(&token);
        if print_tokens {
            print!("{token}");
            std::io::stdout().flush()?;
        }
    }
    Ok(false)
}

async fn ask_llm_streaming(
    cfg: &Config,
    client: &reqwest::Client,
    query: &str,
    context: &str,
    print_tokens: bool,
) -> Result<String, Box<dyn Error>> {
    let mut req = client
        .post(format!(
            "{}/chat/completions",
            cfg.openai_base_url.trim_end_matches('/')
        ))
        .json(&serde_json::json!({
            "model": cfg.openai_model,
            "messages": [
                {"role": "system", "content": "Answer only using provided context. Cite sources like [S1]."},
                {"role": "user", "content": format!("Question: {}\n\nContext:\n{}", query, context)}
            ],
            "temperature": 0.1,
            "stream": true
        }));

    if !cfg.openai_api_key.is_empty() {
        req = req.bearer_auth(&cfg.openai_api_key);
    }

    let response = req.send().await?;
    if !response.status().is_success() {
        return Err(format!("streaming request failed with status {}", response.status()).into());
    }

    let mut stream = response.bytes_stream();
    let mut answer = String::new();
    let mut pending = String::new();
    let mut saw_stream_payload = false;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        pending.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline_idx) = pending.find('\n') {
            let mut line = pending[..newline_idx].to_string();
            pending.drain(..=newline_idx);
            if line.ends_with('\r') {
                let _ = line.pop();
            }
            let done = process_sse_line(&line, &mut answer, print_tokens, &mut saw_stream_payload)?;
            if done {
                return Ok(answer);
            }
        }
    }

    if !pending.trim().is_empty() {
        let _ = process_sse_line(&pending, &mut answer, print_tokens, &mut saw_stream_payload)?;
    }

    if saw_stream_payload {
        return Ok(answer);
    }

    Err("streaming response returned no token payload".into())
}

async fn ask_llm_non_streaming(
    cfg: &Config,
    client: &reqwest::Client,
    query: &str,
    context: &str,
) -> Result<String, Box<dyn Error>> {
    let mut req = client
        .post(format!(
            "{}/chat/completions",
            cfg.openai_base_url.trim_end_matches('/')
        ))
        .json(&serde_json::json!({
            "model": cfg.openai_model,
            "messages": [
                {"role": "system", "content": "Answer only using provided context. Cite sources like [S1]."},
                {"role": "user", "content": format!("Question: {}\n\nContext:\n{}", query, context)}
            ],
            "temperature": 0.1
        }));

    if !cfg.openai_api_key.is_empty() {
        req = req.bearer_auth(&cfg.openai_api_key);
    }

    let response = req.send().await?.error_for_status()?;
    let json: serde_json::Value = response.json().await?;
    Ok(json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(no answer)")
        .to_string())
}

fn push_context_entry(
    entries: &mut Vec<String>,
    context_char_count: &mut usize,
    entry: String,
    separator: &str,
    max_chars: usize,
) -> bool {
    let projected = if entries.is_empty() {
        entry.len()
    } else {
        *context_char_count + separator.len() + entry.len()
    };
    if projected > max_chars {
        return false;
    }
    entries.push(entry);
    *context_char_count = projected;
    true
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

pub async fn run_ask_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let max_context_chars = cfg.ask_max_context_chars;
    let ask_started = std::time::Instant::now();

    let query = cfg
        .query
        .clone()
        .or_else(|| {
            if cfg.positional.is_empty() {
                None
            } else {
                Some(cfg.positional.join(" "))
            }
        })
        .ok_or("ask requires query")?;

    let retrieval_started = std::time::Instant::now();
    let mut ask_vectors = tei::tei_embed(cfg, std::slice::from_ref(&query)).await?;
    if ask_vectors.is_empty() {
        return Err("TEI returned no vector for ask query".into());
    }
    let vecq = ask_vectors.remove(0);
    let candidate_pool_limit = cfg.ask_candidate_limit;
    let chunk_limit = cfg.ask_chunk_limit;
    let full_docs_limit = cfg.ask_full_docs;
    let backfill_limit = cfg.ask_backfill_chunks;
    let doc_fetch_concurrency = cfg.ask_doc_fetch_concurrency;
    let doc_chunk_limit = cfg.ask_doc_chunk_limit;
    let min_relevance_score = cfg.ask_min_relevance_score;
    let query_tokens = ranking::tokenize_query(&query);

    let hits = qdrant::qdrant_search(cfg, &vecq, candidate_pool_limit).await?;
    let mut candidates = Vec::new();
    for hit in hits {
        let score = hit.score;
        let payload = &hit.payload;
        let url = qdrant::payload_url_typed(payload).to_string();
        let path = ranking::extract_path_from_url(&url);
        let chunk_text = qdrant::payload_text_typed(payload).to_string();
        if url.is_empty() || chunk_text.len() < 40 {
            continue;
        }
        candidates.push(ranking::AskCandidate {
            score,
            url: url.clone(),
            path: path.clone(),
            chunk_text: chunk_text.clone(),
            url_tokens: ranking::tokenize_path_set(&path),
            chunk_tokens: ranking::tokenize_text_set(&chunk_text),
            rerank_score: score,
        });
    }
    if candidates.is_empty() {
        return Err("No relevant documents found for ask query".into());
    }

    let reranked = ranking::rerank_ask_candidates(&candidates, &query_tokens)
        .into_iter()
        .filter(|c| c.rerank_score >= min_relevance_score)
        .collect::<Vec<_>>();
    if reranked.is_empty() {
        return Err(format!(
            "No candidates met relevance threshold {:.3}; lower AXON_ASK_MIN_RELEVANCE_SCORE",
            min_relevance_score
        )
        .into());
    }
    let top_chunk_indices = ranking::select_diverse_candidates(&reranked, chunk_limit, 2);
    let top_full_doc_indices = ranking::select_diverse_candidates(&reranked, full_docs_limit, 1);
    let retrieval_elapsed_ms = retrieval_started.elapsed().as_millis();

    let context_started = std::time::Instant::now();
    let mut context_entries: Vec<String> = Vec::new();
    let mut context_char_count = 0usize;
    let separator = "\n\n---\n\n";
    let mut source_idx = 1usize;
    let mut top_chunks_selected = 0usize;
    for &chunk_idx in &top_chunk_indices {
        let chunk = &reranked[chunk_idx];
        let entry = format!(
            "## Top Chunk [S{}]: {}\n\n{}",
            source_idx, chunk.url, chunk.chunk_text
        );
        if !push_context_entry(
            &mut context_entries,
            &mut context_char_count,
            entry,
            separator,
            max_context_chars,
        ) {
            break;
        }
        top_chunks_selected += 1;
        source_idx += 1;
    }

    let mut fetched_docs = Vec::new();
    if context_char_count < max_context_chars {
        let cfg_arc = Arc::new(cfg.clone());
        let mut fetch_stream = stream::iter(top_full_doc_indices.iter().enumerate().map(
            |(order, &doc_idx)| {
                let cfg_for_task = Arc::clone(&cfg_arc);
                let url = reranked[doc_idx].url.clone();
                async move {
                    let points =
                        qdrant::qdrant_retrieve_by_url(&cfg_for_task, &url, Some(doc_chunk_limit))
                            .await;
                    (order, url, points)
                }
            },
        ))
        .buffer_unordered(doc_fetch_concurrency);
        while let Some((order, url, points)) = fetch_stream.next().await {
            fetched_docs.push((order, url, points?));
        }
    }
    fetched_docs.sort_by_key(|(order, _, _)| *order);

    let mut inserted_full_doc_urls: HashSet<String> = HashSet::new();
    let mut full_docs_selected = 0usize;
    for (_idx, url, points) in fetched_docs {
        let text = qdrant::render_full_doc_from_points(points);
        if text.is_empty() {
            continue;
        }
        let entry = format!("## Source Document [S{}]: {}\n\n{}", source_idx, url, text);
        if !push_context_entry(
            &mut context_entries,
            &mut context_char_count,
            entry,
            separator,
            max_context_chars,
        ) {
            break;
        }
        inserted_full_doc_urls.insert(url);
        full_docs_selected += 1;
        source_idx += 1;
    }

    let supplemental_candidate_indices = reranked
        .iter()
        .enumerate()
        .filter(|(_, candidate)| !inserted_full_doc_urls.contains(&candidate.url))
        .map(|(idx, _)| idx)
        .collect::<Vec<_>>();
    let supplemental = ranking::select_diverse_candidates_from_indices(
        &reranked,
        &supplemental_candidate_indices,
        backfill_limit,
        1,
    );

    let mut supplemental_count = 0usize;
    for &chunk_idx in &supplemental {
        let chunk = &reranked[chunk_idx];
        let entry = format!(
            "## Supplemental Chunk [S{}]: {}\n\n{}",
            source_idx, chunk.url, chunk.chunk_text
        );
        if !push_context_entry(
            &mut context_entries,
            &mut context_char_count,
            entry,
            separator,
            max_context_chars,
        ) {
            break;
        }
        supplemental_count += 1;
        source_idx += 1;
    }

    if context_entries.is_empty() {
        return Err("Failed to retrieve any context sources for ask".into());
    }

    let context = format!(
        "Answer only from the provided sources.\nCite supporting sources inline using [S#] labels.\nIf the sources are incomplete, say so explicitly.\n\nSources:\n{}",
        context_entries.join(separator)
    );
    let context_elapsed_ms = context_started.elapsed().as_millis();

    if cfg.ask_diagnostics {
        let mut diagnostic_sources: Vec<String> = Vec::new();
        diagnostic_sources.extend(
            top_full_doc_indices
                .iter()
                .map(|&idx| &reranked[idx])
                .map(|c| format!("full-doc score={:.3} url={}", c.score, c.url)),
        );
        diagnostic_sources.extend(
            supplemental
                .iter()
                .map(|&idx| &reranked[idx])
                .take(supplemental_count)
                .map(|c| format!("chunk score={:.3} url={}", c.score, c.url)),
        );
        if cfg.json_output {
            eprintln!(
                "{}",
                serde_json::json!({
                    "ask_diagnostics": {
                        "candidate_pool": candidates.len(),
                        "reranked_pool": reranked.len(),
                        "chunks_selected": top_chunks_selected,
                        "full_docs_selected": full_docs_selected,
                        "supplemental_selected": supplemental_count,
                        "context_chars": context.len(),
                        "min_relevance_score": min_relevance_score,
                        "doc_fetch_concurrency": doc_fetch_concurrency,
                    "sources": diagnostic_sources,
                    }
                })
            );
        } else {
            eprintln!("{}", primary("Ask Diagnostics"));
            eprintln!(
                "  {} candidates={} reranked={} chunks={} full_docs={} supplemental={} context_chars={}",
                muted("Retrieval:"),
                candidates.len(),
                reranked.len(),
                top_chunks_selected,
                full_docs_selected,
                supplemental_count,
                context.len()
            );
            for source in diagnostic_sources {
                eprintln!("  • {source}");
            }
            eprintln!();
        }
    }

    if cfg.openai_base_url.is_empty() || cfg.openai_model.is_empty() {
        return Err("OPENAI_BASE_URL and OPENAI_MODEL required for ask".into());
    }

    let client = http_client()?;
    let llm_started = std::time::Instant::now();
    if !cfg.json_output {
        println!("{}", primary("Conversation"));
        println!("  {} {}", primary("You:"), query);
        print!("  {} ", primary("Assistant:"));
        std::io::stdout().flush()?;
    }
    let streamed_answer = ask_llm_streaming(cfg, client, &query, &context, !cfg.json_output).await;
    let answer = match streamed_answer {
        Ok(value) => value,
        Err(_) => {
            let fallback = ask_llm_non_streaming(cfg, client, &query, &context).await?;
            if !cfg.json_output {
                print!("{fallback}");
            }
            fallback
        }
    };
    if !cfg.json_output {
        println!();
    }
    let llm_elapsed_ms = llm_started.elapsed().as_millis();
    let total_elapsed_ms = ask_started.elapsed().as_millis();
    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "query": query,
                "answer": answer,
                "diagnostics": if cfg.ask_diagnostics {
                    serde_json::json!({
                        "candidate_pool": candidates.len(),
                        "reranked_pool": reranked.len(),
                        "chunks_selected": top_chunks_selected,
                        "full_docs_selected": full_docs_selected,
                        "supplemental_selected": supplemental_count,
                        "context_chars": context.len(),
                        "min_relevance_score": min_relevance_score,
                        "doc_fetch_concurrency": doc_fetch_concurrency,
                    })
                } else {
                    serde_json::Value::Null
                },
                "timing_ms": {
                    "retrieval": retrieval_elapsed_ms,
                    "context_build": context_elapsed_ms,
                    "llm": llm_elapsed_ms,
                    "total": total_elapsed_ms,
                }
            }))?
        );
    } else {
        if cfg.ask_diagnostics {
            println!(
                "  {} candidates={} reranked={} chunks={} full_docs={} supplemental={} context_chars={}",
                muted("Diagnostics:"),
                candidates.len(),
                reranked.len(),
                top_chunks_selected,
                full_docs_selected,
                supplemental_count,
                context.len()
            );
        }
        println!(
            "  {} retrieval={}ms | context={}ms | llm={}ms | total={}ms",
            muted("Timing:"),
            retrieval_elapsed_ms,
            context_elapsed_ms,
            llm_elapsed_ms,
            total_elapsed_ms
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
