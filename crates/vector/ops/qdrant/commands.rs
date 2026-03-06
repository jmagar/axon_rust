use crate::crates::core::config::Config;
use crate::crates::core::logging::log_warn;
use crate::crates::core::ui::{accent, muted, primary};
use futures_util::stream::{FuturesUnordered, StreamExt};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::error::Error;

use super::client::{
    qdrant_delete_points, qdrant_domain_facets, qdrant_retrieve_by_url, qdrant_scroll_pages,
    qdrant_url_facets,
};
use super::utils::{
    env_usize_clamped, payload_domain, payload_url, render_full_doc_from_points,
    retrieve_max_points,
};

pub async fn retrieve_result(
    cfg: &Config,
    target: &str,
    max_points: Option<usize>,
) -> Result<(usize, String), Box<dyn Error>> {
    let max_points = retrieve_max_points(max_points);
    let candidates = crate::crates::vector::ops::input::url_lookup_candidates(target);

    let mut lookups: FuturesUnordered<_> = candidates
        .iter()
        .map(|candidate| qdrant_retrieve_by_url(cfg, candidate, Some(max_points)))
        .collect();

    let mut points = Vec::new();
    let mut had_success = false;
    let mut first_error: Option<String> = None;
    while let Some(result) = lookups.next().await {
        match result {
            Ok(candidate_points) => {
                had_success = true;
                if !candidate_points.is_empty() {
                    points = candidate_points;
                    break;
                }
            }
            Err(err) => {
                if first_error.is_none() {
                    first_error = Some(err.to_string());
                }
                log_warn(&format!(
                    "retrieve variant lookup failed for {}: {err}",
                    target
                ));
            }
        }
    }
    if points.is_empty() && !had_success {
        if let Some(err) = first_error {
            return Err(format!("retrieve failed for all URL variants: {err}").into());
        }
    }
    if points.is_empty() {
        return Ok((0, String::new()));
    }
    let chunk_count = points.len();
    let out = render_full_doc_from_points(points);
    Ok((chunk_count, out))
}

pub async fn run_retrieve_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let target = cfg.positional.first().ok_or("retrieve requires URL")?;
    let (chunk_count, out) = retrieve_result(cfg, target, None).await?;
    if chunk_count == 0 {
        println!("No content found for URL: {}", target);
        return Ok(());
    }
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
    let facet_limit = env_usize_clamped("AXON_SOURCES_FACET_LIMIT", 100_000, 1, 1_000_000);
    let pagination = crate::crates::services::types::Pagination {
        limit: facet_limit,
        offset: 0,
    };
    let result = crate::crates::services::system::sources(cfg, pagination).await?;
    let url_count = result.urls.len();
    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "count": result.count,
                "limit": result.limit,
                "offset": result.offset,
                "urls": result.urls,
            }))?
        );
    } else {
        println!("{}", primary("Sources"));
        for (url, chunks) in &result.urls {
            println!(
                "  • {} {}",
                accent(url),
                muted(&format!("(chunks: {chunks})"))
            );
        }
        if url_count == facet_limit {
            println!(
                "{}",
                muted(&format!(
                    "Showing top {facet_limit} sources. Set AXON_SOURCES_FACET_LIMIT to see more."
                ))
            );
        }
    }
    Ok(())
}

pub async fn sources_payload(
    cfg: &Config,
    limit: usize,
    offset: usize,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let facet_cap = env_usize_clamped("AXON_SOURCES_FACET_LIMIT", 100_000, 1, 1_000_000);
    let fetch = limit.saturating_add(offset).max(1).min(facet_cap);
    let sources = qdrant_url_facets(cfg, fetch).await?;
    let total = sources.len();
    let urls: Vec<serde_json::Value> = sources
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|(url, chunks)| serde_json::json!({"url": url, "chunks": chunks}))
        .collect();
    Ok(serde_json::json!({
        "count": total,
        "limit": limit,
        "offset": offset,
        "urls": urls,
    }))
}

pub async fn domains_payload(
    cfg: &Config,
    limit: usize,
    offset: usize,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let facet_cap = env_usize_clamped("AXON_DOMAINS_FACET_LIMIT", 100_000, 1, 1_000_000);
    let fetch = limit.saturating_add(offset).max(1).min(facet_cap);
    let domains = qdrant_domain_facets(cfg, fetch).await?;
    let values = domains
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|(domain, vectors)| serde_json::json!({ "domain": domain, "vectors": vectors }))
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "domains": values,
        "limit": limit,
        "offset": offset,
    }))
}

fn domains_detailed_mode() -> bool {
    env::var("AXON_DOMAINS_DETAILED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

fn render_fast_domain_results(
    cfg: &Config,
    domains: Vec<(String, usize)>,
) -> Result<(), Box<dyn Error>> {
    if cfg.json_output {
        let mut out: BTreeMap<String, usize> = BTreeMap::new();
        for (domain, vectors) in domains {
            out.insert(domain, vectors);
        }
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
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
        muted("Tip: set AXON_DOMAINS_DETAILED=1 for exact per-domain unique URL counts (slower).")
    );
    Ok(())
}

async fn try_fast_domains(cfg: &Config) -> Result<bool, Box<dyn Error>> {
    let facet_limit = env_usize_clamped("AXON_DOMAINS_FACET_LIMIT", 100_000, 1, 1_000_000);
    let pagination = crate::crates::services::types::Pagination {
        limit: facet_limit,
        offset: 0,
    };
    match crate::crates::services::system::domains(cfg, pagination).await {
        Ok(result) => {
            let pairs: Vec<(String, usize)> = result
                .domains
                .into_iter()
                .map(|f| (f.domain, f.vectors))
                .collect();
            render_fast_domain_results(cfg, pairs)?;
            Ok(true)
        }
        Err(err) => {
            eprintln!(
                "warning: fast domain facet query failed ({err}); falling back to detailed scan"
            );
            Ok(false)
        }
    }
}

fn render_detailed_domains(
    cfg: &Config,
    by_domain: HashMap<String, (usize, HashSet<String>)>,
) -> Result<(), Box<dyn Error>> {
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

pub async fn run_domains_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if !domains_detailed_mode() && try_fast_domains(cfg).await? {
        return Ok(());
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
    render_detailed_domains(cfg, by_domain)
}

struct DedupeRecord {
    id: String,
    scraped_at: String,
}

pub async fn run_dedupe_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    // Collect all points keyed by (url, chunk_index).
    let mut by_key: HashMap<(String, i64), Vec<DedupeRecord>> = HashMap::new();
    qdrant_scroll_pages(cfg, |points| {
        for p in points {
            let id = p
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() {
                continue;
            }
            let Some(payload) = p.get("payload") else {
                continue;
            };
            let url = payload_url(payload);
            if url.is_empty() {
                continue;
            }
            let chunk_index = payload
                .get("chunk_index")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let scraped_at = payload
                .get("scraped_at")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            by_key
                .entry((url, chunk_index))
                .or_default()
                .push(DedupeRecord { id, scraped_at });
        }
    })
    .await?;

    let mut to_delete: Vec<String> = Vec::new();
    let mut dup_groups = 0usize;
    for mut records in by_key.into_values() {
        if records.len() <= 1 {
            continue;
        }
        dup_groups += 1;
        // Keep the newest point; delete the rest.
        records.sort_unstable_by(|a, b| b.scraped_at.cmp(&a.scraped_at));
        to_delete.extend(records.into_iter().skip(1).map(|r| r.id));
    }

    let deleted = qdrant_delete_points(cfg, &to_delete).await?;

    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({
                "duplicate_groups": dup_groups,
                "deleted": deleted,
                "collection": cfg.collection,
            })
        );
    } else {
        println!(
            "{} deduplicated {} groups, deleted {} points from {}",
            crate::crates::core::ui::symbol_for_status("completed"),
            dup_groups,
            deleted,
            accent(&cfg.collection)
        );
    }
    Ok(())
}
