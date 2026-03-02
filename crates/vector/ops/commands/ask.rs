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

fn ask_query(cfg: &Config) -> Result<String, Box<dyn Error>> {
    super::resolve_query_text(cfg).ok_or_else(|| "ask requires query".into())
}

pub async fn run_ask_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let ask_started = std::time::Instant::now();
    let query = ask_query(cfg)?;

    if cfg.openai_base_url.trim().is_empty() || cfg.openai_model.trim().is_empty() {
        return Err("OPENAI_BASE_URL and OPENAI_MODEL required for ask".into());
    }

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
