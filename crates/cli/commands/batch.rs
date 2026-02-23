use crate::crates::cli::commands::common::parse_urls;
use crate::crates::core::config::Config;
use crate::crates::core::content::{to_markdown, url_to_filename};
use crate::crates::core::http::{fetch_html, http_client};
use crate::crates::core::logging::{log_done, log_warn};
use crate::crates::core::ui::{
    accent, confirm_destructive, muted, primary, status_text, symbol_for_status,
};
use crate::crates::jobs::batch_jobs::{
    cancel_batch_job, cleanup_batch_jobs, clear_batch_jobs, get_batch_job, list_batch_jobs,
    recover_stale_batch_jobs, run_batch_worker, start_batch_job,
};
use crate::crates::vector::ops::embed_path_native;
use indicatif::{ProgressBar, ProgressStyle};
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

type BatchFetchResult = Option<(u32, String, String)>;
type BatchFetchSet = tokio::task::JoinSet<BatchFetchResult>;

pub async fn run_batch(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if maybe_handle_batch_subcommand(cfg).await? {
        return Ok(());
    }

    let urls = parse_urls(cfg);
    if urls.is_empty() {
        return Err("batch requires at least one URL (positional or --urls)".into());
    }

    if !cfg.wait {
        return enqueue_batch_job(cfg, &urls).await;
    }

    run_batch_sync(cfg, urls).await
}

async fn maybe_handle_batch_subcommand(cfg: &Config) -> Result<bool, Box<dyn Error>> {
    let Some(subcmd) = cfg.positional.first().map(|s| s.as_str()) else {
        return Ok(false);
    };

    match subcmd {
        "status" => handle_batch_status(cfg).await?,
        "cancel" => handle_batch_cancel(cfg).await?,
        "errors" => handle_batch_errors(cfg).await?,
        "list" => handle_batch_list(cfg).await?,
        "cleanup" => handle_batch_cleanup(cfg).await?,
        "clear" => handle_batch_clear(cfg).await?,
        "worker" => run_batch_worker(cfg).await?,
        "recover" => handle_batch_recover(cfg).await?,
        _ => return Ok(false),
    }

    Ok(true)
}

fn parse_batch_job_id(cfg: &Config, action: &str) -> Result<Uuid, Box<dyn Error>> {
    let id = cfg
        .positional
        .get(1)
        .ok_or(format!("batch {action} requires <job-id>"))?;
    Ok(Uuid::parse_str(id)?)
}

async fn handle_batch_status(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let id = parse_batch_job_id(cfg, "status")?;
    match get_batch_job(cfg, id).await? {
        Some(job) => {
            if cfg.json_output {
                println!("{}", serde_json::to_string_pretty(&job)?);
            } else {
                println!(
                    "{} {}",
                    primary("Batch Status for"),
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
                if let Some(obs) = job
                    .result_json
                    .as_ref()
                    .and_then(|json| json.get("extraction_observability"))
                {
                    if let Some(tokens) = obs.get("total_tokens_estimated").and_then(|v| v.as_u64())
                    {
                        println!("  {} {}", muted("Extract tokens est:"), tokens);
                    }
                    if let Some(cost) = obs.get("estimated_cost_usd").and_then(|v| v.as_f64()) {
                        println!("  {} ${:.5}", muted("Extract cost est:"), cost);
                    }
                    if let Some(quality_band) = obs.get("quality_band").and_then(|v| v.as_str()) {
                        println!("  {} {}", muted("Extract quality:"), quality_band);
                    }
                }
                if let Some(queue_status) = job
                    .result_json
                    .as_ref()
                    .and_then(|json| json.get("queue_injection"))
                    .and_then(|json| json.get("queue_status"))
                    .and_then(|value| value.as_str())
                {
                    println!("  {} {}", muted("Queue injection:"), queue_status);
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

async fn handle_batch_cancel(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let id = parse_batch_job_id(cfg, "cancel")?;
    let canceled = cancel_batch_job(cfg, id).await?;
    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({"id": id, "canceled": canceled, "source": "rust"})
        );
    } else if canceled {
        println!(
            "{} canceled batch job {}",
            symbol_for_status("canceled"),
            accent(&id.to_string())
        );
        println!("Job ID: {id}");
    } else {
        println!(
            "{} no cancellable batch job found for {}",
            symbol_for_status("error"),
            accent(&id.to_string())
        );
        println!("Job ID: {id}");
    }
    Ok(())
}

async fn handle_batch_errors(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let id = parse_batch_job_id(cfg, "errors")?;
    match get_batch_job(cfg, id).await? {
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

async fn handle_batch_list(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let jobs = list_batch_jobs(cfg, 50).await?;
    if cfg.json_output {
        println!("{}", serde_json::to_string_pretty(&jobs)?);
        return Ok(());
    }

    println!("{}", primary("Batch Jobs"));
    if jobs.is_empty() {
        println!("  {}", muted("No batch jobs found."));
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

async fn handle_batch_cleanup(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let removed = cleanup_batch_jobs(cfg).await?;
    if cfg.json_output {
        println!("{}", serde_json::json!({"removed": removed}));
    } else {
        println!(
            "{} removed {} batch jobs",
            symbol_for_status("completed"),
            removed
        );
    }
    Ok(())
}

async fn handle_batch_clear(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if !confirm_destructive(cfg, "Clear all batch jobs and purge batch queue?")? {
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

    let removed = clear_batch_jobs(cfg).await?;
    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({"removed": removed, "queue_purged": true})
        );
    } else {
        println!(
            "{} cleared {} batch jobs and purged queue",
            symbol_for_status("completed"),
            removed
        );
    }
    Ok(())
}

async fn handle_batch_recover(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let reclaimed = recover_stale_batch_jobs(cfg).await?;
    if cfg.json_output {
        println!("{}", serde_json::json!({"reclaimed": reclaimed}));
    } else {
        println!(
            "{} reclaimed {} stale batch jobs",
            symbol_for_status("completed"),
            reclaimed
        );
    }
    Ok(())
}

async fn enqueue_batch_job(cfg: &Config, urls: &[String]) -> Result<(), Box<dyn Error>> {
    let job_id = start_batch_job(cfg, urls).await?;
    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({"job_id": job_id, "status": "pending", "source": "rust"})
        );
    } else {
        println!("  {} {}", primary("Batch Job"), accent(&job_id.to_string()));
        println!("  {}", muted("Status: pending"));
        println!("Job ID: {job_id}");
    }
    Ok(())
}

async fn run_batch_sync(cfg: &Config, urls: Vec<String>) -> Result<(), Box<dyn Error>> {
    let batch_dir = prepare_batch_output_dir(cfg).await?;
    let mut set = spawn_batch_fetch_tasks(cfg, urls)?;
    emit_batch_fetch_results(cfg, &batch_dir, &mut set).await?;

    if cfg.embed {
        embed_path_native(cfg, &batch_dir.to_string_lossy()).await?;
    }

    log_done("command=batch complete");
    Ok(())
}

async fn prepare_batch_output_dir(cfg: &Config) -> Result<std::path::PathBuf, Box<dyn Error>> {
    let batch_dir = cfg.output_dir.join("batch-markdown");
    if batch_dir.exists() {
        if std::env::var("AXON_NO_WIPE").is_ok() {
            log_warn(&format!(
                "AXON_NO_WIPE set - keeping existing batch dir: {}",
                batch_dir.display()
            ));
        } else {
            log_warn(&format!(
                "Clearing batch output directory: {}",
                batch_dir.display()
            ));
            tokio::fs::remove_dir_all(&batch_dir).await?;
        }
    }
    tokio::fs::create_dir_all(&batch_dir).await?;
    Ok(batch_dir)
}

fn spawn_batch_fetch_tasks(
    cfg: &Config,
    urls: Vec<String>,
) -> Result<BatchFetchSet, Box<dyn Error>> {
    let client = http_client()?.clone();
    let semaphore = Arc::new(tokio::sync::Semaphore::new(cfg.batch_concurrency.max(1)));
    let mut set = tokio::task::JoinSet::new();

    for (idx, url) in urls.into_iter().enumerate() {
        let client = client.clone();
        let sem = semaphore.clone();
        set.spawn(async move {
            let permit = sem.acquire_owned().await.ok()?;
            let _permit = permit;
            let html = fetch_html(&client, &url).await.ok()?;
            let markdown = to_markdown(&html);
            Some((idx as u32 + 1, url, markdown))
        });
    }

    Ok(set)
}

async fn emit_batch_fetch_results(
    cfg: &Config,
    batch_dir: &std::path::Path,
    set: &mut BatchFetchSet,
) -> Result<(), Box<dyn Error>> {
    let progress = ProgressBar::new(set.len() as u64);
    progress.enable_steady_tick(Duration::from_millis(120));
    progress.set_style(
        ProgressStyle::with_template(
            "{spinner:.cyan} [{elapsed_precise}] {wide_bar:.cyan/blue} {pos}/{len} fetched",
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar()),
    );

    while let Some(res) = set.join_next().await {
        progress.inc(1);
        if let Ok(Some((idx, url, markdown))) = res {
            let file = batch_dir.join(url_to_filename(&url, idx));
            tokio::fs::write(&file, &markdown).await?;
            if cfg.json_output {
                println!(
                    "{}",
                    serde_json::json!({
                        "url": url,
                        "file_path": file.to_string_lossy(),
                        "markdown_chars": markdown.chars().count()
                    })
                );
            } else {
                println!(
                    "  {} {} {}",
                    symbol_for_status("completed"),
                    accent(&url),
                    muted(&format!("-> {}", file.display()))
                );
            }
        }
    }
    progress.finish_with_message("batch fetch complete");
    Ok(())
}
