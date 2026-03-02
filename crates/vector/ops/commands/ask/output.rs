use crate::crates::core::config::Config;
use crate::crates::core::http::http_client;
use crate::crates::core::logging::log_warn;
use crate::crates::core::ui::{muted, primary};
use std::error::Error;

use super::super::streaming::{ask_llm_non_streaming, ask_llm_streaming};
use super::context::AskContext;

pub(crate) fn emit_ask_diagnostics(cfg: &Config, ctx: &AskContext) {
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
                    "top_domains": ctx.top_domains,
                    "authority_ratio": ctx.authoritative_ratio,
                    "dropped_by_allowlist": ctx.dropped_by_allowlist,
                    "sources": ctx.diagnostic_sources,
                }
            })
        );
        return;
    }
    eprintln!("{}", primary("Ask Diagnostics"));
    eprintln!(
        "  {} candidates={} reranked={} chunks={} full_docs={} supplemental={} context_chars={} authority_ratio={:.2} dropped_by_allowlist={}",
        muted("Retrieval:"),
        ctx.candidate_count,
        ctx.reranked_count,
        ctx.chunks_selected,
        ctx.full_docs_selected,
        ctx.supplemental_count,
        ctx.context.len(),
        ctx.authoritative_ratio,
        ctx.dropped_by_allowlist
    );
    if !ctx.top_domains.is_empty() {
        eprintln!("  {} {}", muted("Top domains:"), ctx.top_domains.join(", "));
    }
    for source in &ctx.diagnostic_sources {
        eprintln!("  • {source}");
    }
    eprintln!();
}

pub(crate) async fn ask_llm_answer(
    cfg: &Config,
    _query: &str,
    context: &str,
) -> Result<(String, u128, bool), Box<dyn Error>> {
    let client = http_client()?;
    let llm_started = std::time::Instant::now();
    let stream_to_stdout = !cfg.json_output;
    let streamed = ask_llm_streaming(cfg, client, _query, context, stream_to_stdout).await;
    let streamed_ok = streamed.is_ok();
    let answer = match streamed {
        Ok(value) => value,
        Err(e) => {
            log_warn(&format!(
                "streaming failed, falling back to non-streaming: {e}"
            ));
            ask_llm_non_streaming(cfg, client, _query, context).await?
        }
    };
    Ok((
        answer,
        llm_started.elapsed().as_millis(),
        stream_to_stdout && streamed_ok,
    ))
}

pub(crate) fn emit_ask_result(
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
                        "top_domains": ctx.top_domains,
                        "authority_ratio": ctx.authoritative_ratio,
                        "dropped_by_allowlist": ctx.dropped_by_allowlist,
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
            "  {} candidates={} reranked={} chunks={} full_docs={} supplemental={} context_chars={} authority_ratio={:.2} dropped_by_allowlist={}",
            muted("Diagnostics:"),
            ctx.candidate_count,
            ctx.reranked_count,
            ctx.chunks_selected,
            ctx.full_docs_selected,
            ctx.supplemental_count,
            ctx.context.len(),
            ctx.authoritative_ratio,
            ctx.dropped_by_allowlist
        );
        if !ctx.top_domains.is_empty() {
            println!("  {} {}", muted("Top domains:"), ctx.top_domains.join(", "));
        }
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
