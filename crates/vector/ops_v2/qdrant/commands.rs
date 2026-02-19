use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::ui::{accent, muted, primary};
use futures_util::stream::{FuturesUnordered, StreamExt};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::error::Error;

use super::client::{qdrant_domain_facets, qdrant_retrieve_by_url, qdrant_scroll_pages};
use super::utils::{
    env_usize_clamped, payload_domain, payload_url, render_full_doc_from_points,
    retrieve_max_points,
};

pub async fn run_retrieve_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let target = cfg.positional.first().ok_or("retrieve requires URL")?;
    let max_points = retrieve_max_points(None);
    let candidates = crate::axon_cli::crates::vector::ops_v2::input::url_lookup_candidates(target);

    let mut lookups: FuturesUnordered<_> = candidates
        .iter()
        .map(|candidate| qdrant_retrieve_by_url(cfg, candidate, Some(max_points)))
        .collect();

    let mut points = Vec::new();
    while let Some(result) = lookups.next().await {
        let candidate_points = result?;
        if !candidate_points.is_empty() {
            points = candidate_points;
            break;
        }
    }
    if points.is_empty() {
        println!("No content found for URL: {}", target);
        return Ok(());
    }

    let chunk_count = points.len();
    let out = render_full_doc_from_points(points);
    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "url": target,
                "chunks": chunk_count,
                "content": out.trim()
            }))?
        );
    } else {
        println!("{}", primary(&format!("Retrieve Result for {target}")));
        println!("{} {}\n", muted("Chunks:"), chunk_count);
        println!("{}", out.trim());
    }
    Ok(())
}

pub async fn run_sources_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let mut by_url: BTreeMap<String, usize> = BTreeMap::new();
    qdrant_scroll_pages(cfg, |points| {
        for p in points {
            let Some(payload) = p.get("payload") else {
                continue;
            };
            let url = payload_url(payload);
            if url.is_empty() {
                continue;
            }
            *by_url.entry(url).or_insert(0) += 1;
        }
    })
    .await?;
    if cfg.json_output {
        println!("{}", serde_json::to_string_pretty(&by_url)?);
    } else {
        println!("{}", primary("Sources"));
        for (url, chunks) in by_url {
            println!(
                "  • {} {}",
                accent(&url),
                muted(&format!("(chunks: {chunks})"))
            );
        }
    }
    Ok(())
}

pub async fn run_domains_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let detailed_mode = env::var("AXON_DOMAINS_DETAILED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false);

    if !detailed_mode {
        let facet_limit = env_usize_clamped("AXON_DOMAINS_FACET_LIMIT", 100_000, 1, 1_000_000);
        match qdrant_domain_facets(cfg, facet_limit).await {
            Ok(domains) => {
                if cfg.json_output {
                    let mut out: BTreeMap<String, usize> = BTreeMap::new();
                    for (domain, vectors) in domains {
                        out.insert(domain, vectors);
                    }
                    println!("{}", serde_json::to_string_pretty(&out)?);
                } else {
                    println!("{}", primary("Domains"));
                    for (domain, vectors) in domains {
                        println!(
                            "  • {} {}",
                            accent(&domain),
                            muted(&format!("vectors={vectors}"))
                        );
                    }
                    println!(
                        "{}",
                        muted(
                            "Tip: set AXON_DOMAINS_DETAILED=1 for exact per-domain unique URL counts (slower)."
                        )
                    );
                }
                return Ok(());
            }
            Err(err) => {
                eprintln!(
                    "warning: fast domain facet query failed ({err}); falling back to detailed scan"
                );
            }
        }
    }

    let mut by_domain: HashMap<String, (usize, HashSet<String>)> = HashMap::new();
    qdrant_scroll_pages(cfg, |points| {
        for p in points {
            let Some(payload) = p.get("payload") else {
                continue;
            };
            let domain = payload_domain(payload);
            let url = payload_url(payload);
            let entry = by_domain.entry(domain).or_insert((0, HashSet::new()));
            entry.0 += 1;
            if !url.is_empty() {
                entry.1.insert(url);
            }
        }
    })
    .await?;
    if cfg.json_output {
        let mut out: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for (domain, (vectors, urls)) in by_domain {
            out.insert(domain, (vectors, urls.len()));
        }
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!("{}", primary("Domains"));
        let mut rows: Vec<_> = by_domain.into_iter().collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        for (domain, (vectors, urls)) in rows {
            println!(
                "  • {} {}",
                accent(&domain),
                muted(&format!("urls={} vectors={}", urls.len(), vectors))
            );
        }
    }
    Ok(())
}
