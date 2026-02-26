use crate::crates::core::config::Config;
use crate::crates::core::http::http_client;
use crate::crates::core::logging::log_warn;
use crate::crates::core::ui::{muted, primary};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::error::Error;

mod context;
pub(crate) use context::{AskContext, build_ask_context};

use super::streaming::{ask_llm_non_streaming, ask_llm_streaming};

fn ask_query(cfg: &Config) -> Result<String, Box<dyn Error>> {
    super::resolve_query_text(cfg).ok_or_else(|| "ask requires query".into())
}

fn emit_ask_diagnostics(cfg: &Config, ctx: &AskContext) {
    if !cfg.ask_diagnostics {
        return;
    }
    if cfg.json_output {
        eprintln!(
            "{}",
            serde_json::json!({
                "ask_diagnostics": {
                    "candidate_pool": ctx.candidate_count,
                    "reranked_pool": ctx.reranked_count,
                    "chunks_selected": ctx.chunks_selected,
                    "full_docs_selected": ctx.full_docs_selected,
                    "supplemental_selected": ctx.supplemental_count,
                    "context_chars": ctx.context.len(),
                    "min_relevance_score": cfg.ask_min_relevance_score,
                    "doc_fetch_concurrency": cfg.ask_doc_fetch_concurrency,
                    "sources": ctx.diagnostic_sources,
                }
            })
        );
        return;
    }
    eprintln!("{}", primary("Ask Diagnostics"));
    eprintln!(
        "  {} candidates={} reranked={} chunks={} full_docs={} supplemental={} context_chars={}",
        muted("Retrieval:"),
        ctx.candidate_count,
        ctx.reranked_count,
        ctx.chunks_selected,
        ctx.full_docs_selected,
        ctx.supplemental_count,
        ctx.context.len()
    );
    for source in &ctx.diagnostic_sources {
        eprintln!("  • {source}");
    }
    eprintln!();
}

async fn ask_llm_answer(
    cfg: &Config,
    _query: &str,
    context: &str,
) -> Result<(String, u128), Box<dyn Error>> {
    let client = http_client()?;
    let llm_started = std::time::Instant::now();
    let streamed = ask_llm_streaming(cfg, client, _query, context, false).await;
    let answer = match streamed {
        Ok(value) => value,
        Err(e) => {
            log_warn(&format!(
                "streaming failed, falling back to non-streaming: {e}"
            ));
            ask_llm_non_streaming(cfg, client, _query, context).await?
        }
    };
    Ok((answer, llm_started.elapsed().as_millis()))
}

fn emit_ask_result(
    cfg: &Config,
    query: &str,
    answer: &str,
    ctx: &AskContext,
    llm_elapsed_ms: u128,
    total_elapsed_ms: u128,
) -> Result<(), Box<dyn Error>> {
    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "query": query,
                "answer": answer,
                "diagnostics": if cfg.ask_diagnostics {
                    serde_json::json!({
                        "candidate_pool": ctx.candidate_count,
                        "reranked_pool": ctx.reranked_count,
                        "chunks_selected": ctx.chunks_selected,
                        "full_docs_selected": ctx.full_docs_selected,
                        "supplemental_selected": ctx.supplemental_count,
                        "context_chars": ctx.context.len(),
                        "min_relevance_score": cfg.ask_min_relevance_score,
                        "doc_fetch_concurrency": cfg.ask_doc_fetch_concurrency,
                    })
                } else {
                    serde_json::Value::Null
                },
                "timing_ms": {
                    "retrieval": ctx.retrieval_elapsed_ms,
                    "context_build": ctx.context_elapsed_ms,
                    "llm": llm_elapsed_ms,
                    "total": total_elapsed_ms,
                }
            }))?
        );
        return Ok(());
    }
    if cfg.ask_diagnostics {
        println!(
            "  {} candidates={} reranked={} chunks={} full_docs={} supplemental={} context_chars={}",
            muted("Diagnostics:"),
            ctx.candidate_count,
            ctx.reranked_count,
            ctx.chunks_selected,
            ctx.full_docs_selected,
            ctx.supplemental_count,
            ctx.context.len()
        );
    }
    println!(
        "  {} retrieval={}ms | context={}ms | llm={}ms | total={}ms",
        muted("Timing:"),
        ctx.retrieval_elapsed_ms,
        ctx.context_elapsed_ms,
        llm_elapsed_ms,
        total_elapsed_ms
    );
    Ok(())
}

fn strip_sources_section(answer: &str) -> String {
    let lower = answer.to_ascii_lowercase();
    if lower.starts_with("## sources") {
        return String::new();
    }
    if let Some(idx) = lower.find("\n## sources") {
        return answer[..idx].trim_end().to_string();
    }
    answer.trim_end().to_string()
}

fn extract_cited_source_ids(text: &str) -> BTreeSet<usize> {
    let bytes = text.as_bytes();
    let mut out = BTreeSet::new();
    let mut i = 0usize;
    while i + 3 < bytes.len() {
        if bytes[i] == b'[' && (bytes[i + 1] == b'S' || bytes[i + 1] == b's') {
            let mut j = i + 2;
            let mut value: usize = 0;
            let mut saw_digit = false;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                saw_digit = true;
                value = value
                    .saturating_mul(10)
                    .saturating_add((bytes[j] - b'0') as usize);
                j += 1;
            }
            if saw_digit && j < bytes.len() && bytes[j] == b']' {
                out.insert(value);
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn parse_context_source_map(context: &str) -> BTreeMap<usize, String> {
    let mut out = BTreeMap::new();
    for line in context.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("## ") {
            continue;
        }
        let Some(start) = trimmed.find("[S") else {
            continue;
        };
        let rest = &trimmed[start + 2..];
        let Some(end_rel) = rest.find(']') else {
            continue;
        };
        let id_raw = &rest[..end_rel];
        let Ok(id) = id_raw.parse::<usize>() else {
            continue;
        };
        let Some(colon_idx) = trimmed.find(": ") else {
            continue;
        };
        let source = trimmed[colon_idx + 2..].trim();
        if !source.is_empty() {
            out.insert(id, source.to_string());
        }
    }
    out
}

fn indicates_insufficient_evidence(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    lower.contains("insufficient")
        || lower.contains("not enough information")
        || lower.contains("does not contain information")
        || lower.contains("no relevant information")
}

fn format_insufficient_evidence(
    source_map: &BTreeMap<usize, String>,
    cited: Option<&BTreeSet<usize>>,
) -> String {
    let suggestions = source_map
        .values()
        .take(3)
        .map(|source| format!("- Index authoritative documentation for: {source}"))
        .collect::<Vec<_>>();
    let suggestions_block = if suggestions.is_empty() {
        "- Index official product documentation and command reference pages for this topic."
            .to_string()
    } else {
        suggestions.join("\n")
    };
    let mut seen_sources: HashSet<String> = HashSet::new();
    let source_lines = cited
        .map(|ids| {
            ids.iter()
                .filter_map(|id| {
                    source_map.get(id).and_then(|source| {
                        if seen_sources.insert(source.clone()) {
                            Some(format!("- [S{id}] {source}"))
                        } else {
                            None
                        }
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let sources_block = if source_lines.is_empty() {
        "- None cited from retrieved context.".to_string()
    } else {
        source_lines.join("\n")
    };
    format!(
        "Insufficient evidence in indexed sources to answer this question reliably.\n\n\
## Why\n\
- Retrieved context did not contain a direct, source-grounded answer.\n\n\
## Next Index Targets\n\
{suggestions_block}\n\n\
## Sources\n\
{sources_block}"
    )
}

fn normalize_ask_answer(answer: &str, context: &str) -> String {
    let source_map = parse_context_source_map(context);
    let body = strip_sources_section(answer);
    let cited = extract_cited_source_ids(&body);
    if cited.is_empty() {
        return format_insufficient_evidence(&source_map, None);
    }
    if indicates_insufficient_evidence(&body) {
        return format_insufficient_evidence(&source_map, Some(&cited));
    }
    let mut seen_sources: HashSet<String> = HashSet::new();
    let source_lines = cited
        .iter()
        .filter_map(|id| {
            source_map.get(id).and_then(|source| {
                if seen_sources.insert(source.clone()) {
                    Some(format!("- [S{id}] {source}"))
                } else {
                    None
                }
            })
        })
        .collect::<Vec<_>>();
    if source_lines.is_empty() {
        return format_insufficient_evidence(&source_map, Some(&cited));
    }
    format!(
        "{}\n\n## Sources\n{}",
        body.trim_end(),
        source_lines.join("\n")
    )
}

pub async fn run_ask_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let ask_started = std::time::Instant::now();
    let query = ask_query(cfg)?;

    if cfg.openai_base_url.trim().is_empty() || cfg.openai_model.trim().is_empty() {
        return Err("OPENAI_BASE_URL and OPENAI_MODEL required for ask".into());
    }

    let ctx = build_ask_context(cfg, &query).await?;
    emit_ask_diagnostics(cfg, &ctx);
    let (raw_answer, llm_elapsed_ms) = ask_llm_answer(cfg, &query, &ctx.context).await?;
    let answer = normalize_ask_answer(&raw_answer, &ctx.context);
    if !cfg.json_output {
        println!("{}", primary("Conversation"));
        println!("  {} {}", primary("You:"), query);
        println!("  {} {}", primary("Assistant:"), answer);
    }
    let total_elapsed_ms = ask_started.elapsed().as_millis();
    emit_ask_result(cfg, &query, &answer, &ctx, llm_elapsed_ms, total_elapsed_ms)
}

#[cfg(test)]
mod tests {
    use super::{extract_cited_source_ids, normalize_ask_answer, parse_context_source_map};

    #[test]
    fn extract_cited_source_ids_deduplicates_ids() {
        let ids = extract_cited_source_ids("A [S1] B [S2][S1] C [s3]");
        assert_eq!(ids.into_iter().collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn normalize_ask_answer_replaces_sources_with_deduped_section() {
        let context = "Sources:\n## Top Chunk [S1]: https://a.dev/docs\n\n---\n\n## Top Chunk [S2]: https://b.dev/docs";
        let raw = "Use command X [S2] and Y [S1].\n\n## Sources\n- [S1] dup\n- [S1] dup";
        let normalized = normalize_ask_answer(raw, context);
        assert!(normalized.contains("Use command X [S2] and Y [S1]."));
        assert!(normalized.contains("## Sources"));
        assert!(normalized.contains("- [S1] https://a.dev/docs"));
        assert!(normalized.contains("- [S2] https://b.dev/docs"));
        assert!(!normalized.contains("dup"));
    }

    #[test]
    fn normalize_ask_answer_dedupes_sources_by_url() {
        let context = "Sources:\n## Top Chunk [S1]: https://same.dev/docs\n\n---\n\n## Top Chunk [S9]: https://same.dev/docs";
        let raw = "Use this flow [S1][S9].";
        let normalized = normalize_ask_answer(raw, context);
        assert!(normalized.contains("- [S1] https://same.dev/docs"));
        assert!(!normalized.contains("- [S9] https://same.dev/docs"));
    }

    #[test]
    fn normalize_ask_answer_formats_insufficient_evidence_when_uncited() {
        let context = "Sources:\n## Top Chunk [S1]: https://docs.example.com/guide";
        let raw = "I think this probably works, but not sure.";
        let normalized = normalize_ask_answer(raw, context);
        assert!(normalized.starts_with("Insufficient evidence in indexed sources"));
        assert!(normalized.contains("## Why"));
        assert!(normalized.contains("## Next Index Targets"));
        assert!(normalized.contains("## Sources\n- None cited from retrieved context."));
    }

    #[test]
    fn normalize_ask_answer_formats_insufficient_evidence_when_flagged_in_body() {
        let context = "Sources:\n## Top Chunk [S2]: https://docs.example.com/guide";
        let raw = "The indexed sources are insufficient to answer this question [S2].";
        let normalized = normalize_ask_answer(raw, context);
        assert!(normalized.starts_with("Insufficient evidence in indexed sources"));
        assert!(normalized.contains("## Why"));
        assert!(normalized.contains("## Sources\n- [S2] https://docs.example.com/guide"));
    }

    #[test]
    fn parse_context_source_map_reads_source_headers() {
        let context = "Sources:\n## Top Chunk [S1]: https://a.dev\n\n---\n\n## Source Document [S2]: https://b.dev";
        let map = parse_context_source_map(context);
        assert_eq!(map.get(&1).map(|s| s.as_str()), Some("https://a.dev"));
        assert_eq!(map.get(&2).map(|s| s.as_str()), Some("https://b.dev"));
    }
}
