use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::ui::{muted, primary};
use crate::axon_cli::crates::vector::ops_v2::{qdrant, tei};
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
