use crate::crates::core::config::Config;
use crate::crates::core::ui::primary;
use std::error::Error;
use std::io::Write;

mod context;
mod normalize;
mod output;
#[cfg(test)]
mod tests;

pub(crate) use context::{AskContext, build_ask_context};

pub(super) fn validate_ask_llm_config(cfg: &Config) -> Result<(), String> {
    if cfg.openai_base_url.trim().is_empty() || cfg.openai_model.trim().is_empty() {
        return Err(
            "OPENAI_BASE_URL and OPENAI_MODEL are required for ask/evaluate commands".to_string(),
        );
    }
    Ok(())
}

fn ask_query(cfg: &Config) -> Result<String, Box<dyn Error>> {
    super::resolve_query_text(cfg).ok_or_else(|| "ask requires query".into())
}

pub async fn ask_payload(cfg: &Config, query: &str) -> Result<serde_json::Value, String> {
    let ask_started = std::time::Instant::now();

    validate_ask_llm_config(cfg)?;

    let ctx = build_ask_context(cfg, query)
        .await
        .map_err(|e| e.to_string())?;
    let (raw_answer, llm_elapsed_ms, _) = output::ask_llm_answer(cfg, query, &ctx.context)
        .await
        .map_err(|e| e.to_string())?;
    let answer = normalize::normalize_ask_answer(cfg, query, &raw_answer, &ctx.context);
    let total_elapsed_ms = ask_started.elapsed().as_millis();

    Ok(serde_json::json!({
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
    }))
}

pub async fn run_ask_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let ask_started = std::time::Instant::now();
    let query = ask_query(cfg)?;

    validate_ask_llm_config(cfg).map_err(|e| -> Box<dyn Error> { e.into() })?;

    let ctx = build_ask_context(cfg, &query).await?;
    output::emit_ask_diagnostics(cfg, &ctx);
    if !cfg.json_output {
        println!("{}", primary("Conversation"));
        println!("  {} {}", primary("You:"), query);
        print!("  {} ", primary("Assistant:"));
        std::io::stdout().flush()?;
    }
    let (raw_answer, llm_elapsed_ms, streamed_to_stdout) =
        output::ask_llm_answer(cfg, &query, &ctx.context).await?;
    let answer = normalize::normalize_ask_answer(cfg, &query, &raw_answer, &ctx.context);
    if !cfg.json_output && streamed_to_stdout {
        println!();
    }
    if !cfg.json_output && !streamed_to_stdout {
        println!("  {} {}", primary("Assistant:"), answer);
    }
    let total_elapsed_ms = ask_started.elapsed().as_millis();
    output::emit_ask_result(cfg, &query, &answer, &ctx, llm_elapsed_ms, total_elapsed_ms)
}
