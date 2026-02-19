use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::http::http_client;
use std::collections::HashSet;
use std::error::Error;

use super::types::{QdrantPoint, QdrantScrollResponse, QdrantSearchHit, QdrantSearchResponse};
use super::utils::{payload_url, retrieve_max_points};

fn qdrant_base(cfg: &Config) -> String {
    cfg.qdrant_url.trim_end_matches('/').to_string()
}

pub(crate) async fn qdrant_scroll_pages(
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
