use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::content::to_markdown;
use crate::axon_cli::crates::core::http::{fetch_html, http_client};
use crate::axon_cli::crates::core::ui::{accent, symbol_for_status};
use crate::axon_cli::crates::vector::ops_v2::input;
use chrono::Utc;
use futures_util::stream::{FuturesUnordered, StreamExt};
use reqwest::StatusCode;
use spider::url::Url;
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub struct EmbedSummary {
    pub docs_embedded: usize,
    pub chunks_embedded: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct EmbedProgress {
    pub docs_total: usize,
    pub docs_completed: usize,
    pub chunks_embedded: usize,
}

#[derive(Debug)]
struct PreparedDoc {
    url: String,
    domain: String,
    chunks: Vec<String>,
}

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

pub(crate) async fn tei_embed(
    cfg: &Config,
    inputs: &[String],
) -> Result<Vec<Vec<f32>>, Box<dyn Error>> {
    if inputs.is_empty() {
        return Ok(Vec::new());
    }
    let client = http_client()?;
    let mut vectors = Vec::new();

    let configured = env_usize_clamped("TEI_MAX_CLIENT_BATCH_SIZE", 64, 1, 4096);
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
    let upsert_batch_size = env_usize_clamped("AXON_QDRANT_UPSERT_BATCH_SIZE", 256, 1, 4096);
    let url = format!(
        "{}/collections/{}/points?wait=true",
        qdrant_base(cfg),
        cfg.collection
    );
    for batch in points.chunks(upsert_batch_size) {
        client
            .put(&url)
            .json(&serde_json::json!({"points": batch}))
            .send()
            .await?
            .error_for_status()?;
    }
    Ok(())
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

async fn embed_prepared_doc(
    cfg: &Config,
    doc: PreparedDoc,
) -> Result<(usize, Vec<serde_json::Value>), Box<dyn Error>> {
    let vectors = tei_embed(cfg, &doc.chunks).await?;
    if vectors.is_empty() {
        return Err(format!("TEI returned no vectors for {}", doc.url).into());
    }
    if vectors.len() != doc.chunks.len() {
        return Err(format!(
            "TEI vector count mismatch for {}: {} vectors for {} chunks",
            doc.url,
            vectors.len(),
            doc.chunks.len()
        )
        .into());
    }
    let dim = vectors[0].len();
    let timestamp = Utc::now().to_rfc3339();
    let mut points = Vec::with_capacity(vectors.len());
    for (idx, (chunk, vecv)) in doc.chunks.into_iter().zip(vectors.into_iter()).enumerate() {
        points.push(serde_json::json!({
            "id": Uuid::new_v4().to_string(),
            "vector": vecv,
            "payload": {
                "url": doc.url,
                "domain": doc.domain,
                "source_command": "embed",
                "content_type": "markdown",
                "chunk_index": idx,
                "chunk_text": chunk,
                "scraped_at": timestamp,
            }
        }));
    }
    Ok((dim, points))
}

pub async fn embed_path_native(cfg: &Config, input: &str) -> Result<EmbedSummary, Box<dyn Error>> {
    embed_path_native_with_progress(cfg, input, None).await
}

pub async fn embed_path_native_with_progress(
    cfg: &Config,
    input: &str,
    progress_tx: Option<tokio::sync::mpsc::UnboundedSender<EmbedProgress>>,
) -> Result<EmbedSummary, Box<dyn Error>> {
    if cfg.tei_url.is_empty() {
        return Err("TEI_URL not configured".into());
    }
    if cfg.qdrant_url.is_empty() {
        return Err("QDRANT_URL not configured".into());
    }

    let mut docs = read_inputs(input).await?;

    if docs.len() == 1 && !Path::new(input).exists() && input.starts_with("http") {
        let client = http_client()?.clone();
        let html = fetch_html(&client, input).await?;
        docs = vec![(input.to_string(), to_markdown(&html))];
    }

    let mut prepared = Vec::new();
    for (url, raw) in docs {
        if raw.trim().is_empty() {
            continue;
        }
        let chunks = input::chunk_text(&raw);
        let domain = Url::parse(&url)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "unknown".to_string());
        prepared.push(PreparedDoc {
            url,
            domain,
            chunks,
        });
    }
    if prepared.is_empty() {
        if let Some(tx) = &progress_tx {
            let _ = tx.send(EmbedProgress {
                docs_total: 0,
                docs_completed: 0,
                chunks_embedded: 0,
            });
        }
        return Ok(EmbedSummary {
            docs_embedded: 0,
            chunks_embedded: 0,
        });
    }

    let docs_embedded = prepared.len();
    let doc_concurrency = env_usize_clamped(
        "AXON_EMBED_DOC_CONCURRENCY",
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(8)
            .clamp(2, 16),
        1,
        64,
    );
    let flush_point_threshold = env_usize_clamped("AXON_QDRANT_POINT_BUFFER", 2048, 128, 16384);

    let mut work = prepared.into_iter();
    let mut inflight = FuturesUnordered::new();
    for _ in 0..doc_concurrency {
        if let Some(doc) = work.next() {
            inflight.push(embed_prepared_doc(cfg, doc));
        }
    }

    let mut chunks_embedded = 0usize;
    let mut docs_completed = 0usize;
    let mut pending_points: Vec<serde_json::Value> = Vec::new();
    let mut collection_dim: Option<usize> = None;

    while let Some(result) = inflight.next().await {
        let (dim, mut points) = result?;
        match collection_dim {
            None => {
                ensure_collection(cfg, dim).await?;
                collection_dim = Some(dim);
            }
            Some(existing) if existing != dim => {
                return Err(format!(
                    "TEI embedding dimension mismatch: expected {}, got {}",
                    existing, dim
                )
                .into())
            }
            _ => {}
        }
        chunks_embedded += points.len();
        docs_completed += 1;
        if let Some(tx) = &progress_tx {
            let _ = tx.send(EmbedProgress {
                docs_total: docs_embedded,
                docs_completed,
                chunks_embedded,
            });
        }
        pending_points.append(&mut points);
        if pending_points.len() >= flush_point_threshold {
            qdrant_upsert(cfg, &pending_points).await?;
            pending_points.clear();
        }

        if let Some(doc) = work.next() {
            inflight.push(embed_prepared_doc(cfg, doc));
        }
    }
    if !pending_points.is_empty() {
        qdrant_upsert(cfg, &pending_points).await?;
    }

    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({"chunks_embedded": chunks_embedded, "collection": cfg.collection})
        );
    } else {
        println!(
            "{} embedded {} chunks into {}",
            symbol_for_status("completed"),
            chunks_embedded,
            accent(&cfg.collection)
        );
    }
    Ok(EmbedSummary {
        docs_embedded,
        chunks_embedded,
    })
}
