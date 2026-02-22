use crate::crates::core::config::Config;
use crate::crates::core::content::to_markdown;
use crate::crates::core::http::{fetch_html, http_client};
use crate::crates::core::ui::{accent, symbol_for_status};
use crate::crates::vector::ops::input;
use crate::crates::vector::ops::qdrant::{
    env_usize_clamped, qdrant_base, qdrant_delete_by_url_filter,
};
use chrono::Utc;
use futures_util::stream::{FuturesUnordered, StreamExt};
use rand::Rng as _;
use reqwest::StatusCode;
use spider::url::Url;
use std::collections::HashSet;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use uuid::Uuid;

/// Track which collections have already been initialized this process lifetime.
/// Avoids redundant GET+PUT to Qdrant on every document upsert.
static INITIALIZED_COLLECTIONS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn collection_needs_init(name: &str) -> bool {
    let map = INITIALIZED_COLLECTIONS.get_or_init(|| Mutex::new(HashSet::new()));
    let mut set = map.lock().expect("INITIALIZED_COLLECTIONS mutex poisoned");
    if set.contains(name) {
        return false;
    }
    set.insert(name.to_owned());
    true
}

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

pub(crate) async fn tei_embed(
    cfg: &Config,
    inputs: &[String],
) -> Result<Vec<Vec<f32>>, Box<dyn Error>> {
    if inputs.is_empty() {
        return Ok(Vec::new());
    }
    let client = http_client()?;
    let mut vectors = Vec::new();

    let configured = env_usize_clamped("TEI_MAX_CLIENT_BATCH_SIZE", 128, 1, 4096);
    let batch_size = configured.min(128);
    let embed_url = format!("{}/embed", cfg.tei_url.trim_end_matches('/'));

    let mut stack: Vec<&[String]> = inputs.chunks(batch_size).collect();
    // Reverse stack so we process in original order
    stack.reverse();

    while let Some(chunk) = stack.pop() {
        let mut attempt = 0;
        let max_attempts = 5;

        loop {
            let resp = client
                .post(&embed_url)
                .json(&serde_json::json!({"inputs": chunk}))
                .send()
                .await?;

            let status = resp.status();
            if status.is_success() {
                let mut batch_vectors = resp.json::<Vec<Vec<f32>>>().await?;
                vectors.append(&mut batch_vectors);
                break;
            }

            if status == StatusCode::PAYLOAD_TOO_LARGE && chunk.len() > 1 {
                let mid = chunk.len() / 2;
                let (left, right) = chunk.split_at(mid);
                stack.push(right);
                stack.push(left);
                break;
            }

            if (status == StatusCode::TOO_MANY_REQUESTS
                || status == StatusCode::SERVICE_UNAVAILABLE)
                && attempt < max_attempts
            {
                attempt += 1;
                // Jittered exponential backoff: 200ms, 400ms, 800ms...
                let delay = Duration::from_millis(200 * (2u64.pow(attempt as u32)));
                let jitter = Duration::from_millis(rand::rng().random_range(0..100));
                tokio::time::sleep(delay + jitter).await;
                continue;
            }

            return Err(format!(
                "TEI request failed with status {} for {} (attempt {})",
                status, embed_url, attempt
            )
            .into());
        }
    }
    Ok(vectors)
}

async fn ensure_collection(cfg: &Config, dim: usize) -> Result<(), Box<dyn Error>> {
    let client = http_client()?;
    let url = format!("{}/collections/{}", qdrant_base(cfg), cfg.collection);

    // GET first: if the collection (or alias) already exists with matching dimensions, skip the PUT.
    // This avoids a 400 from Qdrant when cfg.collection matches an alias name rather than a real collection.
    let get_resp = client.get(&url).send().await?;
    if get_resp.status().is_success() {
        let body: serde_json::Value = get_resp.json().await?;
        let existing_dim = body
            .pointer("/result/config/params/vectors/size")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        if existing_dim == dim {
            return Ok(());
        }
    }

    // Collection absent or dimension mismatch — create/update it.
    let create = serde_json::json!({
        "vectors": {"size": dim, "distance": "Cosine"}
    });
    let resp = client.put(&url).json(&create).send().await?;
    // 409 Conflict = racing caller already created it; treat as success.
    if resp.status() != StatusCode::CONFLICT {
        resp.error_for_status()?;
    }
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
    match tokio::fs::metadata(&path).await {
        Ok(meta) if meta.is_file() => Ok(vec![(
            path.to_string_lossy().to_string(),
            tokio::fs::read_to_string(&path).await?,
        )]),
        Ok(meta) if meta.is_dir() => {
            let mut dir = tokio::fs::read_dir(&path).await?;
            let mut files = Vec::new();
            while let Some(entry) = dir.next_entry().await? {
                let p = entry.path();
                if tokio::fs::metadata(&p).await.is_ok_and(|m| m.is_file()) {
                    files.push(p);
                }
            }
            files.sort();
            let mut out = Vec::new();
            for p in files {
                let content = tokio::fs::read_to_string(&p).await?;
                out.push((p.to_string_lossy().to_string(), content));
            }
            Ok(out)
        }
        _ => Ok(vec![(input.to_string(), input.to_string())]),
    }
}

async fn embed_prepared_doc(
    cfg: &Config,
    doc: PreparedDoc,
) -> Result<(usize, Vec<serde_json::Value>), Box<dyn Error>> {
    // Delete-before-upsert is intentional: although point IDs are deterministic
    // (Uuid::new_v5 on url:chunk_index), re-embedding may produce *fewer* chunks
    // than the previous version, leaving orphan points with stale chunk indices.
    // The filter delete removes ALL points for this URL before upserting the fresh set.
    qdrant_delete_by_url_filter(cfg, &doc.url).await?;
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
        let point_id = Uuid::new_v5(
            &Uuid::NAMESPACE_URL,
            format!("{}:{}", doc.url, idx).as_bytes(),
        );
        points.push(serde_json::json!({
            "id": point_id.to_string(),
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

/// Embed arbitrary text content with explicit source metadata into Qdrant.
///
/// Unlike `embed_path_native` which takes file/URL inputs, this function accepts
/// pre-fetched text and attaches `source_type` and `title` to every Qdrant point
/// payload — enabling filtered queries like "search only GitHub content".
///
/// Returns the number of chunks embedded.
pub async fn embed_text_with_metadata(
    cfg: &Config,
    content: &str,
    url: &str,
    source_type: &str,
    title: Option<&str>,
) -> Result<usize, Box<dyn Error>> {
    if content.trim().is_empty() {
        return Ok(0);
    }
    let chunks = input::chunk_text(content);
    if chunks.is_empty() {
        return Ok(0);
    }
    let vectors = tei_embed(cfg, &chunks).await?;
    if vectors.is_empty() {
        return Err(format!("TEI returned no vectors for {url}").into());
    }
    if vectors.len() != chunks.len() {
        return Err(format!(
            "TEI vector count mismatch for {url}: {} vectors for {} chunks",
            vectors.len(),
            chunks.len()
        )
        .into());
    }
    let dim = vectors[0].len();
    if collection_needs_init(&cfg.collection) {
        ensure_collection(cfg, dim).await?;
    }
    let domain = spider::url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "unknown".to_string());
    let timestamp = chrono::Utc::now().to_rfc3339();
    let mut points = Vec::with_capacity(vectors.len());
    for (idx, (chunk, vecv)) in chunks.into_iter().zip(vectors.into_iter()).enumerate() {
        let point_id = uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_URL,
            format!("{url}:{idx}").as_bytes(),
        );
        let mut payload = serde_json::json!({
            "url": url,
            "domain": domain,
            "source_type": source_type,
            "source_command": source_type,
            "content_type": "text",
            "chunk_index": idx,
            "chunk_text": chunk,
            "scraped_at": timestamp,
        });
        if let Some(t) = title {
            payload["title"] = serde_json::Value::String(t.to_string());
        }
        points.push(serde_json::json!({
            "id": point_id.to_string(),
            "vector": vecv,
            "payload": payload,
        }));
    }
    qdrant_upsert(cfg, &points).await?;
    Ok(points.len())
}

pub async fn embed_path_native(cfg: &Config, input: &str) -> Result<EmbedSummary, Box<dyn Error>> {
    embed_path_native_with_progress(cfg, input, None).await
}

pub async fn embed_path_native_with_progress(
    cfg: &Config,
    input: &str,
    progress_tx: Option<tokio::sync::mpsc::UnboundedSender<EmbedProgress>>,
) -> Result<EmbedSummary, Box<dyn Error>> {
    embed_path_native_with_progress_impl(cfg, input, progress_tx).await
}

async fn embed_path_native_with_progress_impl(
    cfg: &Config,
    input: &str,
    progress_tx: Option<tokio::sync::mpsc::UnboundedSender<EmbedProgress>>,
) -> Result<EmbedSummary, Box<dyn Error>> {
    validate_embed_config(cfg)?;
    let prepared = prepare_embed_docs(input).await?;
    if prepared.is_empty() {
        return emit_empty_embed(progress_tx);
    }
    let summary = run_embed_pipeline(cfg, prepared, progress_tx).await?;
    emit_embed_summary(cfg, summary.chunks_embedded);
    Ok(summary)
}

fn validate_embed_config(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if cfg.tei_url.is_empty() {
        return Err("TEI_URL not configured".into());
    }
    if cfg.qdrant_url.is_empty() {
        return Err("QDRANT_URL not configured".into());
    }
    Ok(())
}

async fn prepare_embed_docs(input: &str) -> Result<Vec<PreparedDoc>, Box<dyn Error>> {
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
    Ok(prepared)
}

fn emit_empty_embed(
    progress_tx: Option<tokio::sync::mpsc::UnboundedSender<EmbedProgress>>,
) -> Result<EmbedSummary, Box<dyn Error>> {
    if let Some(tx) = &progress_tx {
        let _ = tx.send(EmbedProgress {
            docs_total: 0,
            docs_completed: 0,
            chunks_embedded: 0,
        });
    }
    Ok(EmbedSummary {
        docs_embedded: 0,
        chunks_embedded: 0,
    })
}

async fn run_embed_pipeline(
    cfg: &Config,
    prepared: Vec<PreparedDoc>,
    progress_tx: Option<tokio::sync::mpsc::UnboundedSender<EmbedProgress>>,
) -> Result<EmbedSummary, Box<dyn Error>> {
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
    let flush_point_threshold = env_usize_clamped("AXON_QDRANT_POINT_BUFFER", 256, 128, 16384);

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
                if collection_needs_init(&cfg.collection) {
                    ensure_collection(cfg, dim).await?;
                }
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

    Ok(EmbedSummary {
        docs_embedded,
        chunks_embedded,
    })
}

fn emit_embed_summary(cfg: &Config, chunks_embedded: usize) {
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
}
