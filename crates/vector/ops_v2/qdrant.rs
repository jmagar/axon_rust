use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::http::http_client;
use crate::axon_cli::crates::core::ui::{accent, muted, primary};
use futures_util::future::join_all;
use serde::Deserialize;
use spider::url::Url;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::error::Error;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct QdrantPayload {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub chunk_text: String,
    #[serde(default)]
    pub text: String,
    pub chunk_index: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QdrantPoint {
    #[serde(default)]
    pub payload: QdrantPayload,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QdrantSearchHit {
    pub score: f64,
    #[serde(default)]
    pub payload: QdrantPayload,
}

#[derive(Debug, Deserialize)]
struct QdrantSearchResponse {
    #[serde(default)]
    result: Vec<QdrantSearchHit>,
}

#[derive(Debug, Deserialize)]
struct QdrantScrollResult {
    #[serde(default)]
    points: Vec<QdrantPoint>,
    next_page_offset: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct QdrantScrollResponse {
    result: QdrantScrollResult,
}

const RETRIEVE_MAX_POINTS_CEILING: usize = 500;

fn env_usize_clamped(key: &str, default: usize, min: usize, max: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v >= min)
        .unwrap_or(default)
        .clamp(min, max)
}

fn qdrant_base(cfg: &Config) -> String {
    cfg.qdrant_url.trim_end_matches('/').to_string()
}

pub fn payload_text_typed(payload: &QdrantPayload) -> &str {
    if !payload.chunk_text.is_empty() {
        payload.chunk_text.as_str()
    } else {
        payload.text.as_str()
    }
}

pub fn payload_url_typed(payload: &QdrantPayload) -> &str {
    payload.url.as_str()
}

fn payload_url(payload: &serde_json::Value) -> String {
    payload
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn payload_domain(payload: &serde_json::Value) -> String {
    payload
        .get("domain")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string()
}

pub fn base_url(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    let mut out = format!("{}://{host}", parsed.scheme());
    if let Some(port) = parsed.port() {
        out.push(':');
        out.push_str(&port.to_string());
    }
    Some(out)
}

pub fn render_full_doc_from_points(mut points: Vec<QdrantPoint>) -> String {
    points.sort_by_key(|p| p.payload.chunk_index.unwrap_or(i64::MAX));
    let capacity = points
        .iter()
        .map(|point| payload_text_typed(&point.payload).len())
        .sum::<usize>()
        + points.len();
    let mut text = String::with_capacity(capacity);
    for point in points {
        let chunk = payload_text_typed(&point.payload);
        if chunk.is_empty() {
            continue;
        }
        text.push_str(chunk);
        text.push('\n');
    }
    text.trim().to_string()
}

pub fn query_snippet(payload: &QdrantPayload) -> String {
    let text = payload_text_typed(payload).replace('\n', " ");
    let end = text
        .char_indices()
        .nth(140)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len());
    text[..end].to_string()
}

fn retrieve_max_points(max_points: Option<usize>) -> usize {
    max_points
        .unwrap_or(RETRIEVE_MAX_POINTS_CEILING)
        .min(RETRIEVE_MAX_POINTS_CEILING)
}

async fn qdrant_scroll_pages(
    cfg: &Config,
    mut process_page: impl FnMut(&[serde_json::Value]),
) -> Result<(), Box<dyn Error>> {
    let client = http_client()?;
    let mut offset: Option<serde_json::Value> = None;

    loop {
        let mut body = serde_json::json!({
            "limit": 256,
            "with_payload": true,
            "with_vector": false
        });
        if let Some(off) = offset.take() {
            body["offset"] = off;
        }

        let url = format!(
            "{}/collections/{}/points/scroll",
            qdrant_base(cfg),
            cfg.collection
        );
        let val = client
            .post(url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        let points = val["result"]["points"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if points.is_empty() {
            break;
        }
        process_page(points);

        offset = val["result"].get("next_page_offset").cloned();
        if offset.as_ref().is_none() || offset == Some(serde_json::Value::Null) {
            break;
        }
    }

    Ok(())
}

pub async fn qdrant_indexed_urls(cfg: &Config) -> Result<Vec<String>, Box<dyn Error>> {
    let mut seen = HashSet::new();
    qdrant_scroll_pages(cfg, |points| {
        for p in points {
            let Some(payload) = p.get("payload") else {
                continue;
            };
            let url = payload_url(payload);
            if !url.is_empty() {
                seen.insert(url);
            }
        }
    })
    .await?;

    let mut urls: Vec<String> = seen.into_iter().collect();
    urls.sort();
    Ok(urls)
}

async fn qdrant_domain_facets(
    cfg: &Config,
    limit: usize,
) -> Result<Vec<(String, usize)>, Box<dyn Error>> {
    let client = http_client()?;
    let url = format!("{}/collections/{}/facet", qdrant_base(cfg), cfg.collection);
    let value = client
        .post(url)
        .json(&serde_json::json!({
            "key": "domain",
            "limit": limit,
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let mut out = Vec::new();
    if let Some(hits) = value["result"]["hits"].as_array() {
        for hit in hits {
            let domain = hit
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let vectors = hit.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            out.push((domain, vectors));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub(crate) async fn qdrant_search(
    cfg: &Config,
    vector: &[f32],
    limit: usize,
) -> Result<Vec<QdrantSearchHit>, Box<dyn Error>> {
    let client = http_client()?;
    let url = format!(
        "{}/collections/{}/points/search",
        qdrant_base(cfg),
        cfg.collection
    );
    let res = client
        .post(url)
        .json(&serde_json::json!({
            "vector": vector,
            "limit": limit,
            "with_payload": true,
            "with_vector": false
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<QdrantSearchResponse>()
        .await?;
    Ok(res.result)
}

pub(crate) async fn qdrant_retrieve_by_url(
    cfg: &Config,
    url_match: &str,
    max_points: Option<usize>,
) -> Result<Vec<QdrantPoint>, Box<dyn Error>> {
    let client = http_client()?;
    let mut out = Vec::new();
    let mut offset: Option<serde_json::Value> = None;
    let max_points = retrieve_max_points(max_points);

    loop {
        let mut body = serde_json::json!({
            "limit": 256,
            "with_payload": true,
            "with_vector": false,
            "filter": {
                "must": [
                    {
                        "key": "url",
                        "match": {"value": url_match}
                    }
                ]
            }
        });
        if let Some(off) = offset.take() {
            body["offset"] = off;
        }

        let val = client
            .post(format!(
                "{}/collections/{}/points/scroll",
                qdrant_base(cfg),
                cfg.collection
            ))
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<QdrantScrollResponse>()
            .await?;

        let points = val.result.points;
        if points.is_empty() {
            break;
        }
        out.extend(points);
        if out.len() >= max_points {
            out.truncate(max_points);
            break;
        }

        offset = val.result.next_page_offset;
        if offset.as_ref().is_none() {
            break;
        }
    }

    Ok(out)
}

pub async fn run_retrieve_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let target = cfg.positional.first().ok_or("retrieve requires URL")?;
    let max_points = retrieve_max_points(None);
    let candidates = crate::axon_cli::crates::vector::ops_v2::input::url_lookup_candidates(target);
    let results = join_all(candidates.iter().map(|candidate| async move {
        qdrant_retrieve_by_url(cfg, candidate, Some(max_points)).await
    }))
    .await;

    let mut points = Vec::new();
    for result in results {
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

#[cfg(test)]
mod tests {
    use super::{retrieve_max_points, RETRIEVE_MAX_POINTS_CEILING};

    #[test]
    fn retrieve_max_points_defaults_to_ceiling() {
        assert_eq!(retrieve_max_points(None), RETRIEVE_MAX_POINTS_CEILING);
    }

    #[test]
    fn retrieve_max_points_caps_values_above_ceiling() {
        assert_eq!(
            retrieve_max_points(Some(RETRIEVE_MAX_POINTS_CEILING + 250)),
            RETRIEVE_MAX_POINTS_CEILING
        );
    }

    #[test]
    fn retrieve_max_points_preserves_lower_values() {
        assert_eq!(retrieve_max_points(Some(128)), 128);
    }
}
