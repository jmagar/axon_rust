use crate::crates::core::config::Config;
use crate::crates::core::ui::{muted, primary};
use crate::crates::services::query as query_service;
use crate::crates::services::types::Pagination;
use crate::crates::vector::ops::source_display::display_source;
use crate::crates::vector::ops::{qdrant, ranking, tei};
use std::error::Error;

use super::resolve_query_text;

pub async fn query_results(
    cfg: &Config,
    query: &str,
    limit: usize,
    offset: usize,
) -> Result<Vec<serde_json::Value>, Box<dyn Error>> {
    let mut query_vectors = tei::tei_embed(cfg, std::slice::from_ref(&query.to_string())).await?;
    if query_vectors.is_empty() {
        return Err("TEI returned no vector for query".into());
    }
    let vector = query_vectors.remove(0);

    let fetch_limit = ((limit + offset).max(1) * 8).max(limit + offset).min(500);
    let hits = qdrant::qdrant_search(cfg, &vector, fetch_limit).await?;
    let query_tokens = ranking::tokenize_query(query);
    let candidates: Vec<ranking::AskCandidate> = hits
        .into_iter()
        .filter_map(|h| {
            let url = qdrant::payload_url_typed(&h.payload).to_string();
            let chunk_text = qdrant::payload_text_typed(&h.payload).to_string();
            if url.is_empty() {
                return None;
            }
            let path = ranking::extract_path_from_url(&url);
            Some(ranking::AskCandidate {
                score: h.score,
                url,
                path: path.clone(),
                chunk_text: chunk_text.clone(),
                url_tokens: ranking::tokenize_path_set(&path),
                chunk_tokens: ranking::tokenize_text_set(&chunk_text),
                rerank_score: h.score,
            })
        })
        .collect();
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let reranked = ranking::rerank_ask_candidates(
        &candidates,
        &query_tokens,
        &cfg.ask_authoritative_domains,
        cfg.ask_authoritative_boost,
    );
    let selected_indices =
        ranking::select_diverse_candidates(&reranked, (limit + offset).max(1), 2);

    Ok(selected_indices
        .into_iter()
        .skip(offset)
        .take(limit)
        .enumerate()
        .map(|(i, hit_idx)| {
            let h = &reranked[hit_idx];
            let url = &h.url;
            let source = display_source(url);
            let preview_idx =
                ranking::select_best_preview_chunk(&reranked, url, &query_tokens, hit_idx);
            let snippet =
                ranking::get_meaningful_snippet(&reranked[preview_idx].chunk_text, &query_tokens);
            serde_json::json!({
                "rank": i + 1,
                "score": h.score,
                "rerank_score": h.rerank_score,
                "url": url,
                "source": source,
                "snippet": snippet,
                "chunk_index": serde_json::Value::Null
            })
        })
        .collect::<Vec<_>>())
}

pub async fn run_query_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let query = resolve_query_text(cfg).ok_or("query requires text")?;
    // Route data-fetch through the services layer.
    let opts = Pagination {
        limit: cfg.search_limit.max(1),
        offset: 0,
    };
    let results = query_service::query(cfg, &query, opts).await?.results;
    if results.is_empty() {
        if !cfg.json_output {
            println!("{}", primary(&format!("Query Results for \"{query}\"")));
            println!("  {}", muted("No results found."));
        }
        return Ok(());
    }

    if !cfg.json_output {
        println!("{}", primary(&format!("Query Results for \"{query}\"")));
        println!("{} {}\n", muted("Showing"), results.len());
    }

    for result in &results {
        let rank = result["rank"].as_u64().unwrap_or(0);
        let score = result["score"].as_f64().unwrap_or(0.0);
        let rerank_score = result["rerank_score"].as_f64().unwrap_or(0.0);
        let url = result["url"].as_str().unwrap_or("");
        let source = result["source"].as_str().unwrap_or("");
        let snippet = result["snippet"].as_str().unwrap_or("");
        if cfg.json_output {
            println!("{}", result);
        } else {
            println!(
                "  • {}. {} [{:.3}] {}",
                rank,
                crate::crates::core::ui::status_text("completed"),
                rerank_score,
                crate::crates::core::ui::accent(source)
            );
            println!("    {}", snippet);
            if cfg.ask_diagnostics {
                println!("    {} vector_score={:.3}", muted("diag"), score);
                println!("    {} {}", muted("url"), url);
            }
        }
    }

    Ok(())
}
