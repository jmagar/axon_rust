use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::http::http_client;
use crate::axon_cli::crates::core::logging::log_warn;
use crate::axon_cli::crates::core::ui::{muted, primary};
use crate::axon_cli::crates::vector::ops_v2::{qdrant, ranking, tei};
use std::error::Error;
use std::io::Write;
use std::time::Instant;

const NO_REFERENCE: &str = "No reference material available.";

use super::ask::build_ask_context;
use super::streaming::{
    ask_llm_non_streaming, ask_llm_streaming, baseline_llm_non_streaming, baseline_llm_streaming,
    judge_llm_non_streaming, judge_llm_streaming,
};

struct EvalTiming {
    rag_elapsed_ms: u128,
    baseline_elapsed_ms: u128,
    research_elapsed_ms: u128,
    analysis_elapsed_ms: u128,
    total_elapsed_ms: u128,
}

async fn build_judge_reference(
    cfg: &Config,
    question: &str,
) -> Result<(String, usize), Box<dyn Error>> {
    let mut vecs = tei::tei_embed(cfg, &[question.to_string()]).await?;
    if vecs.is_empty() {
        return Err("TEI returned no vector for judge reference".into());
    }
    let query_vec = vecs.remove(0);
    let hits = qdrant::qdrant_search(cfg, &query_vec, cfg.ask_candidate_limit * 2).await?;
    let query_tokens = ranking::tokenize_query(question);
    let mut candidates: Vec<ranking::AskCandidate> = Vec::new();
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
        return Ok((NO_REFERENCE.to_string(), 0));
    }
    let reranked: Vec<ranking::AskCandidate> =
        ranking::rerank_ask_candidates(&candidates, &query_tokens)
            .into_iter()
            .filter(|c| c.rerank_score >= cfg.ask_min_relevance_score)
            .collect();
    if reranked.is_empty() {
        return Ok((NO_REFERENCE.to_string(), 0));
    }
    let selected_indices = ranking::select_diverse_candidates(&reranked, 8, 2);
    let selected_count = selected_indices.len();
    let reference = selected_indices
        .iter()
        .enumerate()
        .map(|(i, &idx)| {
            format!(
                "## Reference [R{}]: {}\n\n{}\n\n---",
                i + 1,
                reranked[idx].url,
                reranked[idx].chunk_text
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    Ok((reference, selected_count))
}

pub async fn run_evaluate_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    run_evaluate_native_impl(cfg).await
}

async fn run_evaluate_native_impl(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let query = evaluate_query(cfg)?;
    let client = http_client()?;
    let eval_started = Instant::now();
    let ctx = build_ask_context(cfg, &query).await?;
    emit_context_header(cfg, &query, &ctx)?;

    let (rag_answer, rag_elapsed_ms) = run_rag_answer(cfg, client, &query, &ctx.context).await?;
    emit_baseline_header(cfg)?;
    let (baseline_answer, baseline_elapsed_ms) = run_baseline_answer(cfg, client, &query).await?;

    let research_started = Instant::now();
    let (judge_reference, ref_chunk_count) = build_judge_reference(cfg, &query)
        .await
        .unwrap_or_else(|e| {
            log_warn(&format!(
                "evaluate: judge reference retrieval failed (proceeding without grounding): {e}"
            ));
            (NO_REFERENCE.to_string(), 0)
        });
    let research_elapsed_ms = research_started.elapsed().as_millis();
    let rag_sources_list = format_rag_sources(&ctx.diagnostic_sources);
    let ref_quality_note = if ref_chunk_count < 3 {
        "\u{26a0}\u{fe0f}  Reference material is limited — accuracy scores may be less reliable.\n\n"
    } else {
        ""
    };
    emit_analysis_header(cfg)?;
    let source_count = ctx.chunks_selected + ctx.full_docs_selected + ctx.supplemental_count;
    let context_chars = ctx.context.len();
    let (analysis_answer, analysis_elapsed_ms) = run_analysis(
        cfg,
        client,
        &query,
        &rag_answer,
        &baseline_answer,
        &judge_reference,
        &rag_sources_list,
        ref_quality_note,
        rag_elapsed_ms,
        baseline_elapsed_ms,
        source_count,
        context_chars,
    )
    .await;

    let timing = EvalTiming {
        rag_elapsed_ms,
        baseline_elapsed_ms,
        research_elapsed_ms,
        analysis_elapsed_ms,
        total_elapsed_ms: eval_started.elapsed().as_millis(),
    };
    emit_evaluate_output(
        cfg,
        &query,
        &ctx,
        &rag_answer,
        &baseline_answer,
        &analysis_answer,
        ref_chunk_count,
        context_chars,
        &timing,
    )?;
    Ok(())
}

fn evaluate_query(cfg: &Config) -> Result<String, Box<dyn Error>> {
    if cfg.openai_base_url.trim().is_empty() || cfg.openai_model.trim().is_empty() {
        return Err("OPENAI_BASE_URL and OPENAI_MODEL required for evaluate".into());
    }
    super::resolve_query_text(cfg).ok_or_else(|| "evaluate requires a question".into())
}

fn emit_context_header(
    cfg: &Config,
    query: &str,
    ctx: &super::ask::AskContext,
) -> Result<(), Box<dyn Error>> {
    if cfg.json_output {
        return Ok(());
    }
    println!("{}", primary("Evaluate"));
    println!("  {} {}", primary("Question:"), query);
    println!(
        "  {} {} sources · {} chars  {}",
        primary("Context:"),
        ctx.chunks_selected + ctx.full_docs_selected + ctx.supplemental_count,
        ctx.context.len(),
        muted(&format!(
            "(retrieval={}ms · context={}ms)",
            ctx.retrieval_elapsed_ms, ctx.context_elapsed_ms
        ))
    );
    if cfg.ask_diagnostics {
        eprintln!(
            "  {} candidates={} reranked={} chunks={} full_docs={} supplemental={} context_chars={}",
            muted("Context detail:"),
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
    }
    println!();
    println!(
        "{}",
        primary("── RAG Answer (with context) ──────────────────────────────────")
    );
    print!("  ");
    std::io::stdout().flush()?;
    Ok(())
}

async fn run_rag_answer(
    cfg: &Config,
    client: &reqwest::Client,
    query: &str,
    context: &str,
) -> Result<(String, u128), Box<dyn Error>> {
    let started = Instant::now();
    let answer = match ask_llm_streaming(cfg, client, query, context, !cfg.json_output).await {
        Ok(v) => v,
        Err(e) => {
            log_warn(&format!(
                "rag streaming failed, falling back to non-streaming: {e}"
            ));
            let fallback = ask_llm_non_streaming(cfg, client, query, context).await?;
            if !cfg.json_output {
                print!("{fallback}");
            }
            fallback
        }
    };
    Ok((answer, started.elapsed().as_millis()))
}

fn emit_baseline_header(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if cfg.json_output {
        return Ok(());
    }
    println!();
    println!();
    println!(
        "{}",
        primary("── Baseline Answer (no context) ───────────────────────────────")
    );
    print!("  ");
    std::io::stdout().flush()?;
    Ok(())
}

async fn run_baseline_answer(
    cfg: &Config,
    client: &reqwest::Client,
    query: &str,
) -> Result<(String, u128), Box<dyn Error>> {
    let started = Instant::now();
    let answer = match baseline_llm_streaming(cfg, client, query, !cfg.json_output).await {
        Ok(v) => v,
        Err(e) => {
            log_warn(&format!(
                "baseline streaming failed, falling back to non-streaming: {e}"
            ));
            let fallback = baseline_llm_non_streaming(cfg, client, query).await?;
            if !cfg.json_output {
                print!("{fallback}");
            }
            fallback
        }
    };
    Ok((answer, started.elapsed().as_millis()))
}

fn format_rag_sources(diagnostic_sources: &[String]) -> String {
    if diagnostic_sources.is_empty() {
        return "None available".to_string();
    }
    diagnostic_sources
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let url = s.split_once(" url=").map_or(s.as_str(), |(_, u)| u);
            format!("[S{}] {}", i + 1, url)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn emit_analysis_header(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if cfg.json_output {
        return Ok(());
    }
    println!();
    println!();
    println!(
        "{}",
        primary("── Analysis ───────────────────────────────────────────────────")
    );
    print!("  ");
    std::io::stdout().flush()?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_analysis(
    cfg: &Config,
    client: &reqwest::Client,
    query: &str,
    rag_answer: &str,
    baseline_answer: &str,
    judge_reference: &str,
    rag_sources_list: &str,
    ref_quality_note: &str,
    rag_elapsed_ms: u128,
    baseline_elapsed_ms: u128,
    source_count: usize,
    context_chars: usize,
) -> (String, u128) {
    let started = Instant::now();
    let answer = match judge_llm_streaming(
        cfg,
        client,
        query,
        rag_answer,
        baseline_answer,
        judge_reference,
        rag_sources_list,
        ref_quality_note,
        rag_elapsed_ms,
        baseline_elapsed_ms,
        source_count,
        context_chars,
        !cfg.json_output,
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            log_warn(&format!(
                "judge streaming failed, falling back to non-streaming: {e}"
            ));
            match judge_llm_non_streaming(
                cfg,
                client,
                query,
                rag_answer,
                baseline_answer,
                judge_reference,
                rag_sources_list,
                ref_quality_note,
                rag_elapsed_ms,
                baseline_elapsed_ms,
                source_count,
                context_chars,
            )
            .await
            {
                Ok(fallback) => {
                    if !cfg.json_output {
                        print!("{fallback}");
                    }
                    fallback
                }
                Err(e2) => {
                    log_warn(&format!(
                        "evaluate: both streaming and non-streaming judge failed: {e2}"
                    ));
                    String::from(
                        "(judge unavailable — both streaming and non-streaming LLM calls failed)",
                    )
                }
            }
        }
    };
    (answer, started.elapsed().as_millis())
}

#[allow(clippy::too_many_arguments)]
fn emit_evaluate_output(
    cfg: &Config,
    query: &str,
    ctx: &super::ask::AskContext,
    rag_answer: &str,
    baseline_answer: &str,
    analysis_answer: &str,
    ref_chunk_count: usize,
    context_chars: usize,
    timing: &EvalTiming,
) -> Result<(), Box<dyn Error>> {
    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "query": query,
                "rag_answer": rag_answer,
                "baseline_answer": baseline_answer,
                "analysis_answer": analysis_answer,
                "ref_chunk_count": ref_chunk_count,
                "diagnostics": if cfg.ask_diagnostics {
                    serde_json::json!({
                        "candidate_pool": ctx.candidate_count,
                        "reranked_pool": ctx.reranked_count,
                        "chunks_selected": ctx.chunks_selected,
                        "full_docs_selected": ctx.full_docs_selected,
                        "supplemental_selected": ctx.supplemental_count,
                        "context_chars": context_chars,
                        "min_relevance_score": cfg.ask_min_relevance_score,
                        "doc_fetch_concurrency": cfg.ask_doc_fetch_concurrency,
                    })
                } else {
                    serde_json::Value::Null
                },
                "timing_ms": {
                    "retrieval": ctx.retrieval_elapsed_ms,
                    "context_build": ctx.context_elapsed_ms,
                    "rag_llm": timing.rag_elapsed_ms,
                    "baseline_llm": timing.baseline_elapsed_ms,
                    "research_elapsed_ms": timing.research_elapsed_ms,
                    "analysis_llm_ms": timing.analysis_elapsed_ms,
                    "total": timing.total_elapsed_ms,
                }
            }))?
        );
        return Ok(());
    }
    println!();
    println!();
    println!(
        "  {} rag_llm={}ms | baseline_llm={}ms | research={}ms | analysis_llm={}ms | total={}ms",
        muted("Timing:"),
        timing.rag_elapsed_ms,
        timing.baseline_elapsed_ms,
        timing.research_elapsed_ms,
        timing.analysis_elapsed_ms,
        timing.total_elapsed_ms
    );
    Ok(())
}
