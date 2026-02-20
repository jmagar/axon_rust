use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::ui::{muted, primary};
use crate::axon_cli::crates::vector::ops_v2::{qdrant, ranking, tei};
use std::error::Error;

use super::resolve_query_text;

pub async fn run_query_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let query = resolve_query_text(cfg).ok_or("query requires text")?;

    let mut query_vectors = tei::tei_embed(cfg, std::slice::from_ref(&query)).await?;
    if query_vectors.is_empty() {
        return Err("TEI returned no vector for query".into());
    }
    let vector = query_vectors.remove(0);

    // Over-fetch so reranking has headroom; then trim to the requested limit.
    // 8x matches the TS reference impl's strategy for dedup quality.
    let fetch_limit = (cfg.search_limit * 8).max(cfg.search_limit).min(500);
    let hits = qdrant::qdrant_search(cfg, &vector, fetch_limit).await?;

    let query_tokens = ranking::tokenize_query(&query);
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
        if !cfg.json_output {
            println!("{}", primary(&format!("Query Results for \"{query}\"")));
            println!("  {}", muted("No results found."));
        }
        return Ok(());
    }

    let reranked = ranking::rerank_ask_candidates(&candidates, &query_tokens);
    let selected_indices =
        ranking::select_diverse_candidates(&reranked, cfg.search_limit.max(1), 2);

    if !cfg.json_output {
        println!("{}", primary(&format!("Query Results for \"{query}\"")));
        println!("{} {}\n", muted("Showing"), selected_indices.len());
    }

    for (i, &hit_idx) in selected_indices.iter().enumerate() {
        let h = &reranked[hit_idx];
        let url = &h.url;
        // Pick the chunk with the best preview score for this URL (may differ from
        // the top-ranked chunk). Keeps score/URL from the vector-ranked hit while
        // showing the most readable prose chunk as the snippet.
        let preview_idx =
            ranking::select_best_preview_chunk(&reranked, url, &query_tokens, hit_idx);
        let snippet =
            ranking::get_meaningful_snippet(&reranked[preview_idx].chunk_text, &query_tokens);
        if cfg.json_output {
            println!(
                "{}",
                serde_json::json!({
                    "rank": i + 1,
                    "score": h.score,
                    "rerank_score": h.rerank_score,
                    "url": url,
                    "snippet": snippet,
                })
            );
        } else {
            println!(
                "  • {}. {} [{:.3}] {}",
                i + 1,
                crate::axon_cli::crates::core::ui::status_text("completed"),
                h.rerank_score,
                crate::axon_cli::crates::core::ui::accent(url)
            );
            println!("    {}", snippet);
        }
    }

    Ok(())
}
