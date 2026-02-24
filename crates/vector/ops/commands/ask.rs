use crate::crates::core::config::Config;
use crate::crates::core::http::http_client;
use crate::crates::core::logging::log_warn;
use crate::crates::core::ui::{muted, primary};
use std::error::Error;
use std::io::Write;

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
    query: &str,
    context: &str,
) -> Result<(String, u128), Box<dyn Error>> {
    let client = http_client()?;
    let llm_started = std::time::Instant::now();
    if !cfg.json_output {
        println!("{}", primary("Conversation"));
        println!("  {} {}", primary("You:"), query);
        print!("  {} ", primary("Assistant:"));
        std::io::stdout().flush()?;
    }
    let streamed = ask_llm_streaming(cfg, client, query, context, !cfg.json_output).await;
    let answer = match streamed {
        Ok(value) => value,
        Err(e) => {
            log_warn(&format!(
                "streaming failed, falling back to non-streaming: {e}"
            ));
            let fallback = ask_llm_non_streaming(cfg, client, query, context).await?;
            if !cfg.json_output {
                print!("{fallback}");
            }
            fallback
        }
    };
    if !cfg.json_output {
        println!();
    }
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

pub async fn run_ask_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let ask_started = std::time::Instant::now();
    let query = ask_query(cfg)?;

    if cfg.openai_base_url.trim().is_empty() || cfg.openai_model.trim().is_empty() {
        return Err("OPENAI_BASE_URL and OPENAI_MODEL required for ask".into());
    }

    let ctx = build_ask_context(cfg, &query).await?;
    emit_ask_diagnostics(cfg, &ctx);
    let (answer, llm_elapsed_ms) = ask_llm_answer(cfg, &query, &ctx.context).await?;
    let total_elapsed_ms = ask_started.elapsed().as_millis();
    emit_ask_result(cfg, &query, &answer, &ctx, llm_elapsed_ms, total_elapsed_ms)
}
