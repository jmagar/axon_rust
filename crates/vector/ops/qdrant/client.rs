use crate::crates::core::config::Config;
use crate::crates::core::http::http_client;
use std::collections::HashSet;
use std::error::Error;

use super::types::{QdrantPoint, QdrantSearchHit, QdrantSearchResponse};
use super::utils::{qdrant_base, retrieve_max_points};

/// Shared scroll pagination loop. POSTs to the given `endpoint` with `initial_body`,
/// reads `result.points` as raw JSON, and invokes `on_page` for each non-empty page.
/// The callback returns `true` to continue scrolling or `false` to stop early.
async fn scroll_pages_raw(
    client: &reqwest::Client,
    endpoint: &str,
    initial_body: serde_json::Value,
    mut on_page: impl FnMut(&[serde_json::Value]) -> bool,
) -> Result<(), Box<dyn Error>> {
    let mut body = initial_body;
    loop {
        let val = client
            .post(endpoint)
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
        if !on_page(points) {
            break;
        }

        let next = val["result"].get("next_page_offset").cloned();
        if next.is_none() || next == Some(serde_json::Value::Null) {
            break;
        }
        body["offset"] = next.unwrap();
    }
    Ok(())
}

pub(crate) async fn qdrant_scroll_pages(
    cfg: &Config,
    mut process_page: impl FnMut(&[serde_json::Value]),
) -> Result<(), Box<dyn Error>> {
    let client = http_client()?;
    let endpoint = format!(
        "{}/collections/{}/points/scroll",
        qdrant_base(cfg),
        cfg.collection
    );
    let body = serde_json::json!({
        "limit": 256,
        "with_payload": true,
        "with_vector": false
    });
    scroll_pages_raw(client, &endpoint, body, |points| {
        process_page(points);
        true
    })
    .await
}

/// Scroll the collection keeping only the URL field (one entry per unique URL via chunk_index==0
/// filter) and collect into a HashSet. The `filter` value is passed directly as the Qdrant
/// filter body so callers control which subset of documents is scanned.
async fn scroll_url_set(
    cfg: &Config,
    filter: serde_json::Value,
    limit: Option<usize>,
) -> Result<HashSet<String>, Box<dyn Error>> {
    let client = http_client()?;
    let endpoint = format!(
        "{}/collections/{}/points/scroll",
        qdrant_base(cfg),
        cfg.collection
    );
    let mut seen = HashSet::new();
    let body = serde_json::json!({
        "limit": 1000,
        "with_payload": {"include": ["url"]},
        "with_vector": false,
        "filter": filter,
    });
    scroll_pages_raw(client, &endpoint, body, |points| {
        for p in points {
            if let Some(url) = p
                .get("payload")
                .and_then(|pl| pl.get("url"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                seen.insert(url.to_string());
            }
            if limit.is_some_and(|cap| seen.len() >= cap) {
                return false;
            }
        }
        true
    })
    .await?;
    Ok(seen)
}

pub async fn qdrant_indexed_urls(
    cfg: &Config,
    limit: Option<usize>,
) -> Result<Vec<String>, Box<dyn Error>> {
    let filter = serde_json::json!({
        "must": [{"key": "chunk_index", "match": {"value": 0}}]
    });
    scroll_url_set(cfg, filter, limit)
        .await
        .map(|s| s.into_iter().collect())
}

pub(crate) async fn qdrant_urls_for_domain(
    cfg: &Config,
    domain: &str,
) -> Result<HashSet<String>, Box<dyn Error>> {
    let filter = serde_json::json!({
        "must": [
            {"key": "domain", "match": {"value": domain}},
            {"key": "chunk_index", "match": {"value": 0}}
        ]
    });
    scroll_url_set(cfg, filter, None).await
}

/// Delete all Qdrant points matching `url` via payload filter.
pub(crate) async fn qdrant_delete_by_url_filter(
    cfg: &Config,
    url: &str,
) -> Result<(), Box<dyn Error>> {
    let client = http_client()?;
    client
        .post(format!(
            "{}/collections/{}/points/delete?wait=true",
            qdrant_base(cfg),
            cfg.collection
        ))
        .json(&serde_json::json!({
            "filter": {"must": [{"key": "url", "match": {"value": url}}]}
        }))
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

/// Delete all Qdrant points for URLs that belong to `domain` but are NOT in `current_urls`.
/// Uses a single batch delete with a `should` filter instead of per-URL requests.
/// Returns the number of stale URLs whose points were deleted.
pub async fn qdrant_delete_stale_domain_urls(
    cfg: &Config,
    domain: &str,
    current_urls: &HashSet<String>,
) -> Result<usize, Box<dyn Error>> {
    let indexed = qdrant_urls_for_domain(cfg, domain).await?;
    let stale: Vec<String> = indexed
        .into_iter()
        .filter(|url| !current_urls.contains(url))
        .collect();
    if stale.is_empty() {
        return Ok(0);
    }
    // Batch delete: build a single `should` filter matching all stale URLs at once.
    let url_conditions: Vec<serde_json::Value> = stale
        .iter()
        .map(|url| serde_json::json!({"key": "url", "match": {"value": url}}))
        .collect();
    let client = http_client()?;
    // Qdrant filter limit is generous but chunk at 500 to be safe with large stale sets.
    for batch in url_conditions.chunks(500) {
        client
            .post(format!(
                "{}/collections/{}/points/delete?wait=true",
                qdrant_base(cfg),
                cfg.collection
            ))
            .json(&serde_json::json!({
                "filter": {"should": batch}
            }))
            .send()
            .await?
            .error_for_status()?;
    }
    Ok(stale.len())
}

pub(crate) async fn qdrant_delete_points(
    cfg: &Config,
    ids: &[String],
) -> Result<usize, Box<dyn Error>> {
    if ids.is_empty() {
        return Ok(0);
    }
    let client = http_client()?;
    let url = format!(
        "{}/collections/{}/points/delete?wait=true",
        qdrant_base(cfg),
        cfg.collection
    );
    for batch in ids.chunks(1000) {
        client
            .post(&url)
            .json(&serde_json::json!({"points": batch}))
            .send()
            .await?
            .error_for_status()?;
    }
    Ok(ids.len())
}

pub(crate) async fn qdrant_domain_facets(
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

pub(crate) async fn qdrant_url_facets(
    cfg: &Config,
    limit: usize,
) -> Result<Vec<(String, usize)>, Box<dyn Error>> {
    let client = http_client()?;
    let url = format!("{}/collections/{}/facet", qdrant_base(cfg), cfg.collection);
    let value = client
        .post(url)
        .json(&serde_json::json!({
            "key": "url",
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
            let source_url = hit
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let chunks = hit.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if !source_url.is_empty() {
                out.push((source_url, chunks));
            }
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
    let endpoint = format!(
        "{}/collections/{}/points/scroll",
        qdrant_base(cfg),
        cfg.collection
    );
    let body = serde_json::json!({
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
    let max_points = retrieve_max_points(max_points);
    let mut out = Vec::new();
    scroll_pages_raw(client, &endpoint, body, |points| {
        for p in points {
            if let Ok(point) = serde_json::from_value::<QdrantPoint>(p.clone()) {
                out.push(point);
            }
        }
        out.len() < max_points
    })
    .await?;
    out.truncate(max_points);
    Ok(out)
}
