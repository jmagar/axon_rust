use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::content::to_markdown;
use crate::axon_cli::crates::core::http::{fetch_html, http_client, normalize_url};
use crate::axon_cli::crates::core::ui::{accent, muted, primary, status_text, symbol_for_status};
use chrono::Utc;
use futures_util::stream::{self, FuturesUnordered, StreamExt};
use reqwest::StatusCode;
use serde::Deserialize;
use spider::url::Url;
use sqlx::{postgres::PgPoolOptions, Row};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;
use uuid::Uuid;

fn qdrant_base(cfg: &Config) -> String {
    cfg.qdrant_url.trim_end_matches('/').to_string()
}

async fn pg_pool_for_stats(cfg: &Config) -> Option<sqlx::PgPool> {
    if cfg.pg_url.is_empty() {
        return None;
    }
    tokio::time::timeout(
        Duration::from_secs(3),
        PgPoolOptions::new().max_connections(2).connect(&cfg.pg_url),
    )
    .await
    .ok()
    .and_then(Result::ok)
}

async fn table_exists(pool: &sqlx::PgPool, table: &str) -> Result<bool, sqlx::Error> {
    let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
        .bind(table)
        .fetch_one(pool)
        .await?;
    Ok(exists)
}

async fn count_table_rows(pool: &sqlx::PgPool, table: &str) -> Result<i64, sqlx::Error> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    sqlx::query_scalar::<_, i64>(&sql).fetch_one(pool).await
}

async fn command_count(pool: &sqlx::PgPool, command: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM axon_command_runs WHERE command = $1")
        .bind(command)
        .fetch_one(pool)
        .await
}

fn env_usize_clamped(key: &str, default: usize, min: usize, max: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v >= min)
        .unwrap_or(default)
        .clamp(min, max)
}

fn env_f64_clamped(key: &str, default: f64, min: f64, max: f64) -> f64 {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default)
        .clamp(min, max)
}

#[derive(Debug, Clone, Default, Deserialize)]
struct QdrantPayload {
    #[serde(default)]
    url: String,
    #[serde(default)]
    chunk_text: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    title: String,
    chunk_header: Option<String>,
    chunk_index: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct QdrantPoint {
    #[serde(default)]
    payload: QdrantPayload,
}

#[derive(Debug, Clone, Deserialize)]
struct QdrantSearchHit {
    score: f64,
    #[serde(default)]
    payload: QdrantPayload,
}

#[derive(Debug, Deserialize)]
struct QdrantSearchResponse {
    #[serde(default)]
    result: Vec<QdrantSearchHit>,
}

#[derive(Debug, Clone, Deserialize)]
struct QdrantSearchGroup {
    #[serde(default)]
    hits: Vec<QdrantSearchHit>,
}

#[derive(Debug, Deserialize)]
struct QdrantSearchGroupsResult {
    #[serde(default)]
    groups: Vec<QdrantSearchGroup>,
}

#[derive(Debug, Deserialize)]
struct QdrantSearchGroupsResponse {
    result: QdrantSearchGroupsResult,
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

async fn tei_embed(cfg: &Config, inputs: &[String]) -> Result<Vec<Vec<f32>>, Box<dyn Error>> {
    if inputs.is_empty() {
        return Ok(Vec::new());
    }
    let client = http_client()?;
    let mut vectors = Vec::new();

    // Respect TEI max client batch size when provided, but cap default to avoid huge request bodies.
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
            .map(Vec::as_slice)
            .unwrap_or(&[]);
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

async fn qdrant_search(
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

async fn qdrant_search_groups(
    cfg: &Config,
    vector: &[f32],
    group_limit: usize,
    group_size: usize,
) -> Result<Vec<QdrantSearchGroup>, Box<dyn Error>> {
    let client = http_client()?;
    let url = format!(
        "{}/collections/{}/points/search/groups",
        qdrant_base(cfg),
        cfg.collection
    );
    let res = client
        .post(url)
        .json(&serde_json::json!({
            "vector": vector,
            "group_by": "url",
            "limit": group_limit,
            "group_size": group_size,
            "with_payload": true,
            "with_vector": false
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<QdrantSearchGroupsResponse>()
        .await?;
    Ok(res.result.groups)
}

async fn qdrant_retrieve_by_url(
    cfg: &Config,
    url_match: &str,
    max_points: Option<usize>,
) -> Result<Vec<QdrantPoint>, Box<dyn Error>> {
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
            .json::<QdrantScrollResponse>()
            .await?;

        let points = val.result.points;
        if points.is_empty() {
            break;
        }
        out.extend(points);
        if let Some(max) = max_points {
            if out.len() >= max {
                out.truncate(max);
                break;
            }
        }

        offset = val.result.next_page_offset;
        if offset.as_ref().is_none() {
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

pub fn chunk_text(text: &str) -> Vec<String> {
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

fn tokenize_query_terms(text: &str) -> Vec<String> {
    tokenize_query(text)
}

fn tokenize_query_impl(text: &str) -> Vec<String> {
    static STOP_WORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
        HashSet::from([
            "the", "and", "for", "with", "that", "this", "from", "into", "how", "what", "where",
            "when", "you", "your", "are", "can", "does", "create", "make",
        ])
    });

    text.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 3 && !STOP_WORDS.contains(*t))
        .map(str::to_string)
        .collect()
}

fn canonical_url_key(raw: &str) -> String {
    let normalized = normalize_url(raw);
    if !normalized.contains('?') && !normalized.contains('#') {
        return normalized.trim_end_matches('/').to_string();
    }
    let Ok(mut parsed) = Url::parse(&normalized) else {
        return normalized.trim_end_matches('/').to_string();
    };
    parsed.set_fragment(None);
    parsed.set_query(None);
    let mut s = parsed.to_string();
    if s.ends_with('/') {
        s.pop();
    }
    s
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

fn payload_text_typed(payload: &QdrantPayload) -> &str {
    if !payload.chunk_text.is_empty() {
        payload.chunk_text.as_str()
    } else {
        payload.text.as_str()
    }
}

fn payload_url_typed(payload: &QdrantPayload) -> &str {
    payload.url.as_str()
}

#[derive(Debug, Clone)]
struct AskCandidate {
    score: f64,
    url: String,
    chunk_text: String,
    url_tokens: HashSet<String>,
    chunk_tokens: HashSet<String>,
    rerank_score: f64,
}

#[derive(Debug, Clone)]
struct QueryCandidate {
    score: f64,
    url: String,
    chunk_text: String,
    chunk_index: i64,
    chunk_header: Option<String>,
    title: String,
}

fn tokenize_query(text: &str) -> Vec<String> {
    tokenize_query_impl(text)
}

fn sentence_candidates(text: &str) -> Vec<String> {
    text.split(|c: char| matches!(c, '.' | '!' | '?' | '\n'))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn is_probable_header(sentence: &str) -> bool {
    if sentence.len() > 60 {
        return false;
    }
    let mut words = 0usize;
    for _ in sentence.split_whitespace() {
        words += 1;
        if words > 5 {
            return false;
        }
    }
    true
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    let end = input
        .char_indices()
        .nth(max_chars)
        .map(|(idx, _)| idx)
        .unwrap_or(input.len());
    input[..end].to_string()
}

fn meaningful_snippet(chunk_text: &str, query: &str) -> String {
    let terms = tokenize_query_terms(query);
    meaningful_snippet_with_terms(chunk_text, &terms)
}

fn meaningful_snippet_with_terms(chunk_text: &str, terms: &[String]) -> String {
    if terms.is_empty() {
        return truncate_chars(chunk_text.trim(), 180);
    }

    let mut best: Option<(f64, String)> = None;
    for sentence in sentence_candidates(chunk_text) {
        if sentence.len() < 20 || is_probable_header(&sentence) {
            continue;
        }
        let lowered = sentence.to_ascii_lowercase();
        let mut hits = 0usize;
        for t in &terms {
            if lowered.contains(t) {
                hits += 1;
            }
        }
        let coverage = hits as f64 / terms.len() as f64;
        let length_bonus = (sentence.len().min(220) as f64) / 220.0 * 0.10;
        let score = coverage + length_bonus;
        if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
            best = Some((score, sentence));
        }
    }

    if let Some((_, sentence)) = best {
        return truncate_chars(sentence.trim(), 180);
    }

    let fallback = chunk_text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && line.len() >= 30)
        .unwrap_or(chunk_text.trim());
    truncate_chars(fallback, 180)
}

#[derive(Debug, Clone)]
struct QueryPreview {
    item: QueryCandidate,
    snippet: String,
}

fn select_best_preview_item(items: &[QueryCandidate], terms: &[String]) -> Option<QueryPreview> {
    items
        .iter()
        .cloned()
        .map(|item| {
            let snippet = meaningful_snippet_with_terms(&item.chunk_text, terms);
            let lowered = snippet.to_ascii_lowercase();
            let term_hits = terms
                .iter()
                .filter(|t| lowered.contains(t.as_str()))
                .count();
            let lexical = if terms.is_empty() {
                0.0
            } else {
                term_hits as f64 / terms.len() as f64
            };
            let header_penalty = if item
                .chunk_header
                .as_ref()
                .map(|h| h.len() <= 60)
                .unwrap_or(false)
            {
                0.05
            } else {
                0.0
            };
            let preview_score = item.score + lexical * 0.35 - header_penalty;
            (preview_score, QueryPreview { item, snippet })
        })
        .max_by(|(a, _), (b, _)| a.total_cmp(b))
        .map(|(_, preview)| preview)
}

fn tokenize_text_set(text: &str) -> HashSet<String> {
    tokenize_query(text).into_iter().collect()
}

fn tokenize_path_set(path_or_url: &str) -> HashSet<String> {
    let path = Url::parse(path_or_url)
        .ok()
        .map(|u| u.path().to_string())
        .unwrap_or_else(|| path_or_url.to_string());
    path.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(str::to_string)
        .collect()
}

fn rerank_ask_candidates(candidates: &[AskCandidate], query: &str) -> Vec<AskCandidate> {
    let tokens: Vec<String> = tokenize_query(query);
    if tokens.is_empty() {
        return candidates.to_vec();
    }

    let mut reranked = candidates
        .iter()
        .cloned()
        .map(|mut candidate| {
            let mut lexical_boost = 0.0f64;
            for token in &tokens {
                if candidate.url_tokens.contains(token) {
                    lexical_boost += 0.045;
                }
                if candidate.chunk_tokens.contains(token) {
                    lexical_boost += 0.015;
                }
            }
            lexical_boost = lexical_boost.min(0.30);

            let docs_boost = if candidate.url.contains("/docs/")
                || candidate.url.contains("/guides/")
                || candidate.url.contains("/api/")
                || candidate.url.contains("/reference/")
            {
                0.04
            } else {
                0.0
            };
            candidate.rerank_score = candidate.score + lexical_boost + docs_boost;
            candidate
        })
        .collect::<Vec<_>>();
    reranked.sort_by(|a, b| {
        b.rerank_score
            .partial_cmp(&a.rerank_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    reranked
}

fn select_diverse_candidates(
    candidates: &[AskCandidate],
    target_count: usize,
    max_per_url: usize,
) -> Vec<AskCandidate> {
    if candidates.len() <= target_count {
        return candidates.to_vec();
    }

    let mut selected: Vec<AskCandidate> = Vec::new();
    let mut per_url_count: HashMap<String, usize> = HashMap::new();
    let mut selected_indices: HashSet<usize> = HashSet::new();

    // Pass 1: one per URL for source diversity.
    for (i, candidate) in candidates.iter().enumerate() {
        if selected.len() >= target_count {
            break;
        }
        if per_url_count.contains_key(&candidate.url) {
            continue;
        }
        selected.push(candidate.clone());
        per_url_count.insert(candidate.url.clone(), 1);
        selected_indices.insert(i);
    }

    // Pass 2: fill remaining slots up to max-per-url, skipping already-selected.
    for (i, candidate) in candidates.iter().enumerate() {
        if selected.len() >= target_count {
            break;
        }
        if selected_indices.contains(&i) {
            continue;
        }
        let used = *per_url_count.get(&candidate.url).unwrap_or(&0);
        if used >= max_per_url {
            continue;
        }
        selected.push(candidate.clone());
        per_url_count.insert(candidate.url.clone(), used + 1);
        selected_indices.insert(i);
    }

    selected
}

fn render_full_doc_from_points(mut points: Vec<QdrantPoint>) -> String {
    points.sort_by_key(|p| p.payload.chunk_index.unwrap_or(i64::MAX));
    let mut text = String::new();
    for point in points {
        let chunk = payload_text_typed(&point.payload);
        if chunk.is_empty() {
            continue;
        }
        text.push_str(&chunk);
        text.push('\n');
    }
    text.trim().to_string()
}

fn query_snippet(payload: &QdrantPayload) -> String {
    let text = payload_text_typed(payload).replace('\n', " ");
    let end = text
        .char_indices()
        .nth(140)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len());
    text[..end].to_string()
}

pub fn url_lookup_candidates(target: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let normalized = normalize_url(target);
    let variants = [
        target.to_string(),
        normalized.clone(),
        normalized.trim_end_matches('/').to_string(),
        format!("{}/", normalized.trim_end_matches('/')),
    ];
    for variant in variants {
        if variant.is_empty() {
            continue;
        }
        if seen.insert(variant.clone()) {
            out.push(variant);
        }
    }
    out
}

fn push_context_entry(
    entries: &mut Vec<String>,
    context_char_count: &mut usize,
    entry: String,
    separator: &str,
    max_chars: usize,
) -> bool {
    let projected = if entries.is_empty() {
        entry.len()
    } else {
        *context_char_count + separator.len() + entry.len()
    };
    if projected > max_chars {
        return false;
    }
    entries.push(entry);
    *context_char_count = projected;
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
        let chunks = chunk_text(&raw);
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
    let requested_limit = cfg.search_limit.max(1);
    let query_terms = tokenize_query_terms(&query);
    let group_limit = (requested_limit.saturating_mul(2)).clamp(requested_limit, 1000);
    let group_size = 10usize;
    let groups = qdrant_search_groups(cfg, &vector, group_limit, group_size).await?;

    let mut grouped: HashMap<String, Vec<QueryCandidate>> = HashMap::new();
    for group in groups {
        for hit in group.hits {
            let payload = hit.payload;
            let url = payload_url_typed(&payload).to_string();
            let chunk_text = payload_text_typed(&payload).to_string();
            if url.is_empty() || chunk_text.is_empty() {
                continue;
            }
            let key = canonical_url_key(&url);
            grouped.entry(key).or_default().push(QueryCandidate {
                score: hit.score,
                url,
                chunk_text,
                chunk_index: payload.chunk_index.unwrap_or(0),
                chunk_header: payload.chunk_header,
                title: payload.title,
            });
        }
    }

    let mut previews = grouped
        .into_values()
        .filter_map(|items| {
            select_best_preview_item(&items, &query_terms).map(|best| (best, items.len()))
        })
        .collect::<Vec<_>>();
    previews.sort_by(|(a, _), (b, _)| {
        b.item
            .score
            .partial_cmp(&a.item.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    previews.truncate(requested_limit);

    if !cfg.json_output {
        println!("{}", primary(&format!("Query Results for \"{query}\"")));
        println!("{} {}\n", muted("Showing"), previews.len());
    }

    for (i, (item, chunk_count)) in previews.iter().enumerate() {
        let score = item.item.score;
        let url = &item.item.url;
        let snippet = &item.snippet;
        let header_display = item
            .item
            .chunk_header
            .as_ref()
            .filter(|h| !h.is_empty())
            .map(|h| format!(" - {h}"))
            .unwrap_or_default();
        if cfg.json_output {
            println!(
                "{}",
                serde_json::json!({
                    "rank": i + 1,
                    "score": score,
                    "url": url,
                    "snippet": snippet,
                    "chunk_index": item.item.chunk_index,
                    "chunks": chunk_count,
                    "chunk_header": item.item.chunk_header.clone(),
                    "title": if item.item.title.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(item.item.title.clone()) }
                })
            );
        } else {
            println!(
                "  • {}. {} [{:.2}] {}{} {}",
                i + 1,
                status_text("completed"),
                score,
                accent(url),
                muted(&format!(" ({} chunks)", chunk_count)),
                muted(&header_display)
            );
            println!("    {}", snippet);
        }
    }

    Ok(())
}

pub async fn run_retrieve_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let target = cfg.positional.first().ok_or("retrieve requires URL")?;
    let mut points = Vec::new();
    for candidate in url_lookup_candidates(target) {
        points = qdrant_retrieve_by_url(cfg, &candidate, None).await?;
        if !points.is_empty() {
            break;
        }
    }
    if points.is_empty() {
        println!("No content found for URL: {}", target);
        return Ok(());
    }

    points.sort_by_key(|p| p.payload.chunk_index.unwrap_or(i64::MAX));

    let mut out = String::new();
    for p in &points {
        let t = payload_text_typed(&p.payload);
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
    let docs_count = client
        .post(format!(
            "{}/collections/{}/points/count",
            qdrant_base(cfg),
            cfg.collection
        ))
        .json(&serde_json::json!({
            "exact": true,
            "filter": {
                "must": [
                    {
                        "key": "chunk_index",
                        "match": { "value": 0 }
                    }
                ]
            }
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let points_count = count["result"]["count"].as_u64().unwrap_or(0);
    let docs_embedded = docs_count["result"]["count"].as_u64().unwrap_or(0);
    let avg_chunks_per_doc = if docs_embedded > 0 {
        points_count as f64 / docs_embedded as f64
    } else {
        0.0
    };
    let indexed_vectors = info["result"]["indexed_vectors_count"]
        .as_u64()
        .or_else(|| info["result"]["vectors_count"].as_u64());
    let segments_count = info["result"]["segments_count"].as_u64();
    let payload_schema = info["result"]["payload_schema"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let payload_fields: Vec<String> = payload_schema.keys().cloned().collect();
    let payload_fields_count = payload_fields.len();

    let mut crawl_count: Option<i64> = None;
    let mut batch_count: Option<i64> = None;
    let mut extract_count: Option<i64> = None;
    let mut average_pages_per_second: Option<f64> = None;
    let mut average_crawl_duration_seconds: Option<f64> = None;
    let mut average_embedding_duration_seconds: Option<f64> = None;
    let mut average_overall_crawl_duration_seconds: Option<f64> = None;
    let mut longest_crawl: Option<serde_json::Value> = None;
    let mut most_chunks: Option<serde_json::Value> = None;
    let mut total_chunks: Option<i64> = None;
    let mut total_docs: Option<i64> = None;
    let mut base_urls_count: Option<i64> = None;

    let mut scrape_count: Option<i64> = None;
    let mut query_count: Option<i64> = None;
    let mut ask_count: Option<i64> = None;
    let mut retrieve_count: Option<i64> = None;
    let mut map_count: Option<i64> = None;
    let mut search_count: Option<i64> = None;

    if let Some(pool) = pg_pool_for_stats(cfg).await {
        if table_exists(&pool, "axon_crawl_jobs")
            .await
            .unwrap_or(false)
        {
            crawl_count = count_table_rows(&pool, "axon_crawl_jobs").await.ok();
            base_urls_count =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(DISTINCT url) FROM axon_crawl_jobs")
                    .fetch_one(&pool)
                    .await
                    .ok();
            average_pages_per_second = sqlx::query_scalar::<_, Option<f64>>(
                r#"
                SELECT AVG(
                    COALESCE((result_json->>'pages_discovered')::double precision, 0.0)
                    / GREATEST(EXTRACT(EPOCH FROM (finished_at - started_at))::double precision, 0.001::double precision)
                )
                FROM axon_crawl_jobs
                WHERE status='completed' AND started_at IS NOT NULL AND finished_at IS NOT NULL
                "#,
            )
            .fetch_one(&pool)
            .await
            .ok()
            .flatten();
            average_crawl_duration_seconds = sqlx::query_scalar::<_, Option<f64>>(
                "SELECT AVG(EXTRACT(EPOCH FROM (finished_at - started_at))::double precision) FROM axon_crawl_jobs WHERE status='completed' AND started_at IS NOT NULL AND finished_at IS NOT NULL",
            )
            .fetch_one(&pool)
            .await
            .ok()
            .flatten();
            if let Ok(Some(row)) = sqlx::query(
                r#"
                SELECT id::text AS id, url, EXTRACT(EPOCH FROM (finished_at - started_at))::double precision AS seconds
                FROM axon_crawl_jobs
                WHERE status='completed' AND started_at IS NOT NULL AND finished_at IS NOT NULL
                ORDER BY (finished_at - started_at) DESC
                LIMIT 1
                "#,
            )
            .fetch_optional(&pool)
            .await
            {
                let id: String = row.get("id");
                let url: String = row.get("url");
                let seconds: f64 = row.get("seconds");
                longest_crawl = Some(serde_json::json!({
                    "id": id,
                    "url": url,
                    "seconds": seconds
                }));
            }
            average_overall_crawl_duration_seconds = sqlx::query_scalar::<_, Option<f64>>(
                r#"
                SELECT AVG(
                    EXTRACT(EPOCH FROM (
                        COALESCE(e.finished_at, c.finished_at) - c.started_at
                    ))::double precision
                )
                FROM axon_crawl_jobs c
                LEFT JOIN LATERAL (
                    SELECT finished_at
                    FROM axon_embed_jobs e
                    WHERE e.status='completed'
                      AND e.input_text LIKE ('%' || c.id::text || '/markdown')
                    ORDER BY finished_at DESC
                    LIMIT 1
                ) e ON TRUE
                WHERE c.status='completed' AND c.started_at IS NOT NULL AND c.finished_at IS NOT NULL
                "#,
            )
            .fetch_one(&pool)
            .await
            .ok()
            .flatten();
        }
        if table_exists(&pool, "axon_batch_jobs")
            .await
            .unwrap_or(false)
        {
            batch_count = count_table_rows(&pool, "axon_batch_jobs").await.ok();
        }
        if table_exists(&pool, "axon_extract_jobs")
            .await
            .unwrap_or(false)
        {
            extract_count = count_table_rows(&pool, "axon_extract_jobs").await.ok();
        }
        if table_exists(&pool, "axon_embed_jobs")
            .await
            .unwrap_or(false)
        {
            average_embedding_duration_seconds = sqlx::query_scalar::<_, Option<f64>>(
                "SELECT AVG(EXTRACT(EPOCH FROM (finished_at - started_at))::double precision) FROM axon_embed_jobs WHERE status='completed' AND started_at IS NOT NULL AND finished_at IS NOT NULL",
            )
            .fetch_one(&pool)
            .await
            .ok()
            .flatten();
            total_chunks = sqlx::query_scalar::<_, Option<i64>>(
                "SELECT SUM(COALESCE((result_json->>'chunks_embedded')::bigint, 0)) FROM axon_embed_jobs WHERE status='completed'",
            )
            .fetch_one(&pool)
            .await
            .ok()
            .flatten();
            total_docs = sqlx::query_scalar::<_, Option<i64>>(
                "SELECT SUM(COALESCE((result_json->>'docs_embedded')::bigint, 0)) FROM axon_embed_jobs WHERE status='completed'",
            )
            .fetch_one(&pool)
            .await
            .ok()
            .flatten();
            if let Ok(Some(row)) = sqlx::query(
                r#"
                SELECT id::text AS id,
                       COALESCE((result_json->>'chunks_embedded')::bigint, 0) AS chunks
                FROM axon_embed_jobs
                WHERE status='completed'
                ORDER BY COALESCE((result_json->>'chunks_embedded')::bigint, 0) DESC
                LIMIT 1
                "#,
            )
            .fetch_optional(&pool)
            .await
            {
                let id: String = row.get("id");
                let chunks: i64 = row.get("chunks");
                most_chunks = Some(serde_json::json!({
                    "embed_job_id": id,
                    "chunks": chunks
                }));
            }
        }
        if table_exists(&pool, "axon_command_runs")
            .await
            .unwrap_or(false)
        {
            scrape_count = command_count(&pool, "scrape").await.ok();
            query_count = command_count(&pool, "query").await.ok();
            ask_count = command_count(&pool, "ask").await.ok();
            retrieve_count = command_count(&pool, "retrieve").await.ok();
            map_count = command_count(&pool, "map").await.ok();
            search_count = command_count(&pool, "search").await.ok();
        }
    }

    let stats = serde_json::json!({
        "collection": cfg.collection,
        "status": info["result"]["status"],
        "vectors_count": info["result"]["vectors_count"],
        "indexed_vectors_count": indexed_vectors,
        "points_count": points_count,
        "dimension": info["result"]["config"]["params"]["vectors"]["size"],
        "distance": info["result"]["config"]["params"]["vectors"]["distance"],
        "segments_count": segments_count,
        "docs_embedded_estimate": docs_embedded,
        "avg_chunks_per_doc": avg_chunks_per_doc,
        "payload_fields_count": payload_fields_count,
        "payload_fields": payload_fields,
        "avg_pages_crawled_per_second": average_pages_per_second,
        "avg_crawl_duration_seconds": average_crawl_duration_seconds,
        "avg_embedding_duration_seconds": average_embedding_duration_seconds,
        "avg_overall_crawl_duration_seconds": average_overall_crawl_duration_seconds,
        "longest_crawl": longest_crawl,
        "most_chunks": most_chunks,
        "total_chunks": total_chunks,
        "total_docs": total_docs,
        "base_urls_count": base_urls_count,
        "counts": {
            "crawls": crawl_count,
            "scrapes": scrape_count,
            "extracts": extract_count,
            "batches": batch_count,
            "queries": query_count,
            "asks": ask_count,
            "retrieves": retrieve_count,
            "maps": map_count,
            "searches": search_count
        },
        "usage_counts_note": "scrape/query/ask/retrieve/map/search are tracked in axon_command_runs from this release onward"
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
        println!(
            "  {} {}",
            muted("Indexed Vectors:"),
            stats["indexed_vectors_count"]
        );
        println!("  {} {}", muted("Points:"), stats["points_count"]);
        println!(
            "  {} {}",
            muted("Docs (est):"),
            stats["docs_embedded_estimate"]
        );
        println!(
            "  {} {:.2}",
            muted("Avg Chunks/Doc:"),
            stats["avg_chunks_per_doc"].as_f64().unwrap_or(0.0)
        );
        println!("  {} {}", muted("Dimension:"), stats["dimension"]);
        println!("  {} {}", muted("Distance:"), stats["distance"]);
        println!("  {} {}", muted("Segments:"), stats["segments_count"]);
        println!(
            "  {} {}",
            muted("Payload Fields:"),
            stats["payload_fields_count"]
        );
        if let Some(fields) = stats["payload_fields"].as_array() {
            let rendered = fields
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            if !rendered.is_empty() {
                println!("  {} {}", muted("Field Names:"), rendered);
            }
        }
        println!();
        println!("{}", primary("Pipeline Stats"));
        let avg_pages = stats["avg_pages_crawled_per_second"]
            .as_f64()
            .map(|v| format!("{v:.2}"))
            .unwrap_or_else(|| "n/a".to_string());
        let avg_crawl = stats["avg_crawl_duration_seconds"]
            .as_f64()
            .map(|v| format!("{v:.2}s"))
            .unwrap_or_else(|| "n/a".to_string());
        let avg_embed = stats["avg_embedding_duration_seconds"]
            .as_f64()
            .map(|v| format!("{v:.2}s"))
            .unwrap_or_else(|| "n/a".to_string());
        let avg_overall = stats["avg_overall_crawl_duration_seconds"]
            .as_f64()
            .map(|v| format!("{v:.2}s"))
            .unwrap_or_else(|| "n/a".to_string());
        println!("  {} {}", muted("Avg Pages/sec:"), avg_pages);
        println!("  {} {}", muted("Avg Crawl Duration:"), avg_crawl);
        println!("  {} {}", muted("Avg Embedding Duration:"), avg_embed);
        println!("  {} {}", muted("Avg Overall Crawl:"), avg_overall);
        println!("  {} {}", muted("Total Chunks:"), stats["total_chunks"]);
        println!("  {} {}", muted("Total Docs:"), stats["total_docs"]);
        println!("  {} {}", muted("Base URLs:"), stats["base_urls_count"]);
        if let Some(longest) = stats["longest_crawl"].as_object() {
            println!(
                "  {} {} ({:.2}s)",
                muted("Longest Crawl:"),
                longest.get("id").and_then(|v| v.as_str()).unwrap_or("n/a"),
                longest
                    .get("seconds")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0)
            );
        }
        if let Some(most) = stats["most_chunks"].as_object() {
            println!(
                "  {} {} ({})",
                muted("Most Chunks:"),
                most.get("embed_job_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("n/a"),
                most.get("chunks").and_then(|v| v.as_i64()).unwrap_or(0)
            );
        }
        println!();
        println!("{}", primary("Command Counts"));
        println!("  {} {}", muted("Crawls:"), stats["counts"]["crawls"]);
        println!("  {} {}", muted("Scrapes:"), stats["counts"]["scrapes"]);
        println!("  {} {}", muted("Extracts:"), stats["counts"]["extracts"]);
        println!("  {} {}", muted("Batches:"), stats["counts"]["batches"]);
        println!("  {} {}", muted("Queries:"), stats["counts"]["queries"]);
        println!("  {} {}", muted("Asks:"), stats["counts"]["asks"]);
        println!("  {} {}", muted("Retrieves:"), stats["counts"]["retrieves"]);
        println!("  {} {}", muted("Maps:"), stats["counts"]["maps"]);
        println!("  {} {}", muted("Searches:"), stats["counts"]["searches"]);
        println!(
            "  {}",
            muted(stats["usage_counts_note"].as_str().unwrap_or(""))
        );
    }
    Ok(())
}

pub async fn run_ask_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let max_context_chars =
        env_usize_clamped("AXON_ASK_MAX_CONTEXT_CHARS", 120_000, 20_000, 400_000);
    let ask_started = std::time::Instant::now();

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

    let retrieval_started = std::time::Instant::now();
    let mut ask_vectors = tei_embed(cfg, std::slice::from_ref(&query)).await?;
    if ask_vectors.is_empty() {
        return Err("TEI returned no vector for ask query".into());
    }
    let vecq = ask_vectors.remove(0);
    let candidate_pool_limit = env_usize_clamped("AXON_ASK_CANDIDATE_LIMIT", 64, 8, 200);
    let chunk_limit = env_usize_clamped("AXON_ASK_CHUNK_LIMIT", 10, 3, 40);
    let full_docs_limit = env_usize_clamped("AXON_ASK_FULL_DOCS", 4, 1, 20);
    let backfill_limit = env_usize_clamped("AXON_ASK_BACKFILL_CHUNKS", 3, 0, 20);
    let doc_fetch_concurrency = env_usize_clamped("AXON_ASK_DOC_FETCH_CONCURRENCY", 4, 1, 16);
    let doc_chunk_limit = env_usize_clamped("AXON_ASK_DOC_CHUNK_LIMIT", 192, 8, 2000);
    let min_relevance_score = env_f64_clamped("AXON_ASK_MIN_RELEVANCE_SCORE", 0.0, -1.0, 2.0);

    let hits = qdrant_search(cfg, &vecq, candidate_pool_limit).await?;
    let mut candidates = Vec::new();
    for hit in hits {
        let score = hit.score;
        let payload = &hit.payload;
        let url = payload_url_typed(payload).to_string();
        let chunk_text = payload_text_typed(payload).to_string();
        if url.is_empty() || chunk_text.len() < 40 {
            continue;
        }
        candidates.push(AskCandidate {
            score,
            url: url.clone(),
            chunk_text: chunk_text.clone(),
            url_tokens: tokenize_path_set(&url),
            chunk_tokens: tokenize_text_set(&chunk_text),
            rerank_score: score,
        });
    }
    if candidates.is_empty() {
        return Err("No relevant documents found for ask query".into());
    }

    let reranked = rerank_ask_candidates(&candidates, &query)
        .into_iter()
        .filter(|c| c.rerank_score >= min_relevance_score)
        .collect::<Vec<_>>();
    if reranked.is_empty() {
        return Err(format!(
            "No candidates met relevance threshold {:.3}; lower AXON_ASK_MIN_RELEVANCE_SCORE",
            min_relevance_score
        )
        .into());
    }
    let top_chunks = select_diverse_candidates(&reranked, chunk_limit, 2);
    let top_full_docs = select_diverse_candidates(&reranked, full_docs_limit, 1);
    let retrieval_elapsed_ms = retrieval_started.elapsed().as_millis();

    let context_started = std::time::Instant::now();
    let mut context_entries: Vec<String> = Vec::new();
    let mut context_char_count = 0usize;
    let separator = "\n\n---\n\n";
    let mut source_idx = 1usize;
    let mut top_chunks_selected = 0usize;
    for chunk in &top_chunks {
        let entry = format!(
            "## Top Chunk [S{}]: {}\n\n{}",
            source_idx, chunk.url, chunk.chunk_text
        );
        if !push_context_entry(
            &mut context_entries,
            &mut context_char_count,
            entry,
            separator,
            max_context_chars,
        ) {
            break;
        }
        top_chunks_selected += 1;
        source_idx += 1;
    }

    let mut fetched_docs = Vec::new();
    if context_char_count < max_context_chars {
        let mut fetch_stream = stream::iter(top_full_docs.iter().enumerate().map(
            |(idx, doc)| async move {
                let url = doc.url.clone();
                let points = qdrant_retrieve_by_url(cfg, &url, Some(doc_chunk_limit)).await;
                (idx, url, points)
            },
        ))
        .buffer_unordered(doc_fetch_concurrency);
        while let Some((idx, url, points)) = fetch_stream.next().await {
            fetched_docs.push((idx, url, points?));
        }
    }
    fetched_docs.sort_by_key(|(idx, _, _)| *idx);

    let mut inserted_full_doc_urls: HashSet<String> = HashSet::new();
    let mut full_docs_selected = 0usize;
    for (_idx, url, points) in fetched_docs {
        let text = render_full_doc_from_points(points);
        if text.is_empty() {
            continue;
        }
        let entry = format!("## Source Document [S{}]: {}\n\n{}", source_idx, url, text);
        if !push_context_entry(
            &mut context_entries,
            &mut context_char_count,
            entry,
            separator,
            max_context_chars,
        ) {
            break;
        }
        inserted_full_doc_urls.insert(url);
        full_docs_selected += 1;
        source_idx += 1;
    }

    let supplemental = select_diverse_candidates(
        &reranked
            .iter()
            .filter(|c| !inserted_full_doc_urls.contains(&c.url))
            .cloned()
            .collect::<Vec<_>>(),
        backfill_limit,
        1,
    );

    let mut supplemental_count = 0usize;
    for chunk in &supplemental {
        let entry = format!(
            "## Supplemental Chunk [S{}]: {}\n\n{}",
            source_idx, chunk.url, chunk.chunk_text
        );
        if !push_context_entry(
            &mut context_entries,
            &mut context_char_count,
            entry,
            separator,
            max_context_chars,
        ) {
            break;
        }
        supplemental_count += 1;
        source_idx += 1;
    }

    if context_entries.is_empty() {
        return Err("Failed to retrieve any context sources for ask".into());
    }

    let context = format!("Sources:\n{}", context_entries.join(separator));
    let context_elapsed_ms = context_started.elapsed().as_millis();

    if cfg.ask_diagnostics {
        let mut diagnostic_sources: Vec<String> = Vec::new();
        diagnostic_sources.extend(
            top_full_docs
                .iter()
                .map(|c| format!("full-doc score={:.3} url={}", c.score, c.url)),
        );
        diagnostic_sources.extend(
            supplemental
                .iter()
                .take(supplemental_count)
                .map(|c| format!("chunk score={:.3} url={}", c.score, c.url)),
        );
        if cfg.json_output {
            eprintln!(
                "{}",
                serde_json::json!({
                    "ask_diagnostics": {
                        "candidate_pool": candidates.len(),
                        "reranked_pool": reranked.len(),
                        "chunks_selected": top_chunks_selected,
                        "full_docs_selected": full_docs_selected,
                        "supplemental_selected": supplemental_count,
                        "context_chars": context.len(),
                        "min_relevance_score": min_relevance_score,
                        "doc_fetch_concurrency": doc_fetch_concurrency,
                    "sources": diagnostic_sources,
                    }
                })
            );
        } else {
            eprintln!("{}", primary("Ask Diagnostics"));
            eprintln!(
                "  {} candidates={} reranked={} chunks={} full_docs={} supplemental={} context_chars={}",
                muted("Retrieval:"),
                candidates.len(),
                reranked.len(),
                top_chunks_selected,
                full_docs_selected,
                supplemental_count,
                context.len()
            );
            for source in diagnostic_sources {
                eprintln!("  • {source}");
            }
            eprintln!();
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
                {"role": "system", "content": "Answer only from the provided sources. Cite supporting sources inline using [S#] labels. If the sources are incomplete, say so explicitly."},
                {"role": "user", "content": format!("{}\n\n{}", query, context)}
            ],
            "temperature": 0.1
        }));

    if !cfg.openai_api_key.is_empty() {
        req = req.bearer_auth(&cfg.openai_api_key);
    }

    let llm_started = std::time::Instant::now();
    let response = req.send().await?.error_for_status()?;
    let json: serde_json::Value = response.json().await?;
    let answer = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(no answer)");
    let llm_elapsed_ms = llm_started.elapsed().as_millis();
    let total_elapsed_ms = ask_started.elapsed().as_millis();
    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "query": query,
                "answer": answer,
                "diagnostics": if cfg.ask_diagnostics {
                    serde_json::json!({
                        "candidate_pool": candidates.len(),
                        "reranked_pool": reranked.len(),
                        "chunks_selected": top_chunks_selected,
                        "full_docs_selected": full_docs_selected,
                        "supplemental_selected": supplemental_count,
                        "context_chars": context.len(),
                        "min_relevance_score": min_relevance_score,
                        "doc_fetch_concurrency": doc_fetch_concurrency,
                    })
                } else {
                    serde_json::Value::Null
                },
                "timing_ms": {
                    "retrieval": retrieval_elapsed_ms,
                    "context_build": context_elapsed_ms,
                    "llm": llm_elapsed_ms,
                    "total": total_elapsed_ms,
                }
            }))?
        );
    } else {
        println!("{}", primary("Conversation"));
        println!("  {} {}", primary("You:"), query);
        println!("  {} {}", primary("Assistant:"), answer);
        if cfg.ask_diagnostics {
            println!(
                "  {} candidates={} reranked={} chunks={} full_docs={} supplemental={} context_chars={}",
                muted("Diagnostics:"),
                candidates.len(),
                reranked.len(),
                top_chunks_selected,
                full_docs_selected,
                supplemental_count,
                context.len()
            );
        }
        println!(
            "  {} retrieval={}ms | context={}ms | llm={}ms | total={}ms",
            muted("Timing:"),
            retrieval_elapsed_ms,
            context_elapsed_ms,
            llm_elapsed_ms,
            total_elapsed_ms
        );
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

    #[test]
    fn test_rerank_candidates_boosts_url_token_match() {
        let candidates = vec![
            AskCandidate {
                score: 0.40,
                url: "https://example.com/blog/overview".to_string(),
                chunk_text: "general notes".to_string(),
                url_tokens: tokenize_path_set("https://example.com/blog/overview"),
                chunk_tokens: tokenize_text_set("general notes"),
                rerank_score: 0.40,
            },
            AskCandidate {
                score: 0.39,
                url: "https://example.com/docs/install".to_string(),
                chunk_text: "installation steps and setup".to_string(),
                url_tokens: tokenize_path_set("https://example.com/docs/install"),
                chunk_tokens: tokenize_text_set("installation steps and setup"),
                rerank_score: 0.39,
            },
        ];
        let reranked = rerank_ask_candidates(&candidates, "install setup");
        assert_eq!(reranked[0].url, "https://example.com/docs/install");
        assert!(reranked[0].rerank_score > reranked[1].rerank_score);
    }

    #[test]
    fn test_select_diverse_candidates_respects_max_per_url() {
        let make_candidate = |score: f64, url: &str| AskCandidate {
            score,
            url: url.to_string(),
            chunk_text: "chunk".to_string(),
            url_tokens: tokenize_path_set(url),
            chunk_tokens: tokenize_text_set("chunk"),
            rerank_score: score,
        };
        let candidates = vec![
            make_candidate(0.9, "https://a.dev/docs"),
            make_candidate(0.8, "https://a.dev/docs"),
            make_candidate(0.7, "https://b.dev/docs"),
            make_candidate(0.6, "https://c.dev/docs"),
        ];
        let selected = select_diverse_candidates(&candidates, 3, 1);
        let urls: HashSet<String> = selected.into_iter().map(|c| c.url).collect();
        assert_eq!(urls.len(), 3);
    }

    #[test]
    fn test_render_full_doc_orders_by_chunk_index_and_trims() {
        let points = vec![
            QdrantPoint {
                payload: QdrantPayload {
                    chunk_index: Some(1),
                    chunk_text: "second".to_string(),
                    ..QdrantPayload::default()
                },
            },
            QdrantPoint {
                payload: QdrantPayload {
                    chunk_index: Some(0),
                    chunk_text: "first".to_string(),
                    ..QdrantPayload::default()
                },
            },
            QdrantPoint {
                payload: QdrantPayload {
                    chunk_index: Some(2),
                    chunk_text: String::new(),
                    ..QdrantPayload::default()
                },
            },
        ];
        let text = render_full_doc_from_points(points);
        assert_eq!(text, "first\nsecond");
    }

    #[test]
    fn test_query_snippet_truncates_and_flattens_newlines() {
        let payload = QdrantPayload {
            chunk_text: format!("line1\n{}", "x".repeat(200)),
            ..QdrantPayload::default()
        };
        let snippet = query_snippet(&payload);
        assert!(!snippet.contains('\n'));
        assert!(snippet.chars().count() <= 140);
    }

    #[test]
    fn test_meaningful_snippet_prefers_query_relevant_sentence() {
        let chunk = "Installation Guide\nWelcome.\nTo configure auth tokens, set API_KEY and restart service. Additional unrelated footer.";
        let snippet = meaningful_snippet(chunk, "auth token api");
        assert!(snippet.to_ascii_lowercase().contains("auth"));
    }

    #[test]
    fn test_canonical_url_key_drops_query_fragment() {
        let key = canonical_url_key("https://example.dev/docs/install/?a=1#intro");
        assert_eq!(key, "https://example.dev/docs/install");
    }

    #[test]
    fn test_url_lookup_candidates_include_normalized_forms() {
        let variants = url_lookup_candidates("example.com/docs");
        assert!(variants.iter().any(|v| v == "https://example.com/docs"));
        assert!(variants.iter().any(|v| v == "https://example.com/docs/"));
    }
}
