mod display;
mod scoring;
mod streaming;

use crate::crates::core::config::{Config, EvaluateResponsesMode};
use crate::crates::core::http::http_client;
use crate::crates::core::logging::log_warn;
use crate::crates::jobs::crawl::start_crawl_job;
use std::error::Error;
use std::time::Instant;

use super::ask::build_ask_context;
use super::suggest::discover_crawl_suggestions;
use display::{emit_analysis_header, emit_context_header, emit_evaluate_output, emit_event};
use scoring::{
    build_judge_reference, build_suggestion_focus, format_rag_sources, rag_underperformed,
};
use streaming::{
    STREAM_WITH_CONTEXT, STREAM_WITHOUT_CONTEXT, run_analysis, run_baseline_answer,
    run_parallel_answers_streaming, run_rag_answer,
};

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

struct EvalAnswers<'a> {
    rag: &'a str,
    baseline: &'a str,
    analysis: &'a str,
    crawl_suggestions: &'a [CrawlSuggestion],
    crawl_enqueue_outcomes: &'a [CrawlEnqueueOutcome],
    ref_chunk_count: usize,
    context_chars: usize,
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
            ("No reference material available.".to_string(), 0)
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
    let judge_ctx = super::streaming::JudgeContext {
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
    super::ask::validate_ask_llm_config(cfg).map_err(|e| -> Box<dyn Error> { e.into() })?;
    super::resolve_query_text(cfg).ok_or_else(|| "evaluate requires a question".into())
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

#[cfg(test)]
mod tests {
    use super::display::{build_side_by_side_frame, wrap_fixed_width};
    use super::scoring::{build_suggestion_focus, rag_underperformed, score_totals_from_analysis};

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
