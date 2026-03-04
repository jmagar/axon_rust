use crate::crates::core::config::{Config, EvaluateResponsesMode};
use crate::crates::core::http::http_client;
use crate::crates::core::logging::log_warn;
use crate::crates::core::ui::{muted, primary};
use crate::crates::jobs::crawl::start_crawl_job;
use crate::crates::vector::ops::{qdrant, ranking, tei};
use std::error::Error;
use std::io::{IsTerminal, Write};
use std::time::Instant;
use tokio::sync::mpsc;

const NO_REFERENCE: &str = "No reference material available.";
const STREAM_WITH_CONTEXT: &str = "with_context";
const STREAM_WITHOUT_CONTEXT: &str = "without_context";

use super::ask::build_ask_context;
use super::streaming::{
    JudgeContext, TaggedToken, ask_llm_non_streaming, ask_llm_streaming, ask_llm_streaming_tagged,
    baseline_llm_non_streaming, baseline_llm_streaming, baseline_llm_streaming_tagged,
    judge_llm_non_streaming, judge_llm_streaming,
};
use super::suggest::discover_crawl_suggestions;

struct EvalTiming {
    rag_elapsed_ms: u128,
    baseline_elapsed_ms: u128,
    research_elapsed_ms: u128,
    analysis_elapsed_ms: u128,
    total_elapsed_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CrawlSuggestion {
    url: String,
    reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CrawlEnqueueOutcome {
    url: String,
    job_id: Option<String>,
    error: Option<String>,
}

#[derive(Default)]
struct SideBySideBuffer {
    with_context: String,
    without_context: String,
}

impl SideBySideBuffer {
    fn new() -> Self {
        Self::default()
    }

    fn push(&mut self, stream: &str, delta: &str) {
        match stream {
            STREAM_WITH_CONTEXT => self.with_context.push_str(delta),
            STREAM_WITHOUT_CONTEXT => self.without_context.push_str(delta),
            _ => {}
        }
    }
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
    let reranked: Vec<ranking::AskCandidate> = ranking::rerank_ask_candidates(
        &candidates,
        &query_tokens,
        &cfg.ask_authoritative_domains,
        cfg.ask_authoritative_boost,
    )
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
    if !cfg.json_output && cfg.evaluate_responses_mode == EvaluateResponsesMode::Events {
        emit_event(&serde_json::json!({
            "type": "evaluate_start",
            "query": query,
            "stage": "building_context",
            "responses_mode": cfg.evaluate_responses_mode.to_string(),
        }))?;
    }
    let ctx = build_ask_context(cfg, &query).await?;
    emit_context_header(cfg, &query, &ctx)?;
    let ((rag_answer, rag_elapsed_ms), (baseline_answer, baseline_elapsed_ms)) = if cfg.json_output
    {
        let rag = run_rag_answer(cfg, client, &query, &ctx.context).await?;
        let baseline = run_baseline_answer(cfg, client, &query).await?;
        (rag, baseline)
    } else {
        run_parallel_answers_streaming(
            cfg,
            client,
            &query,
            &ctx.context,
            cfg.evaluate_responses_mode,
        )
        .await?
    };

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
    let judge_ctx = JudgeContext {
        query: &query,
        rag_answer: &rag_answer,
        baseline_answer: &baseline_answer,
        reference_chunks: &judge_reference,
        rag_sources_list: &rag_sources_list,
        ref_quality_note,
        rag_elapsed_ms,
        baseline_elapsed_ms,
        source_count,
        context_chars,
    };
    let (analysis_answer, analysis_elapsed_ms) = run_analysis(cfg, client, &judge_ctx).await;
    let crawl_suggestions = if rag_underperformed(&analysis_answer) {
        let focus = build_suggestion_focus(&query, &analysis_answer);
        discover_crawl_suggestions(cfg, &focus, 5)
            .await
            .unwrap_or_else(|e| {
                log_warn(&format!(
                    "evaluate: suggestion discovery failed after rag underperformance: {e}"
                ));
                Vec::new()
            })
            .into_iter()
            .map(|(url, reason)| CrawlSuggestion { url, reason })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let crawl_enqueue_outcomes = if crawl_suggestions.is_empty() {
        Vec::new()
    } else {
        enqueue_suggested_crawls(cfg, &crawl_suggestions).await
    };

    let timing = EvalTiming {
        rag_elapsed_ms,
        baseline_elapsed_ms,
        research_elapsed_ms,
        analysis_elapsed_ms,
        total_elapsed_ms: eval_started.elapsed().as_millis(),
    };
    let eval_answers = EvalAnswers {
        rag: &rag_answer,
        baseline: &baseline_answer,
        analysis: &analysis_answer,
        crawl_suggestions: &crawl_suggestions,
        crawl_enqueue_outcomes: &crawl_enqueue_outcomes,
        ref_chunk_count,
        context_chars,
    };
    emit_evaluate_output(cfg, &query, &ctx, &eval_answers, &timing)?;
    Ok(())
}

fn evaluate_query(cfg: &Config) -> Result<String, Box<dyn Error>> {
    if cfg.openai_base_url.trim().is_empty() || cfg.openai_model.trim().is_empty() {
        return Err("OPENAI_BASE_URL and OPENAI_MODEL required for evaluate".into());
    }
    super::resolve_query_text(cfg).ok_or_else(|| "evaluate requires a question".into())
}

fn emit_event(value: &serde_json::Value) -> Result<(), Box<dyn Error>> {
    println!("{}", serde_json::to_string(value)?);
    std::io::stdout().flush()?;
    Ok(())
}

fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .map(|v| v.clamp(80, 240))
        .unwrap_or(140)
}

fn char_len(value: &str) -> usize {
    value.chars().count()
}

fn pad_to_width(value: &str, width: usize) -> String {
    let current = char_len(value);
    if current >= width {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len() + (width - current));
    out.push_str(value);
    out.push_str(&" ".repeat(width - current));
    out
}

fn wrap_fixed_width(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    for raw_line in text.split('\n') {
        if raw_line.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut acc = String::new();
        let mut count = 0usize;
        for ch in raw_line.chars() {
            if count == width {
                lines.push(acc);
                acc = String::new();
                count = 0;
            }
            acc.push(ch);
            count += 1;
        }
        lines.push(acc);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn build_side_by_side_frame(
    total_width: usize,
    with_context: &str,
    without_context: &str,
) -> Vec<String> {
    let gutter = " │ ";
    let content_width = total_width.saturating_sub(2);
    let col_width = ((content_width.saturating_sub(gutter.len())) / 2).max(20);
    let left_header = pad_to_width("WITH CONTEXT", col_width);
    let right_header = pad_to_width("WITHOUT CONTEXT", col_width);
    let divider = format!(
        "{}{}{}",
        "─".repeat(col_width),
        "─┼─",
        "─".repeat(col_width)
    );
    let mut lines = vec![
        format!("  {left_header}{gutter}{right_header}"),
        format!("  {divider}"),
    ];

    let left_lines = wrap_fixed_width(with_context, col_width);
    let right_lines = wrap_fixed_width(without_context, col_width);
    let rows = left_lines.len().max(right_lines.len());
    for idx in 0..rows {
        let left = left_lines.get(idx).cloned().unwrap_or_default();
        let right = right_lines.get(idx).cloned().unwrap_or_default();
        lines.push(format!(
            "  {}{}{}",
            pad_to_width(&left, col_width),
            gutter,
            pad_to_width(&right, col_width)
        ));
    }
    lines
}

fn repaint_frame(lines: &[String], previous_lines: usize) -> Result<usize, Box<dyn Error>> {
    if previous_lines > 0 {
        print!("\x1b[{}A\x1b[J", previous_lines);
    }
    for line in lines {
        println!("{line}");
    }
    std::io::stdout().flush()?;
    Ok(lines.len())
}

fn parse_first_score(value: &str, label: &str) -> Option<f64> {
    let start = value.find(label)?;
    let tail = &value[start + label.len()..];
    let mut number = String::new();
    let mut seen = false;
    for ch in tail.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            number.push(ch);
            seen = true;
            continue;
        }
        if seen {
            break;
        }
    }
    number.parse::<f64>().ok()
}

fn score_totals_from_analysis(analysis: &str) -> Option<(f64, f64)> {
    let mut rag_total = 0.0f64;
    let mut baseline_total = 0.0f64;
    let mut score_rows = 0usize;
    for line in analysis.lines() {
        let rag = parse_first_score(line, "RAG: ");
        let base = parse_first_score(line, "Baseline: ");
        if let (Some(r), Some(b)) = (rag, base) {
            rag_total += r;
            baseline_total += b;
            score_rows += 1;
        }
    }
    if score_rows == 0 {
        return None;
    }
    Some((rag_total, baseline_total))
}

fn rag_underperformed(analysis: &str) -> bool {
    score_totals_from_analysis(analysis)
        .map(|(rag, baseline)| rag + 0.001 < baseline)
        .unwrap_or(false)
}

fn build_suggestion_focus(query: &str, analysis: &str) -> String {
    let mut weak_dimensions = Vec::new();
    for line in analysis.lines() {
        let rag = parse_first_score(line, "RAG: ");
        let base = parse_first_score(line, "Baseline: ");
        if let (Some(r), Some(b)) = (rag, base) {
            if r + 0.001 < b {
                weak_dimensions.push(line.trim().to_string());
            }
        }
    }
    if weak_dimensions.is_empty() {
        return query.to_string();
    }
    format!(
        "{query}\n\nRAG scored below baseline in these areas:\n- {}",
        weak_dimensions.join("\n- ")
    )
}

async fn enqueue_suggested_crawls(
    cfg: &Config,
    suggestions: &[CrawlSuggestion],
) -> Vec<CrawlEnqueueOutcome> {
    let mut outcomes = Vec::with_capacity(suggestions.len());
    for suggestion in suggestions {
        match start_crawl_job(cfg, &suggestion.url).await {
            Ok(job_id) => outcomes.push(CrawlEnqueueOutcome {
                url: suggestion.url.clone(),
                job_id: Some(job_id.to_string()),
                error: None,
            }),
            Err(err) => outcomes.push(CrawlEnqueueOutcome {
                url: suggestion.url.clone(),
                job_id: None,
                error: Some(err.to_string()),
            }),
        }
    }
    outcomes
}

fn emit_context_header(
    cfg: &Config,
    query: &str,
    ctx: &super::ask::AskContext,
) -> Result<(), Box<dyn Error>> {
    if cfg.json_output {
        return Ok(());
    }
    if cfg.evaluate_responses_mode == EvaluateResponsesMode::Events {
        let source_urls = extract_source_urls(&ctx.diagnostic_sources);
        return emit_event(&serde_json::json!({
            "type": "evaluate_context_ready",
            "query": query,
            "responses_mode": cfg.evaluate_responses_mode.to_string(),
            "context": {
                "source_count": ctx.chunks_selected + ctx.full_docs_selected + ctx.supplemental_count,
                "source_urls": source_urls,
                "chars": ctx.context.len(),
                "retrieval_ms": ctx.retrieval_elapsed_ms,
                "context_build_ms": ctx.context_elapsed_ms,
            }
        }));
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
    match cfg.evaluate_responses_mode {
        EvaluateResponsesMode::Inline => println!(
            "{}",
            primary("── Parallel Answers (with and without context) ────────────────")
        ),
        EvaluateResponsesMode::SideBySide => println!(
            "{}",
            primary("── Parallel Answers (side-by-side) ───────────────────────────")
        ),
        EvaluateResponsesMode::Events => {}
    }
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

async fn run_parallel_answers_streaming(
    cfg: &Config,
    client: &reqwest::Client,
    query: &str,
    context: &str,
    mode: EvaluateResponsesMode,
) -> Result<((String, u128), (String, u128)), Box<dyn Error>> {
    let (tx, mut rx) = mpsc::unbounded_channel::<TaggedToken>();
    let rag_tx = tx.clone();
    let baseline_tx = tx.clone();
    let rag_cfg = cfg.clone();
    let baseline_cfg = cfg.clone();
    let rag_client = client.clone();
    let baseline_client = client.clone();
    let rag_query = query.to_string();
    let baseline_query = query.to_string();
    let rag_context = context.to_string();

    let rag_future = async move {
        let started = Instant::now();
        let answer = match ask_llm_streaming_tagged(
            &rag_cfg,
            &rag_client,
            &rag_query,
            &rag_context,
            STREAM_WITH_CONTEXT,
            &rag_tx,
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                log_warn(&format!(
                    "rag parallel streaming failed, falling back to non-streaming: {e}"
                ));
                let fallback =
                    ask_llm_non_streaming(&rag_cfg, &rag_client, &rag_query, &rag_context).await?;
                let _ = rag_tx.send(TaggedToken {
                    stream: STREAM_WITH_CONTEXT,
                    delta: fallback.clone(),
                });
                fallback
            }
        };
        Ok::<(String, u128), Box<dyn Error>>((answer, started.elapsed().as_millis()))
    };

    let baseline_future = async move {
        let started = Instant::now();
        let answer = match baseline_llm_streaming_tagged(
            &baseline_cfg,
            &baseline_client,
            &baseline_query,
            STREAM_WITHOUT_CONTEXT,
            &baseline_tx,
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                log_warn(&format!(
                    "baseline parallel streaming failed, falling back to non-streaming: {e}"
                ));
                let fallback =
                    baseline_llm_non_streaming(&baseline_cfg, &baseline_client, &baseline_query)
                        .await?;
                let _ = baseline_tx.send(TaggedToken {
                    stream: STREAM_WITHOUT_CONTEXT,
                    delta: fallback.clone(),
                });
                fallback
            }
        };
        Ok::<(String, u128), Box<dyn Error>>((answer, started.elapsed().as_millis()))
    };

    tokio::pin!(rag_future);
    tokio::pin!(baseline_future);
    drop(tx);

    let mut active: Option<&'static str> = None;
    let mut side_by_side = SideBySideBuffer::new();
    let mut rendered_lines = 0usize;
    let mut rag_result: Option<(String, u128)> = None;
    let mut baseline_result: Option<(String, u128)> = None;
    let side_by_side_supported = std::io::stdout().is_terminal();

    loop {
        tokio::select! {
            evt = rx.recv() => {
                match evt {
                    Some(evt) => {
                        match mode {
                            EvaluateResponsesMode::Inline => {
                                let label = match evt.stream {
                                    STREAM_WITH_CONTEXT => "[RAG]",
                                    STREAM_WITHOUT_CONTEXT => "[BASE]",
                                    _ => "[STREAM]",
                                };
                                if active != Some(evt.stream) {
                                    if active.is_some() {
                                        println!();
                                    }
                                    print!("  {label} ");
                                    std::io::stdout().flush()?;
                                    active = Some(evt.stream);
                                }
                                print!("{}", evt.delta);
                                std::io::stdout().flush()?;
                            }
                            EvaluateResponsesMode::SideBySide => {
                                if side_by_side_supported {
                                    side_by_side.push(evt.stream, &evt.delta);
                                    let frame = build_side_by_side_frame(
                                        terminal_width(),
                                        &side_by_side.with_context,
                                        &side_by_side.without_context,
                                    );
                                    rendered_lines = repaint_frame(&frame, rendered_lines)?;
                                } else {
                                    let label = match evt.stream {
                                        STREAM_WITH_CONTEXT => "[RAG]",
                                        STREAM_WITHOUT_CONTEXT => "[BASE]",
                                        _ => "[STREAM]",
                                    };
                                    if active != Some(evt.stream) {
                                        if active.is_some() {
                                            println!();
                                        }
                                        print!("  {label} ");
                                        std::io::stdout().flush()?;
                                        active = Some(evt.stream);
                                    }
                                    print!("{}", evt.delta);
                                    std::io::stdout().flush()?;
                                }
                            }
                            EvaluateResponsesMode::Events => {
                                emit_event(&serde_json::json!({
                                    "type": "token",
                                    "stream": evt.stream,
                                    "delta": evt.delta,
                                }))?;
                            }
                        }
                    }
                    None => {
                        if rag_result.is_some() && baseline_result.is_some() {
                            break;
                        }
                    }
                }
            }
            res = &mut rag_future, if rag_result.is_none() => {
                let done = res?;
                if mode == EvaluateResponsesMode::Events {
                    emit_event(&serde_json::json!({
                        "type": "stream_done",
                        "stream": STREAM_WITH_CONTEXT,
                        "elapsed_ms": done.1,
                        "chars": done.0.len(),
                    }))?;
                }
                rag_result = Some(done);
                if rag_result.is_some() && baseline_result.is_some() && rx.is_closed() {
                    break;
                }
            }
            res = &mut baseline_future, if baseline_result.is_none() => {
                let done = res?;
                if mode == EvaluateResponsesMode::Events {
                    emit_event(&serde_json::json!({
                        "type": "stream_done",
                        "stream": STREAM_WITHOUT_CONTEXT,
                        "elapsed_ms": done.1,
                        "chars": done.0.len(),
                    }))?;
                }
                baseline_result = Some(done);
                if rag_result.is_some() && baseline_result.is_some() && rx.is_closed() {
                    break;
                }
            }
        }
    }

    if mode != EvaluateResponsesMode::Events {
        println!();
    }
    Ok((
        rag_result.ok_or("missing rag answer from parallel streaming")?,
        baseline_result.ok_or("missing baseline answer from parallel streaming")?,
    ))
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

fn extract_source_urls(diagnostic_sources: &[String]) -> Vec<String> {
    diagnostic_sources
        .iter()
        .map(|s| {
            s.split_once(" url=")
                .map_or_else(|| s.to_string(), |(_, u)| u.to_string())
        })
        .collect()
}

fn emit_analysis_header(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if cfg.json_output {
        return Ok(());
    }
    if cfg.evaluate_responses_mode == EvaluateResponsesMode::Events {
        return emit_event(&serde_json::json!({
            "type": "analysis_start",
        }));
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

async fn run_analysis(
    cfg: &Config,
    client: &reqwest::Client,
    judge_ctx: &JudgeContext<'_>,
) -> (String, u128) {
    let started = Instant::now();
    let print_tokens =
        !cfg.json_output && cfg.evaluate_responses_mode != EvaluateResponsesMode::Events;
    let answer = match judge_llm_streaming(cfg, client, judge_ctx, print_tokens).await {
        Ok(v) => v,
        Err(e) => {
            log_warn(&format!(
                "judge streaming failed, falling back to non-streaming: {e}"
            ));
            match judge_llm_non_streaming(cfg, client, judge_ctx).await {
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

struct EvalAnswers<'a> {
    rag: &'a str,
    baseline: &'a str,
    analysis: &'a str,
    crawl_suggestions: &'a [CrawlSuggestion],
    crawl_enqueue_outcomes: &'a [CrawlEnqueueOutcome],
    ref_chunk_count: usize,
    context_chars: usize,
}

fn emit_evaluate_output(
    cfg: &Config,
    query: &str,
    ctx: &super::ask::AskContext,
    answers: &EvalAnswers<'_>,
    timing: &EvalTiming,
) -> Result<(), Box<dyn Error>> {
    let source_urls = extract_source_urls(&ctx.diagnostic_sources);
    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "query": query,
                "rag_answer": answers.rag,
                "baseline_answer": answers.baseline,
                "analysis_answer": answers.analysis,
                "source_urls": source_urls,
                "crawl_suggestions": answers.crawl_suggestions.iter().map(|s| serde_json::json!({
                    "url": s.url,
                    "reason": s.reason,
                })).collect::<Vec<_>>(),
                "crawl_enqueue_outcomes": answers.crawl_enqueue_outcomes.iter().map(|o| serde_json::json!({
                    "url": o.url,
                    "job_id": o.job_id,
                    "error": o.error,
                })).collect::<Vec<_>>(),
                "ref_chunk_count": answers.ref_chunk_count,
                "diagnostics": if cfg.ask_diagnostics {
                    serde_json::json!({
                        "candidate_pool": ctx.candidate_count,
                        "reranked_pool": ctx.reranked_count,
                        "chunks_selected": ctx.chunks_selected,
                        "full_docs_selected": ctx.full_docs_selected,
                        "supplemental_selected": ctx.supplemental_count,
                        "context_chars": answers.context_chars,
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
    if cfg.evaluate_responses_mode == EvaluateResponsesMode::Events {
        emit_event(&serde_json::json!({
            "type": "evaluate_complete",
            "query": query,
            "rag_answer": answers.rag,
            "baseline_answer": answers.baseline,
            "analysis_answer": answers.analysis,
            "source_urls": source_urls,
            "crawl_suggestions": answers.crawl_suggestions.iter().map(|s| serde_json::json!({
                "url": s.url,
                "reason": s.reason,
            })).collect::<Vec<_>>(),
            "crawl_enqueue_outcomes": answers.crawl_enqueue_outcomes.iter().map(|o| serde_json::json!({
                "url": o.url,
                "job_id": o.job_id,
                "error": o.error,
            })).collect::<Vec<_>>(),
            "timing_ms": {
                "retrieval": ctx.retrieval_elapsed_ms,
                "context_build": ctx.context_elapsed_ms,
                "rag_llm": timing.rag_elapsed_ms,
                "baseline_llm": timing.baseline_elapsed_ms,
                "research_elapsed_ms": timing.research_elapsed_ms,
                "analysis_llm_ms": timing.analysis_elapsed_ms,
                "total": timing.total_elapsed_ms,
            }
        }))?;
        return Ok(());
    }

    if !answers.crawl_suggestions.is_empty() {
        println!();
        println!();
        println!(
            "{}",
            primary("── Suggested Sources To Crawl (RAG scored below baseline) ───")
        );
        for (idx, suggestion) in answers.crawl_suggestions.iter().enumerate() {
            println!("  {}. {}", idx + 1, suggestion.url);
            println!("     {}", muted(&suggestion.reason));
        }
        if !answers.crawl_enqueue_outcomes.is_empty() {
            println!();
            println!("  {}", muted("Auto-crawl enqueue results:"));
            for outcome in answers.crawl_enqueue_outcomes {
                match (&outcome.job_id, &outcome.error) {
                    (Some(job_id), _) => println!("    • {} -> {}", outcome.url, muted(job_id)),
                    (_, Some(err)) => println!("    • {} -> {}", outcome.url, muted(err)),
                    _ => {}
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::{
        build_side_by_side_frame, build_suggestion_focus, rag_underperformed,
        score_totals_from_analysis, wrap_fixed_width,
    };

    #[test]
    fn wrap_fixed_width_respects_limit() {
        let lines = wrap_fixed_width("abcdefghij", 4);
        assert_eq!(lines, vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn side_by_side_frame_contains_both_headers() {
        let frame = build_side_by_side_frame(100, "left answer", "right answer");
        assert!(frame[0].contains("WITH CONTEXT"));
        assert!(frame[0].contains("WITHOUT CONTEXT"));
        assert!(frame.iter().any(|line| line.contains("left answer")));
        assert!(frame.iter().any(|line| line.contains("right answer")));
    }

    #[test]
    fn score_totals_detects_rag_loss() {
        let analysis = "\
## Accuracy        RAG: 2/5 | Baseline: 4/5
## Relevance       RAG: 3/5 | Baseline: 4/5
## Completeness    RAG: 2/5 | Baseline: 4/5
## Specificity     RAG: 3/5 | Baseline: 4/5";
        let totals = score_totals_from_analysis(analysis).expect("expected parsed totals");
        assert!(totals.0 < totals.1);
        assert!(rag_underperformed(analysis));
    }

    #[test]
    fn score_totals_detects_rag_win() {
        let analysis = "\
## Accuracy        RAG: 5/5 | Baseline: 3/5
## Relevance       RAG: 5/5 | Baseline: 4/5";
        assert!(!rag_underperformed(analysis));
    }

    #[test]
    fn suggestion_focus_includes_weak_dimensions() {
        let analysis = "## Accuracy RAG: 2/5 | Baseline: 4/5";
        let focus = build_suggestion_focus("How does crawl fallback work?", analysis);
        assert!(focus.contains("How does crawl fallback work?"));
        assert!(focus.contains("RAG scored below baseline"));
        assert!(focus.contains("## Accuracy"));
    }
}
