use crate::crates::core::config::Config;
use crate::crates::core::logging::log_warn;
use crate::crates::vector::ops::source_display::display_source;
use crate::crates::vector::ops::{qdrant, ranking, tei};
use anyhow::{Result, anyhow};
use futures_util::stream::{self, StreamExt};
use spider::url::Url;
use std::collections::{HashMap, HashSet};
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

const SUPPLEMENTAL_CONTEXT_BUDGET_PCT: usize = 85;
const SUPPLEMENTAL_MIN_TOP_CHUNKS_FOR_COVERAGE: usize = 6;
const SUPPLEMENTAL_RELEVANCE_BONUS: f64 = 0.05;

fn should_inject_supplemental(
    context_char_count: usize,
    max_context_chars: usize,
    full_docs_selected: usize,
    top_chunks_selected: usize,
) -> bool {
    if max_context_chars == 0 {
        return false;
    }
    let within_budget =
        context_char_count * 100 < max_context_chars * SUPPLEMENTAL_CONTEXT_BUDGET_PCT;
    let coverage_needs_backfill =
        full_docs_selected == 0 || top_chunks_selected < SUPPLEMENTAL_MIN_TOP_CHUNKS_FOR_COVERAGE;
    within_budget && coverage_needs_backfill
}

fn query_requests_low_signal_sources(query_tokens: &[String], raw_query: &str) -> bool {
    if raw_query.to_ascii_lowercase().contains("docs/sessions") {
        return true;
    }
    query_tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "session" | "sessions" | "log" | "logs" | "history" | "histories"
        )
    })
}

fn is_low_signal_source_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let is_web_url = lower.starts_with("http://") || lower.starts_with("https://");
    // Session logs and cache files — local file path patterns only.
    // /logs/ and .log are NOT filtered for web URLs to avoid false-positives on
    // legitimately indexed pages like docs.example.com/logs/ or Datadog docs.
    lower.contains("/docs/sessions/")
        || lower.contains("docs/sessions/")
        || lower.contains("/.cache/")
        || lower.contains(".cache/")
        || (!is_web_url && lower.contains("/logs/"))
        || (!is_web_url && lower.ends_with(".log"))
}

fn url_matches_domain_list(url: &str, domains: &[String]) -> bool {
    if domains.is_empty() {
        return true;
    }
    let host = Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(|h| h.to_ascii_lowercase()));
    let Some(host) = host else {
        return false;
    };
    domains.iter().any(|domain| {
        let normalized = domain.trim().to_ascii_lowercase();
        !normalized.is_empty() && (host == normalized || host.ends_with(&format!(".{normalized}")))
    })
}

fn host_from_url(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(|h| h.to_ascii_lowercase()))
}

fn top_domains(candidates: &[ranking::AskCandidate], limit: usize) -> Vec<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for candidate in candidates {
        if let Some(host) = host_from_url(&candidate.url) {
            *counts.entry(host).or_insert(0) += 1;
        }
    }
    let mut entries = counts.into_iter().collect::<Vec<_>>();
    entries.sort_by(|(domain_a, count_a), (domain_b, count_b)| {
        count_b.cmp(count_a).then_with(|| domain_a.cmp(domain_b))
    });
    entries
        .into_iter()
        .take(limit)
        .map(|(domain, count)| format!("{domain}:{count}"))
        .collect()
}

fn authoritative_ratio(candidates: &[ranking::AskCandidate], domains: &[String]) -> f64 {
    if candidates.is_empty() || domains.is_empty() {
        return 0.0;
    }
    let authoritative = candidates
        .iter()
        .filter(|candidate| url_matches_domain_list(&candidate.url, domains))
        .count();
    authoritative as f64 / candidates.len() as f64
}

fn candidate_topical_overlap_count(
    candidate: &ranking::AskCandidate,
    query_tokens: &[String],
) -> usize {
    query_tokens
        .iter()
        .filter(|token| {
            candidate.url_tokens.contains(token.as_str())
                || candidate.chunk_tokens.contains(token.as_str())
        })
        .count()
}

fn candidate_has_topical_overlap(
    candidate: &ranking::AskCandidate,
    query_tokens: &[String],
) -> bool {
    if query_tokens.is_empty() {
        return true;
    }
    let overlap = candidate_topical_overlap_count(candidate, query_tokens);
    let coverage = overlap as f64 / query_tokens.len() as f64;
    match query_tokens.len() {
        0 => true,
        1 | 2 => overlap >= 1,
        3 | 4 => overlap >= 2 || coverage >= 0.5,
        _ => overlap >= 2 && coverage >= 0.34,
    }
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
    pub top_domains: Vec<String>,
    pub authoritative_ratio: f64,
    pub dropped_by_allowlist: usize,
}

struct AskRetrieval {
    candidates: Vec<ranking::AskCandidate>,
    reranked: Vec<ranking::AskCandidate>,
    top_chunk_indices: Vec<usize>,
    top_full_doc_indices: Vec<usize>,
    retrieval_elapsed_ms: u128,
    top_domains: Vec<String>,
    authoritative_ratio: f64,
    dropped_by_allowlist: usize,
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
        top_domains: retrieval.top_domains,
        authoritative_ratio: retrieval.authoritative_ratio,
        dropped_by_allowlist: retrieval.dropped_by_allowlist,
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
    let allow_low_signal = query_requests_low_signal_sources(&query_tokens, query);
    let hits = qdrant::qdrant_search(cfg, &vecq, cfg.ask_candidate_limit)
        .await
        .map_err(|e| anyhow!(e.to_string()))?;
    let mut candidates = Vec::new();
    let mut dropped_by_allowlist = 0usize;
    for hit in hits {
        let url = qdrant::payload_url_typed(&hit.payload).to_string();
        let chunk_text = qdrant::payload_text_typed(&hit.payload).to_string();
        if url.is_empty() || chunk_text.len() < 40 {
            continue;
        }
        if !allow_low_signal && is_low_signal_source_url(&url) {
            continue;
        }
        if !cfg.ask_authoritative_allowlist.is_empty()
            && !url_matches_domain_list(&url, &cfg.ask_authoritative_allowlist)
        {
            dropped_by_allowlist += 1;
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
    let reranked = ranking::rerank_ask_candidates(
        &candidates,
        &query_tokens,
        &cfg.ask_authoritative_domains,
        cfg.ask_authoritative_boost,
    )
    .into_iter()
    .filter(|candidate| {
        candidate.rerank_score >= cfg.ask_min_relevance_score
            && candidate_has_topical_overlap(candidate, &query_tokens)
    })
    .collect::<Vec<_>>();
    if reranked.is_empty() {
        return Err(anyhow!(
            "No candidates met relevance threshold {:.3}; lower AXON_ASK_MIN_RELEVANCE_SCORE",
            cfg.ask_min_relevance_score
        ));
    }
    Ok(AskRetrieval {
        top_chunk_indices: ranking::select_diverse_candidates(&reranked, cfg.ask_chunk_limit, 1),
        top_full_doc_indices: ranking::select_diverse_candidates(&reranked, cfg.ask_full_docs, 1),
        top_domains: top_domains(&reranked, 5),
        authoritative_ratio: authoritative_ratio(&reranked, &cfg.ask_authoritative_domains),
        dropped_by_allowlist,
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

    let mut supplemental: Vec<usize> = Vec::new();
    let mut supplemental_count = 0usize;
    if should_inject_supplemental(
        context_char_count,
        max_context_chars,
        full_docs_selected,
        top_chunks_selected,
    ) {
        let min_supplemental_score = cfg.ask_min_relevance_score + SUPPLEMENTAL_RELEVANCE_BONUS;
        let supplemental_candidate_indices = collect_supplemental_candidate_indices(
            reranked,
            &inserted_full_doc_urls,
            min_supplemental_score,
        );
        supplemental = ranking::select_diverse_candidates_from_indices(
            reranked,
            &supplemental_candidate_indices,
            backfill_limit,
            1,
        );

        supplemental_count = append_supplemental_chunks(
            reranked,
            &supplemental,
            &mut context_entries,
            &mut context_char_count,
            &mut source_idx,
            separator,
            max_context_chars,
        );
    }

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
    min_supplemental_score: f64,
) -> Vec<usize> {
    reranked
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            !inserted_full_doc_urls.contains(&candidate.url)
                && candidate.rerank_score >= min_supplemental_score
        })
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

#[cfg(test)]
mod tests {
    use super::{
        candidate_has_topical_overlap, collect_supplemental_candidate_indices,
        is_low_signal_source_url, query_requests_low_signal_sources, should_inject_supplemental,
        url_matches_domain_list,
    };
    use crate::crates::vector::ops::ranking::AskCandidate;
    use std::collections::HashSet;

    fn test_candidate(url: &str, rerank_score: f64) -> AskCandidate {
        AskCandidate {
            score: rerank_score,
            url: url.to_string(),
            path: url.to_string(),
            chunk_text: "chunk text for testing".to_string(),
            url_tokens: HashSet::new(),
            chunk_tokens: HashSet::new(),
            rerank_score,
        }
    }

    #[test]
    fn supplemental_injects_when_coverage_is_thin_and_budget_is_available() {
        let should = should_inject_supplemental(
            10_000, 100_000, 0, // no full docs selected yet
            4, // below coverage threshold
        );
        assert!(should);
    }

    #[test]
    fn supplemental_skips_when_context_budget_is_nearly_full() {
        let should = should_inject_supplemental(
            90_000,  // exceeds 85% budget gate
            100_000, //
            0, 2,
        );
        assert!(!should);
    }

    #[test]
    fn supplemental_skips_when_coverage_is_already_strong() {
        let should = should_inject_supplemental(
            50_000, // budget headroom remains
            100_000, 2, // full docs present
            6, // meets chunk coverage threshold
        );
        assert!(!should);
    }

    #[test]
    fn low_signal_source_filter_matches_sessions_and_cache() {
        assert!(is_low_signal_source_url(
            "docs/sessions/2026-02-26-context-injection-cleanup.md"
        ));
        assert!(is_low_signal_source_url(".cache/axon-rust/output/file.md"));
        // Local file paths with /logs/ or .log are filtered.
        assert!(is_low_signal_source_url("/home/user/app/logs/access.log"));
        assert!(is_low_signal_source_url("logs/debug.log"));
        // Web URLs with /logs/ in the path must NOT be filtered (e.g. Datadog docs).
        assert!(!is_low_signal_source_url(
            "https://docs.datadoghq.com/logs/explorer/"
        ));
        assert!(!is_low_signal_source_url(
            "https://docs.rs/spider/latest/spider/"
        ));
    }

    #[test]
    fn low_signal_sources_allowed_when_query_explicitly_requests_them() {
        let tokens = vec!["debug".to_string(), "session".to_string()];
        assert!(query_requests_low_signal_sources(
            &tokens,
            "debug this session"
        ));
        assert!(query_requests_low_signal_sources(
            &["debug".to_string()],
            "show docs/sessions files"
        ));
        assert!(!query_requests_low_signal_sources(
            &["debug".to_string(), "crawl".to_string()],
            "debug crawl failures"
        ));
    }

    #[test]
    fn supplemental_candidates_respect_score_threshold_and_full_doc_exclusions() {
        let candidates = vec![
            test_candidate("https://a.dev/docs/one", 0.70),
            test_candidate("https://a.dev/docs/two", 0.52),
            test_candidate("https://b.dev/docs/three", 0.61),
        ];
        let mut excluded = HashSet::new();
        excluded.insert("https://a.dev/docs/one".to_string());
        let selected = collect_supplemental_candidate_indices(&candidates, &excluded, 0.60);
        assert_eq!(selected, vec![2]);
    }

    #[test]
    fn topical_overlap_requires_multiple_query_tokens_for_longer_queries() {
        let candidate = test_candidate("https://example.com/docs/commands", 0.9);
        let tokens = vec![
            "create".to_string(),
            "claude".to_string(),
            "code".to_string(),
            "custom".to_string(),
            "slash".to_string(),
            "commands".to_string(),
        ];
        assert!(!candidate_has_topical_overlap(&candidate, &tokens));

        let strong_candidate = AskCandidate {
            score: 0.9,
            url: "https://docs.claude.com/en/docs/claude-code/slash-commands".to_string(),
            path: "/docs/claude-code/slash-commands".to_string(),
            chunk_text: "Create custom slash commands in Claude Code.".to_string(),
            url_tokens: ["claude", "code", "slash", "commands"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            chunk_tokens: ["create", "custom", "slash", "commands"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            rerank_score: 0.9,
        };
        assert!(candidate_has_topical_overlap(&strong_candidate, &tokens));
    }

    #[test]
    fn authoritative_allowlist_matches_exact_and_suffix_hosts() {
        let allow = vec!["docs.claude.com".to_string(), "openai.com".to_string()];
        assert!(url_matches_domain_list(
            "https://docs.claude.com/en/docs/claude-code/overview",
            &allow
        ));
        assert!(url_matches_domain_list(
            "https://platform.openai.com/docs/overview",
            &allow
        ));
        assert!(!url_matches_domain_list(
            "https://medium.com/some-post",
            &allow
        ));
    }
}
