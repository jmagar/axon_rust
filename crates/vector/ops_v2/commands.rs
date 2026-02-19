use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::http::http_client;
use crate::axon_cli::crates::core::ui::{muted, primary};
use crate::axon_cli::crates::vector::ops_v2::{qdrant, ranking, tei};
use futures_util::stream::{self, StreamExt};
use std::collections::HashSet;
use std::env;
use std::error::Error;

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

fn env_f64_clamped(key: &str, default: f64, min: f64, max: f64) -> f64 {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default)
        .clamp(min, max)
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

pub async fn run_ask_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let max_context_chars =
        env_usize_clamped("AXON_ASK_MAX_CONTEXT_CHARS", 120_000, 20_000, 400_000);
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
    let candidate_pool_limit = env_usize_clamped("AXON_ASK_CANDIDATE_LIMIT", 64, 8, 200);
    let chunk_limit = env_usize_clamped("AXON_ASK_CHUNK_LIMIT", 10, 3, 40);
    let full_docs_limit = env_usize_clamped("AXON_ASK_FULL_DOCS", 4, 1, 20);
    let backfill_limit = env_usize_clamped("AXON_ASK_BACKFILL_CHUNKS", 3, 0, 20);
    let doc_fetch_concurrency = env_usize_clamped("AXON_ASK_DOC_FETCH_CONCURRENCY", 4, 1, 16);
    let doc_chunk_limit = env_usize_clamped("AXON_ASK_DOC_CHUNK_LIMIT", 192, 8, 2000);
    let min_relevance_score = env_f64_clamped("AXON_ASK_MIN_RELEVANCE_SCORE", 0.0, -1.0, 2.0);

    let hits = qdrant::qdrant_search(cfg, &vecq, candidate_pool_limit).await?;
    let mut candidates = Vec::new();
    for hit in hits {
        let score = hit.score;
        let payload = &hit.payload;
        let url = qdrant::payload_url_typed(payload).to_string();
        let chunk_text = qdrant::payload_text_typed(payload).to_string();
        if url.is_empty() || chunk_text.len() < 40 {
            continue;
        }
        candidates.push(ranking::AskCandidate {
            score,
            url: url.clone(),
            chunk_text: chunk_text.clone(),
            url_tokens: ranking::tokenize_path_set(&url),
            chunk_tokens: ranking::tokenize_text_set(&chunk_text),
            rerank_score: score,
        });
    }
    if candidates.is_empty() {
        return Err("No relevant documents found for ask query".into());
    }

    let reranked = ranking::rerank_ask_candidates(&candidates, &query)
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
    let top_chunks = ranking::select_diverse_candidates(&reranked, chunk_limit, 2);
    let top_full_docs = ranking::select_diverse_candidates(&reranked, full_docs_limit, 1);
    let retrieval_elapsed_ms = retrieval_started.elapsed().as_millis();

    let context_started = std::time::Instant::now();
    let mut context_entries: Vec<String> = Vec::new();
    let mut context_char_count = 0usize;
    let separator = "\n\n---\n\n";
    let mut source_idx = 1usize;
    let mut top_chunks_selected = 0usize;
    for chunk in &top_chunks {
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
        let mut fetch_stream = stream::iter(top_full_docs.iter().enumerate().map(
            |(idx, doc)| async move {
                let url = doc.url.clone();
                let points = qdrant::qdrant_retrieve_by_url(cfg, &url, Some(doc_chunk_limit)).await;
                (idx, url, points)
            },
        ))
        .buffer_unordered(doc_fetch_concurrency);
        while let Some((idx, url, points)) = fetch_stream.next().await {
            fetched_docs.push((idx, url, points?));
        }
    }
    fetched_docs.sort_by_key(|(idx, _, _)| *idx);

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

    let supplemental = ranking::select_diverse_candidates(
        &reranked
            .iter()
            .filter(|c| !inserted_full_doc_urls.contains(&c.url))
            .cloned()
            .collect::<Vec<_>>(),
        backfill_limit,
        1,
    );

    let mut supplemental_count = 0usize;
    for chunk in &supplemental {
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
            top_full_docs
                .iter()
                .map(|c| format!("full-doc score={:.3} url={}", c.score, c.url)),
        );
        diagnostic_sources.extend(
            supplemental
                .iter()
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

    let llm_started = std::time::Instant::now();
    let response = req.send().await?.error_for_status()?;
    let json: serde_json::Value = response.json().await?;
    let answer = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(no answer)");
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
        println!("{}", primary("Conversation"));
        println!("  {} {}", primary("You:"), query);
        println!("  {} {}", primary("Assistant:"), answer);
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
