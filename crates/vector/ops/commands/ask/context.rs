use crate::crates::core::config::Config;
use crate::crates::core::logging::log_warn;
use crate::crates::vector::ops::source_display::display_source;
use crate::crates::vector::ops::{qdrant, ranking, tei};
use anyhow::{Result, anyhow};
use futures_util::stream::{self, StreamExt};
use std::collections::HashSet;
use std::sync::Arc;

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

pub(crate) struct AskContext {
    pub context: String,
    pub candidate_count: usize,
    pub reranked_count: usize,
    pub chunks_selected: usize,
    pub full_docs_selected: usize,
    pub supplemental_count: usize,
    pub retrieval_elapsed_ms: u128,
    pub context_elapsed_ms: u128,
    /// Pre-built source descriptions for diagnostics display.
    pub diagnostic_sources: Vec<String>,
}

struct AskRetrieval {
    candidates: Vec<ranking::AskCandidate>,
    reranked: Vec<ranking::AskCandidate>,
    top_chunk_indices: Vec<usize>,
    top_full_doc_indices: Vec<usize>,
    retrieval_elapsed_ms: u128,
}

struct BuiltAskContext {
    context: String,
    chunks_selected: usize,
    full_docs_selected: usize,
    supplemental_count: usize,
    context_elapsed_ms: u128,
    diagnostic_sources: Vec<String>,
}

pub(crate) async fn build_ask_context(cfg: &Config, query: &str) -> Result<AskContext> {
    let retrieval = retrieve_ask_candidates(cfg, query).await?;
    let built = build_context_from_candidates(
        cfg,
        &retrieval.reranked,
        &retrieval.top_chunk_indices,
        &retrieval.top_full_doc_indices,
    )
    .await?;

    Ok(AskContext {
        context: built.context,
        candidate_count: retrieval.candidates.len(),
        reranked_count: retrieval.reranked.len(),
        chunks_selected: built.chunks_selected,
        full_docs_selected: built.full_docs_selected,
        supplemental_count: built.supplemental_count,
        retrieval_elapsed_ms: retrieval.retrieval_elapsed_ms,
        context_elapsed_ms: built.context_elapsed_ms,
        diagnostic_sources: built.diagnostic_sources,
    })
}

async fn retrieve_ask_candidates(cfg: &Config, query: &str) -> Result<AskRetrieval> {
    let retrieval_started = std::time::Instant::now();
    let mut ask_vectors = tei::tei_embed(cfg, &[query.to_string()])
        .await
        .map_err(|e| anyhow!(e.to_string()))?;
    if ask_vectors.is_empty() {
        return Err(anyhow!("TEI returned no vector for ask query"));
    }
    let vecq = ask_vectors.remove(0);
    let query_tokens = ranking::tokenize_query(query);
    let hits = qdrant::qdrant_search(cfg, &vecq, cfg.ask_candidate_limit)
        .await
        .map_err(|e| anyhow!(e.to_string()))?;
    let mut candidates = Vec::new();
    for hit in hits {
        let url = qdrant::payload_url_typed(&hit.payload).to_string();
        let chunk_text = qdrant::payload_text_typed(&hit.payload).to_string();
        if url.is_empty() || chunk_text.len() < 40 {
            continue;
        }
        let path = ranking::extract_path_from_url(&url);
        candidates.push(ranking::AskCandidate {
            score: hit.score,
            url: url.clone(),
            path: path.clone(),
            chunk_text: chunk_text.clone(),
            url_tokens: ranking::tokenize_path_set(&path),
            chunk_tokens: ranking::tokenize_text_set(&chunk_text),
            rerank_score: hit.score,
        });
    }
    if candidates.is_empty() {
        return Err(anyhow!("No relevant documents found for ask query"));
    }
    let reranked = ranking::rerank_ask_candidates(&candidates, &query_tokens)
        .into_iter()
        .filter(|c| c.rerank_score >= cfg.ask_min_relevance_score)
        .collect::<Vec<_>>();
    if reranked.is_empty() {
        return Err(anyhow!(
            "No candidates met relevance threshold {:.3}; lower AXON_ASK_MIN_RELEVANCE_SCORE",
            cfg.ask_min_relevance_score
        ));
    }
    Ok(AskRetrieval {
        top_chunk_indices: ranking::select_diverse_candidates(&reranked, cfg.ask_chunk_limit, 2),
        top_full_doc_indices: ranking::select_diverse_candidates(&reranked, cfg.ask_full_docs, 1),
        candidates,
        reranked,
        retrieval_elapsed_ms: retrieval_started.elapsed().as_millis(),
    })
}

async fn build_context_from_candidates(
    cfg: &Config,
    reranked: &[ranking::AskCandidate],
    top_chunk_indices: &[usize],
    top_full_doc_indices: &[usize],
) -> Result<BuiltAskContext> {
    let max_context_chars = cfg.ask_max_context_chars;
    let backfill_limit = cfg.ask_backfill_chunks;
    let doc_fetch_concurrency = cfg.ask_doc_fetch_concurrency;
    let doc_chunk_limit = cfg.ask_doc_chunk_limit;
    let context_started = std::time::Instant::now();
    let mut context_entries: Vec<String> = Vec::new();
    let mut context_char_count = 0usize;
    let separator = "\n\n---\n\n";
    let mut source_idx = 1usize;
    let top_chunks_selected = append_top_chunks_to_context(
        reranked,
        top_chunk_indices,
        &mut context_entries,
        &mut context_char_count,
        &mut source_idx,
        separator,
        max_context_chars,
    );

    let mut inserted_full_doc_urls: HashSet<String> = HashSet::new();
    let fetched_docs = fetch_full_docs(
        cfg,
        reranked,
        top_full_doc_indices,
        context_char_count,
        max_context_chars,
        doc_chunk_limit,
        doc_fetch_concurrency,
    )
    .await?;
    let (full_docs_selected, next_source_idx) = append_full_docs_to_context(
        &mut context_entries,
        &mut context_char_count,
        &mut inserted_full_doc_urls,
        source_idx,
        separator,
        max_context_chars,
        fetched_docs,
    );
    source_idx = next_source_idx;

    let supplemental_candidate_indices =
        collect_supplemental_candidate_indices(reranked, &inserted_full_doc_urls);
    let supplemental = ranking::select_diverse_candidates_from_indices(
        reranked,
        &supplemental_candidate_indices,
        backfill_limit,
        1,
    );

    let supplemental_count = append_supplemental_chunks(
        reranked,
        &supplemental,
        &mut context_entries,
        &mut context_char_count,
        &mut source_idx,
        separator,
        max_context_chars,
    );

    if context_entries.is_empty() {
        return Err(anyhow!("Failed to retrieve any context sources for ask"));
    }

    let context = format!("Sources:\n{}", context_entries.join(separator));
    let context_elapsed_ms = context_started.elapsed().as_millis();

    let diagnostic_sources = build_diagnostic_sources(
        reranked,
        top_chunk_indices,
        top_chunks_selected,
        top_full_doc_indices,
        &supplemental,
        supplemental_count,
    );

    Ok(BuiltAskContext {
        context,
        chunks_selected: top_chunks_selected,
        full_docs_selected,
        supplemental_count,
        context_elapsed_ms,
        diagnostic_sources,
    })
}

fn append_top_chunks_to_context(
    reranked: &[ranking::AskCandidate],
    top_chunk_indices: &[usize],
    context_entries: &mut Vec<String>,
    context_char_count: &mut usize,
    source_idx: &mut usize,
    separator: &str,
    max_context_chars: usize,
) -> usize {
    let mut top_chunks_selected = 0usize;
    for &chunk_idx in top_chunk_indices {
        let chunk = &reranked[chunk_idx];
        let source = display_source(&chunk.url);
        let entry = format!(
            "## Top Chunk [S{}]: {}\n\n{}",
            *source_idx, source, chunk.chunk_text
        );
        if !push_context_entry(
            context_entries,
            context_char_count,
            entry,
            separator,
            max_context_chars,
        ) {
            break;
        }
        top_chunks_selected += 1;
        *source_idx += 1;
    }
    top_chunks_selected
}

fn collect_supplemental_candidate_indices(
    reranked: &[ranking::AskCandidate],
    inserted_full_doc_urls: &HashSet<String>,
) -> Vec<usize> {
    reranked
        .iter()
        .enumerate()
        .filter(|(_, candidate)| !inserted_full_doc_urls.contains(&candidate.url))
        .map(|(idx, _)| idx)
        .collect::<Vec<_>>()
}

async fn fetch_full_docs(
    cfg: &Config,
    reranked: &[ranking::AskCandidate],
    top_full_doc_indices: &[usize],
    context_char_count: usize,
    max_context_chars: usize,
    doc_chunk_limit: usize,
    doc_fetch_concurrency: usize,
) -> Result<Vec<(usize, String, Vec<qdrant::QdrantPoint>)>> {
    let mut fetched_docs = Vec::new();
    if context_char_count >= max_context_chars {
        return Ok(fetched_docs);
    }
    let cfg_arc = Arc::new(cfg.clone());
    let mut fetch_stream = stream::iter(top_full_doc_indices.iter().enumerate().map(
        |(order, &doc_idx)| {
            let cfg_for_task = Arc::clone(&cfg_arc);
            let url = reranked[doc_idx].url.clone();
            async move {
                let points =
                    qdrant::qdrant_retrieve_by_url(&cfg_for_task, &url, Some(doc_chunk_limit))
                        .await;
                (order, url, points)
            }
        },
    ))
    .buffer_unordered(doc_fetch_concurrency);
    while let Some((order, url, points)) = fetch_stream.next().await {
        match points {
            Ok(points) => fetched_docs.push((order, url, points)),
            Err(err) => {
                log_warn(&format!(
                    "ask: failed to retrieve full document for {url}; continuing with remaining context: {err}"
                ));
            }
        }
    }
    fetched_docs.sort_by_key(|(order, _, _)| *order);
    Ok(fetched_docs)
}

fn append_full_docs_to_context(
    context_entries: &mut Vec<String>,
    context_char_count: &mut usize,
    inserted_full_doc_urls: &mut HashSet<String>,
    mut source_idx: usize,
    separator: &str,
    max_context_chars: usize,
    fetched_docs: Vec<(usize, String, Vec<qdrant::QdrantPoint>)>,
) -> (usize, usize) {
    let mut full_docs_selected = 0usize;
    for (_idx, url, points) in fetched_docs {
        let text = qdrant::render_full_doc_from_points(points);
        if text.is_empty() {
            continue;
        }
        let source = display_source(&url);
        let entry = format!(
            "## Source Document [S{}]: {}\n\n{}",
            source_idx, source, text
        );
        if !push_context_entry(
            context_entries,
            context_char_count,
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
    (full_docs_selected, source_idx)
}

fn append_supplemental_chunks(
    reranked: &[ranking::AskCandidate],
    supplemental: &[usize],
    context_entries: &mut Vec<String>,
    context_char_count: &mut usize,
    source_idx: &mut usize,
    separator: &str,
    max_context_chars: usize,
) -> usize {
    let mut supplemental_count = 0usize;
    for &chunk_idx in supplemental {
        let chunk = &reranked[chunk_idx];
        let source = display_source(&chunk.url);
        let entry = format!(
            "## Supplemental Chunk [S{}]: {}\n\n{}",
            *source_idx, source, chunk.chunk_text
        );
        if !push_context_entry(
            context_entries,
            context_char_count,
            entry,
            separator,
            max_context_chars,
        ) {
            break;
        }
        supplemental_count += 1;
        *source_idx += 1;
    }
    supplemental_count
}

fn build_diagnostic_sources(
    reranked: &[ranking::AskCandidate],
    top_chunk_indices: &[usize],
    top_chunks_selected: usize,
    top_full_doc_indices: &[usize],
    supplemental: &[usize],
    supplemental_count: usize,
) -> Vec<String> {
    let mut diagnostic_sources: Vec<String> = Vec::new();
    diagnostic_sources.extend(
        top_chunk_indices
            .iter()
            .take(top_chunks_selected)
            .map(|&idx| &reranked[idx])
            .map(|c| format!("chunk score={:.3} url={}", c.score, display_source(&c.url))),
    );
    diagnostic_sources.extend(
        top_full_doc_indices
            .iter()
            .map(|&idx| &reranked[idx])
            .map(|c| {
                format!(
                    "full-doc score={:.3} url={}",
                    c.score,
                    display_source(&c.url)
                )
            }),
    );
    diagnostic_sources.extend(
        supplemental
            .iter()
            .map(|&idx| &reranked[idx])
            .take(supplemental_count)
            .map(|c| format!("chunk score={:.3} url={}", c.score, display_source(&c.url))),
    );
    diagnostic_sources
}
