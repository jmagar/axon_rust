use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::content::to_markdown;
use crate::axon_cli::crates::core::http::{build_client, fetch_html, http_client};
use crate::axon_cli::crates::core::ui::{accent, muted, primary, status_text, symbol_for_status};
use chrono::Utc;
use reqwest::StatusCode;
use spider::url::Url;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn qdrant_base(cfg: &Config) -> String {
    cfg.qdrant_url.trim_end_matches('/').to_string()
}

async fn tei_embed(cfg: &Config, inputs: &[String]) -> Result<Vec<Vec<f32>>, Box<dyn Error>> {
    if inputs.is_empty() {
        return Ok(Vec::new());
    }
    let client = http_client()?;
    let mut vectors = Vec::new();

    // Respect TEI max client batch size when provided, but cap default to avoid huge request bodies.
    let configured = env::var("TEI_MAX_CLIENT_BATCH_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(64);
    let batch_size = configured.min(128);

    let mut stack: Vec<&[String]> = inputs.chunks(batch_size).collect();
    while let Some(chunk) = stack.pop() {
        let resp = client
            .post(format!("{}/embed", cfg.tei_url.trim_end_matches('/')))
            .json(&serde_json::json!({"inputs": chunk}))
            .send()
            .await?;

        if resp.status() == StatusCode::PAYLOAD_TOO_LARGE && chunk.len() > 1 {
            let mid = chunk.len() / 2;
            let (left, right) = chunk.split_at(mid);
            // Push right then left so left executes first (stack LIFO).
            stack.push(right);
            stack.push(left);
            continue;
        }

        let resp = resp.error_for_status()?;
        let mut batch_vectors = resp.json::<Vec<Vec<f32>>>().await?;
        vectors.append(&mut batch_vectors);
    }
    Ok(vectors)
}

async fn ensure_collection(cfg: &Config, dim: usize) -> Result<(), Box<dyn Error>> {
    let client = http_client()?;
    let url = format!("{}/collections/{}", qdrant_base(cfg), cfg.collection);
    let create = serde_json::json!({
        "vectors": {"size": dim, "distance": "Cosine"}
    });
    let _ = client.put(url).json(&create).send().await?;
    Ok(())
}

async fn qdrant_upsert(cfg: &Config, points: &[serde_json::Value]) -> Result<(), Box<dyn Error>> {
    if points.is_empty() {
        return Ok(());
    }
    let client = http_client()?;
    let url = format!(
        "{}/collections/{}/points?wait=true",
        qdrant_base(cfg),
        cfg.collection
    );
    for batch in points.chunks(256) {
        client
            .put(&url)
            .json(&serde_json::json!({"points": batch}))
            .send()
            .await?
            .error_for_status()?;
    }
    Ok(())
}

/// Paginate the entire Qdrant collection, calling `process_page` on each batch
/// of points. Each page is dropped after processing, so only the aggregated
/// state (held by the caller via the closure) stays in memory.
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
        if let Some(off) = offset.clone() {
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
            .cloned()
            .unwrap_or_default();
        if points.is_empty() {
            break;
        }
        process_page(&points);

        offset = val["result"].get("next_page_offset").cloned();
        if offset.as_ref().is_none() || offset == Some(serde_json::Value::Null) {
            break;
        }
    }

    Ok(())
}

async fn qdrant_search(
    cfg: &Config,
    vector: &[f32],
    limit: usize,
) -> Result<Vec<serde_json::Value>, Box<dyn Error>> {
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
        .json::<serde_json::Value>()
        .await?;
    Ok(res["result"].as_array().cloned().unwrap_or_default())
}

async fn qdrant_retrieve_by_url(
    cfg: &Config,
    url_match: &str,
) -> Result<Vec<serde_json::Value>, Box<dyn Error>> {
    let client = http_client()?;
    let mut out = Vec::new();
    let mut offset: Option<serde_json::Value> = None;

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
        if let Some(off) = offset.clone() {
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
            .json::<serde_json::Value>()
            .await?;

        let points = val["result"]["points"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if points.is_empty() {
            break;
        }
        out.extend(points);

        offset = val["result"].get("next_page_offset").cloned();
        if offset.as_ref().is_none() || offset == Some(serde_json::Value::Null) {
            break;
        }
    }

    Ok(out)
}

async fn read_inputs(input: &str) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let path = PathBuf::from(input);
    if path.is_file() {
        return Ok(vec![(
            path.to_string_lossy().to_string(),
            tokio::fs::read_to_string(&path).await?,
        )]);
    }
    if path.is_dir() {
        // Directory listing is metadata-only and fast; file reads use tokio::fs.
        let mut files: Vec<PathBuf> = fs::read_dir(&path)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        files.sort();
        let mut out = Vec::new();
        for p in files {
            let content = tokio::fs::read_to_string(&p).await?;
            out.push((p.to_string_lossy().to_string(), content));
        }
        return Ok(out);
    }
    Ok(vec![(input.to_string(), input.to_string())])
}

fn chunk_text(text: &str) -> Vec<String> {
    const MAX: usize = 2000;
    const OVERLAP: usize = 200;

    // Collect byte offsets at each character boundary in a single pass.
    // This avoids materialising Vec<char> (4 bytes/char) and replaces the
    // char-by-char collect in the hot path with a direct &str memcpy.
    let offsets: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    if offsets.len() <= MAX {
        return vec![text.to_string()];
    }
    let char_count = offsets.len();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < char_count {
        let end = (i + MAX).min(char_count);
        let byte_start = offsets[i];
        let byte_end = if end < char_count {
            offsets[end]
        } else {
            text.len()
        };
        out.push(text[byte_start..byte_end].to_string());
        if end == char_count {
            break;
        }
        i = end.saturating_sub(OVERLAP);
    }
    out
}

fn payload_text(payload: &serde_json::Value) -> String {
    payload
        .get("chunk_text")
        .or_else(|| payload.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn payload_url(payload: &serde_json::Value) -> String {
    payload
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn payload_domain(payload: &serde_json::Value) -> String {
    payload
        .get("domain")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string()
}

#[derive(Debug, Clone, Copy)]
pub struct EmbedSummary {
    pub docs_embedded: usize,
    pub chunks_embedded: usize,
}

pub async fn embed_path_native(cfg: &Config, input: &str) -> Result<EmbedSummary, Box<dyn Error>> {
    if cfg.tei_url.is_empty() {
        return Err("TEI_URL not configured".into());
    }
    if cfg.qdrant_url.is_empty() {
        return Err("QDRANT_URL not configured".into());
    }

    let mut docs = read_inputs(input).await?;

    if docs.len() == 1 && !Path::new(input).exists() && input.starts_with("http") {
        let client = build_client(20)?;
        let html = fetch_html(&client, input).await?;
        docs = vec![(input.to_string(), to_markdown(&html))];
    }

    let mut all_points = Vec::new();
    let mut docs_embedded = 0usize;
    let mut collection_ensured = false;
    for (url, raw) in docs {
        let chunks = chunk_text(&raw);
        if chunks.is_empty() {
            continue;
        }
        docs_embedded += 1;
        let vectors = tei_embed(cfg, &chunks).await?;
        if vectors.is_empty() {
            return Err("TEI returned no vectors for this document".into());
        }
        if !collection_ensured {
            ensure_collection(cfg, vectors[0].len()).await?;
            collection_ensured = true;
        }

        let domain = Url::parse(&url)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "unknown".to_string());

        for (idx, (chunk, vecv)) in chunks.into_iter().zip(vectors.into_iter()).enumerate() {
            all_points.push(serde_json::json!({
                "id": Uuid::new_v4().to_string(),
                "vector": vecv,
                "payload": {
                    "url": url,
                    "domain": domain,
                    "source_command": "embed",
                    "content_type": "markdown",
                    "chunk_index": idx,
                    "chunk_text": chunk,
                    "scraped_at": Utc::now().to_rfc3339(),
                }
            }));
        }
    }

    qdrant_upsert(cfg, &all_points).await?;
    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({"chunks_embedded": all_points.len(), "collection": cfg.collection})
        );
    } else {
        println!(
            "{} embedded {} chunks into {}",
            symbol_for_status("completed"),
            all_points.len(),
            accent(&cfg.collection)
        );
    }
    Ok(EmbedSummary {
        docs_embedded,
        chunks_embedded: all_points.len(),
    })
}

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

    let mut query_vectors = tei_embed(cfg, std::slice::from_ref(&query)).await?;
    if query_vectors.is_empty() {
        return Err("TEI returned no vector for query".into());
    }
    let vector = query_vectors.remove(0);
    let hits = qdrant_search(cfg, &vector, 10).await?;

    if !cfg.json_output {
        println!("{}", primary(&format!("Query Results for \"{query}\"")));
        println!("{} {}\n", muted("Showing"), hits.len());
    }

    for (i, h) in hits.iter().enumerate() {
        let score = h["score"].as_f64().unwrap_or(0.0);
        let payload = h.get("payload").cloned().unwrap_or_default();
        let url = payload_url(&payload);
        let text = payload_text(&payload).replace('\n', " ");
        let end = text
            .char_indices()
            .nth(140)
            .map(|(i, _)| i)
            .unwrap_or(text.len());
        let snippet = &text[..end];
        if cfg.json_output {
            println!(
                "{}",
                serde_json::json!({"rank": i + 1, "score": score, "url": url, "snippet": snippet})
            );
        } else {
            println!(
                "  • {}. {} [{:.2}] {}",
                i + 1,
                status_text("completed"),
                score,
                accent(&url)
            );
            println!("    {snippet}");
        }
    }

    Ok(())
}

pub async fn run_retrieve_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let target = cfg.positional.first().ok_or("retrieve requires URL")?;
    let mut points = qdrant_retrieve_by_url(cfg, target).await?;
    if points.is_empty() {
        println!("No content found for URL: {}", target);
        return Ok(());
    }

    points.sort_by_key(|p| p["payload"]["chunk_index"].as_i64().unwrap_or(i64::MAX));

    let mut out = String::new();
    for p in &points {
        let payload = p.get("payload").cloned().unwrap_or_default();
        let t = payload_text(&payload);
        if !t.is_empty() {
            out.push_str(&t);
            out.push('\n');
        }
    }
    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "url": target,
                "chunks": points.len(),
                "content": out.trim()
            }))?
        );
    } else {
        println!("{}", primary(&format!("Retrieve Result for {target}")));
        println!("{} {}\n", muted("Chunks:"), points.len());
        println!("{}", out.trim());
    }
    Ok(())
}

pub async fn run_sources_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let mut by_url: BTreeMap<String, usize> = BTreeMap::new();
    qdrant_scroll_pages(cfg, |points| {
        for p in points {
            let payload = p.get("payload").cloned().unwrap_or_default();
            let url = payload_url(&payload);
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
    let mut by_domain: BTreeMap<String, (usize, BTreeSet<String>)> = BTreeMap::new();
    qdrant_scroll_pages(cfg, |points| {
        for p in points {
            let payload = p.get("payload").cloned().unwrap_or_default();
            let domain = payload_domain(&payload);
            let url = payload_url(&payload);
            let entry = by_domain.entry(domain).or_insert((0, BTreeSet::new()));
            entry.0 += 1;
            if !url.is_empty() {
                entry.1.insert(url);
            }
        }
    })
    .await?;
    if cfg.json_output {
        println!("{}", serde_json::to_string_pretty(&by_domain)?);
    } else {
        println!("{}", primary("Domains"));
        for (domain, (vectors, urls)) in by_domain {
            println!(
                "  • {} {}",
                accent(&domain),
                muted(&format!("urls={} vectors={}", urls.len(), vectors))
            );
        }
    }
    Ok(())
}

pub async fn run_stats_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let client = http_client()?;
    let info = client
        .get(format!(
            "{}/collections/{}",
            qdrant_base(cfg),
            cfg.collection
        ))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    let count = client
        .post(format!(
            "{}/collections/{}/points/count",
            qdrant_base(cfg),
            cfg.collection
        ))
        .json(&serde_json::json!({"exact": true}))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let stats = serde_json::json!({
        "collection": cfg.collection,
        "status": info["result"]["status"],
        "vectors_count": info["result"]["vectors_count"],
        "points_count": count["result"]["count"],
        "dimension": info["result"]["config"]["params"]["vectors"]["size"],
        "distance": info["result"]["config"]["params"]["vectors"]["distance"]
    });
    if cfg.json_output {
        println!("{}", serde_json::to_string_pretty(&stats)?);
    } else {
        println!("{}", primary("Vector Stats"));
        println!("  {} {}", muted("Collection:"), stats["collection"]);
        println!(
            "  {} {}",
            muted("Status:"),
            status_text(stats["status"].as_str().unwrap_or("unknown"))
        );
        println!("  {} {}", muted("Vectors:"), stats["vectors_count"]);
        println!("  {} {}", muted("Points:"), stats["points_count"]);
        println!("  {} {}", muted("Dimension:"), stats["dimension"]);
        println!("  {} {}", muted("Distance:"), stats["distance"]);
    }
    Ok(())
}

pub async fn run_ask_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    const MAX_CONTEXT_CHARS: usize = 12_000;

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

    let mut ask_vectors = tei_embed(cfg, std::slice::from_ref(&query)).await?;
    if ask_vectors.is_empty() {
        return Err("TEI returned no vector for ask query".into());
    }
    let vecq = ask_vectors.remove(0);
    let hits = qdrant_search(cfg, &vecq, 8).await?;
    let mut context = String::new();
    for h in hits {
        let payload = h.get("payload").cloned().unwrap_or_default();
        let url = payload_url(&payload);
        let txt = payload_text(&payload);
        if !txt.is_empty() {
            let entry = format!("Source: {}\n{}\n\n", url, txt);
            if context.len() + entry.len() > MAX_CONTEXT_CHARS {
                eprintln!(
                    "warning: context truncated at {} chars (max {}); some chunks omitted",
                    context.len(),
                    MAX_CONTEXT_CHARS
                );
                break;
            }
            context.push_str(&entry);
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
                {"role": "system", "content": "Answer only using provided context."},
                {"role": "user", "content": format!("Question: {}\n\nContext:\n{}", query, context)}
            ],
            "temperature": 0.1
        }));

    if !cfg.openai_api_key.is_empty() {
        req = req.bearer_auth(&cfg.openai_api_key);
    }

    let response = req.send().await?.error_for_status()?;
    let json: serde_json::Value = response.json().await?;
    let answer = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(no answer)");
    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({"query": query, "answer": answer}))?
        );
    } else {
        println!("{}", primary("Conversation"));
        println!("  {} {}", primary("You:"), query);
        println!("  {} {}", primary("Assistant:"), answer);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_text_short_returns_single() {
        let text = "hello world";
        let chunks = chunk_text(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn test_chunk_text_exactly_2000_chars() {
        let text = "a".repeat(2000);
        let chunks = chunk_text(&text);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_chunk_text_2001_chars_gives_two() {
        let text = "a".repeat(2001);
        let chunks = chunk_text(&text);
        assert_eq!(chunks.len(), 2);
        // Second chunk should have 201 chars (1 new + 200 overlap)
        assert_eq!(chunks[1].len(), 201);
    }

    #[test]
    fn test_chunk_text_multibyte_utf8_no_panic() {
        // CJK characters --- 3 bytes each
        let text = "\u{4e2d}".repeat(2500);
        let chunks = chunk_text(&text);
        // Must not panic; each chunk must be valid UTF-8
        for chunk in &chunks {
            assert!(std::str::from_utf8(chunk.as_bytes()).is_ok());
        }
    }

    #[test]
    fn test_chunk_text_empty_string() {
        let chunks = chunk_text("");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "");
    }

    #[test]
    fn test_chunk_text_overlap_content() {
        // Create text that's 2200 chars: chunks should overlap by 200
        let text: String = (0..2200)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        let chunks = chunk_text(&text);
        assert_eq!(chunks.len(), 2);
        // The last 200 chars of chunk[0] should equal the first 200 chars of chunk[1]
        let overlap_from_first = &chunks[0][chunks[0].len() - 200..];
        let overlap_from_second = &chunks[1][..200];
        assert_eq!(overlap_from_first, overlap_from_second);
    }

    #[test]
    fn test_chunk_text_large_document() {
        let text = "x".repeat(10_000);
        let chunks = chunk_text(&text);
        // With 2000 char chunks and 200 overlap, step is 1800
        // ceil((10000 - 2000) / 1800) + 1 = ceil(8000/1800) + 1 = 5 + 1 = 6
        assert!(chunks.len() >= 5);
        // All chunks except possibly the last should be exactly 2000 chars
        for chunk in &chunks[..chunks.len() - 1] {
            assert_eq!(chunk.len(), 2000);
        }
    }
}
