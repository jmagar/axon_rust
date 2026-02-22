use crate::crates::cli::commands::common::parse_urls;
use crate::crates::core::config::Config;
use crate::crates::core::content::{run_extract_with_engine, DeterministicExtractionEngine};
use crate::crates::core::logging::log_done;
use crate::crates::core::ui::{
    accent, confirm_destructive, muted, primary, status_text, symbol_for_status,
};
use crate::crates::jobs::extract_jobs::{
    cancel_extract_job, cleanup_extract_jobs, clear_extract_jobs, get_extract_job,
    list_extract_jobs, recover_stale_extract_jobs, run_extract_worker, start_extract_job,
};
use futures_util::stream::FuturesUnordered;
use futures_util::StreamExt;
use std::error::Error;
use std::sync::Arc;
use uuid::Uuid;

pub async fn run_extract(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if maybe_handle_extract_subcommand(cfg).await? {
        return Ok(());
    }

    let urls = parse_urls(cfg);
    if urls.is_empty() {
        return Err("extract requires at least one URL (positional or --urls)".into());
    }
    let prompt = require_extract_prompt(cfg)?;

    if !cfg.wait {
        return enqueue_extract_job(cfg, &urls, prompt).await;
    }

    run_extract_sync(cfg, urls, &prompt).await
}

async fn maybe_handle_extract_subcommand(cfg: &Config) -> Result<bool, Box<dyn Error>> {
    let Some(subcmd) = cfg.positional.first().map(|s| s.as_str()) else {
        return Ok(false);
    };

    match subcmd {
        "status" => handle_extract_status(cfg).await?,
        "cancel" => handle_extract_cancel(cfg).await?,
        "errors" => handle_extract_errors(cfg).await?,
        "list" => handle_extract_list(cfg).await?,
        "cleanup" => handle_extract_cleanup(cfg).await?,
        "clear" => handle_extract_clear(cfg).await?,
        "worker" => run_extract_worker(cfg).await?,
        "recover" => handle_extract_recover(cfg).await?,
        _ => return Ok(false),
    }

    Ok(true)
}

fn parse_extract_job_id(cfg: &Config, action: &str) -> Result<Uuid, Box<dyn Error>> {
    let id = cfg
        .positional
        .get(1)
        .ok_or(format!("extract {action} requires <job-id>"))?;
    Ok(Uuid::parse_str(id)?)
}

async fn handle_extract_status(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let id = parse_extract_job_id(cfg, "status")?;
    match get_extract_job(cfg, id).await? {
        Some(job) => {
            if cfg.json_output {
                println!("{}", serde_json::to_string_pretty(&job)?);
            } else {
                println!(
                    "{} {}",
                    primary("Extract Status for"),
                    accent(&job.id.to_string())
                );
                println!(
                    "  {} {}",
                    symbol_for_status(&job.status),
                    status_text(&job.status)
                );
                println!("  {} {}", muted("Created:"), job.created_at);
                println!("  {} {}", muted("Updated:"), job.updated_at);
                if let Some(err) = job.error_text.as_deref() {
                    println!("  {} {}", muted("Error:"), err);
                }
                println!("Job ID: {}", job.id);
            }
        }
        None => println!(
            "{} {}",
            symbol_for_status("error"),
            muted(&format!("job not found: {id}"))
        ),
    }
    Ok(())
}

async fn handle_extract_cancel(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let id = parse_extract_job_id(cfg, "cancel")?;
    let canceled = cancel_extract_job(cfg, id).await?;
    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({"id": id, "canceled": canceled, "source": "rust"})
        );
    } else if canceled {
        println!(
            "{} canceled extract job {}",
            symbol_for_status("canceled"),
            accent(&id.to_string())
        );
        println!("Job ID: {id}");
    } else {
        println!(
            "{} no cancellable extract job found for {}",
            symbol_for_status("error"),
            accent(&id.to_string())
        );
        println!("Job ID: {id}");
    }
    Ok(())
}

async fn handle_extract_errors(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let id = parse_extract_job_id(cfg, "errors")?;
    match get_extract_job(cfg, id).await? {
        Some(job) => {
            if cfg.json_output {
                println!(
                    "{}",
                    serde_json::json!({"id": id, "status": job.status, "error": job.error_text})
                );
            } else {
                println!(
                    "{} {} {}",
                    symbol_for_status(&job.status),
                    accent(&id.to_string()),
                    status_text(&job.status)
                );
                println!(
                    "  {} {}",
                    muted("Error:"),
                    job.error_text.unwrap_or_else(|| "None".to_string())
                );
                println!("Job ID: {id}");
            }
        }
        None => println!(
            "{} {}",
            symbol_for_status("error"),
            muted(&format!("job not found: {id}"))
        ),
    }
    Ok(())
}

async fn handle_extract_list(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let jobs = list_extract_jobs(cfg, 50).await?;
    if cfg.json_output {
        println!("{}", serde_json::to_string_pretty(&jobs)?);
        return Ok(());
    }

    println!("{}", primary("Extract Jobs"));
    if jobs.is_empty() {
        println!("  {}", muted("No extract jobs found."));
        return Ok(());
    }

    for job in jobs {
        println!(
            "  {} {} {}",
            symbol_for_status(&job.status),
            accent(&job.id.to_string()),
            status_text(&job.status)
        );
    }
    Ok(())
}

async fn handle_extract_cleanup(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let removed = cleanup_extract_jobs(cfg).await?;
    if cfg.json_output {
        println!("{}", serde_json::json!({"removed": removed}));
    } else {
        println!(
            "{} removed {} extract jobs",
            symbol_for_status("completed"),
            removed
        );
    }
    Ok(())
}

async fn handle_extract_clear(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if !confirm_destructive(cfg, "Clear all extract jobs and purge extract queue?")? {
        if cfg.json_output {
            println!(
                "{}",
                serde_json::json!({"removed": 0, "queue_purged": false})
            );
        } else {
            println!("{} aborted", symbol_for_status("canceled"));
        }
        return Ok(());
    }

    let removed = clear_extract_jobs(cfg).await?;
    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({"removed": removed, "queue_purged": true})
        );
    } else {
        println!(
            "{} cleared {} extract jobs and purged queue",
            symbol_for_status("completed"),
            removed
        );
    }
    Ok(())
}

async fn handle_extract_recover(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let reclaimed = recover_stale_extract_jobs(cfg).await?;
    if cfg.json_output {
        println!("{}", serde_json::json!({"reclaimed": reclaimed}));
    } else {
        println!(
            "{} reclaimed {} stale extract jobs",
            symbol_for_status("completed"),
            reclaimed
        );
    }
    Ok(())
}

fn require_extract_prompt(cfg: &Config) -> Result<String, Box<dyn Error>> {
    cfg.query
        .as_ref()
        .ok_or("extract requires --query <prompt>")
        .map(|v| v.to_string())
        .map_err(Into::into)
}

async fn enqueue_extract_job(
    cfg: &Config,
    urls: &[String],
    prompt: String,
) -> Result<(), Box<dyn Error>> {
    let job_id = start_extract_job(cfg, urls, Some(prompt)).await?;
    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({"job_id": job_id, "status": "pending", "source": "rust"})
        );
    } else {
        println!(
            "  {} {}",
            primary("Extract Job"),
            accent(&job_id.to_string())
        );
        println!("  {}", muted("Status: pending"));
        println!("Job ID: {job_id}");
    }
    Ok(())
}

#[derive(Default)]
struct ExtractAggregation {
    runs: Vec<serde_json::Value>,
    all_results: Vec<serde_json::Value>,
    pages_visited: usize,
    pages_with_data: usize,
    deterministic_pages: usize,
    llm_fallback_pages: usize,
    llm_requests: usize,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    estimated_cost_usd: f64,
    parser_hits: serde_json::Map<String, serde_json::Value>,
}

async fn run_extract_sync(
    cfg: &Config,
    urls: Vec<String>,
    prompt: &str,
) -> Result<(), Box<dyn Error>> {
    let runs = execute_extract_runs(cfg, &urls, prompt).await?;
    let output = build_extract_output(cfg, &urls, prompt, runs)?;
    let output_path = write_extract_output(cfg, &output).await?;
    emit_extract_output(cfg, &output, &output_path)?;
    log_done("command=extract complete");
    Ok(())
}

// Design note: axon_rust uses its own DeterministicExtractionEngine rather than
// spider_agent::Agent::extract() for performance reasons — deterministic parsing
// is O(1) in LLM calls and works offline, while spider_agent's extraction requires
// an LLM API call per page. For complex visual layouts, spider_agent's multimodal
// extraction is more powerful; use it by replacing this function with Agent::extract().
async fn execute_extract_runs(
    cfg: &Config,
    urls: &[String],
    prompt: &str,
) -> Result<ExtractAggregation, Box<dyn Error>> {
    let engine = Arc::new(DeterministicExtractionEngine::with_default_parsers());
    let max_pages = cfg.max_pages;
    let openai_base_url = cfg.openai_base_url.clone();
    let openai_api_key = cfg.openai_api_key.clone();
    let openai_model = cfg.openai_model.clone();

    let mut pending_runs = FuturesUnordered::new();
    for url in urls.iter().cloned() {
        let engine = Arc::clone(&engine);
        let prompt = prompt.to_string();
        let openai_base_url = openai_base_url.clone();
        let openai_api_key = openai_api_key.clone();
        let openai_model = openai_model.clone();
        pending_runs.push(async move {
            let run = run_extract_with_engine(
                &url,
                &prompt,
                max_pages,
                &openai_base_url,
                &openai_api_key,
                &openai_model,
                engine,
            )
            .await;
            (url, run)
        });
    }

    let mut aggregated = ExtractAggregation::default();
    while let Some((_url, run_result)) = pending_runs.next().await {
        let run = run_result?;
        aggregated.pages_visited += run.pages_visited;
        aggregated.pages_with_data += run.pages_with_data;
        aggregated.deterministic_pages += run.metrics.deterministic_pages;
        aggregated.llm_fallback_pages += run.metrics.llm_fallback_pages;
        aggregated.llm_requests += run.metrics.llm_requests;
        aggregated.prompt_tokens += run.metrics.prompt_tokens;
        aggregated.completion_tokens += run.metrics.completion_tokens;
        aggregated.total_tokens += run.metrics.total_tokens;
        aggregated.estimated_cost_usd += run.metrics.estimated_cost_usd;
        for (name, count) in &run.parser_hits {
            let current = aggregated
                .parser_hits
                .get(name.as_str())
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            aggregated
                .parser_hits
                .insert(name.clone(), serde_json::json!(current + *count as u64));
        }
        aggregated.all_results.extend(run.results.clone());
        aggregated.runs.push(serde_json::json!({
            "url": run.start_url,
            "pages_visited": run.pages_visited,
            "pages_with_data": run.pages_with_data,
            "deterministic_pages": run.metrics.deterministic_pages,
            "llm_fallback_pages": run.metrics.llm_fallback_pages,
            "llm_requests": run.metrics.llm_requests,
            "prompt_tokens": run.metrics.prompt_tokens,
            "completion_tokens": run.metrics.completion_tokens,
            "total_tokens": run.metrics.total_tokens,
            "estimated_cost_usd": run.metrics.estimated_cost_usd,
            "parser_hits": run.parser_hits,
            "total_items": run.results.len(),
            "results": run.results
        }));
    }

    Ok(aggregated)
}

fn build_extract_output(
    cfg: &Config,
    urls: &[String],
    prompt: &str,
    aggregated: ExtractAggregation,
) -> Result<serde_json::Value, Box<dyn Error>> {
    Ok(serde_json::json!({
        "urls": urls,
        "prompt": prompt,
        "model": cfg.openai_model,
        "pages_visited": aggregated.pages_visited,
        "pages_with_data": aggregated.pages_with_data,
        "deterministic_pages": aggregated.deterministic_pages,
        "llm_fallback_pages": aggregated.llm_fallback_pages,
        "llm_requests": aggregated.llm_requests,
        "prompt_tokens": aggregated.prompt_tokens,
        "completion_tokens": aggregated.completion_tokens,
        "total_tokens": aggregated.total_tokens,
        "estimated_cost_usd": aggregated.estimated_cost_usd,
        "parser_hits": aggregated.parser_hits,
        "total_items": aggregated.all_results.len(),
        "runs": aggregated.runs,
        "results": aggregated.all_results
    }))
}

async fn write_extract_output(
    cfg: &Config,
    output: &serde_json::Value,
) -> Result<std::path::PathBuf, Box<dyn Error>> {
    let output_path = cfg
        .output_path
        .clone()
        .unwrap_or_else(|| cfg.output_dir.join("extract.json"));
    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&output_path, serde_json::to_string_pretty(output)?).await?;
    Ok(output_path)
}

fn emit_extract_output(
    cfg: &Config,
    output: &serde_json::Value,
    output_path: &std::path::Path,
) -> Result<(), Box<dyn Error>> {
    if cfg.json_output {
        println!("{}", serde_json::to_string_pretty(output)?);
        return Ok(());
    }

    println!("{}", primary("Extract Results"));
    println!("  {} {}", muted("Pages visited:"), output["pages_visited"]);
    println!(
        "  {} {}",
        muted("Pages with data:"),
        output["pages_with_data"]
    );
    println!(
        "  {} {}",
        muted("Deterministic pages:"),
        output["deterministic_pages"]
    );
    println!(
        "  {} {}",
        muted("LLM fallback pages:"),
        output["llm_fallback_pages"]
    );
    println!("  {} {}", muted("LLM requests:"), output["llm_requests"]);
    println!("  {} {}", muted("Total tokens:"), output["total_tokens"]);
    println!(
        "  {} {:.6}",
        muted("Estimated cost (USD):"),
        output["estimated_cost_usd"].as_f64().unwrap_or(0.0)
    );
    println!("  {} {}", muted("Total items:"), output["total_items"]);
    println!("  {} {}", muted("Saved:"), output_path.display());
    Ok(())
}
