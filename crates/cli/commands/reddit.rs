use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::logging::log_done;
use crate::axon_cli::crates::core::ui::confirm_destructive;
use crate::axon_cli::crates::core::ui::{accent, muted, primary, status_text, symbol_for_status};
use crate::axon_cli::crates::jobs::ingest_jobs::{
    cancel_ingest_job, cleanup_ingest_jobs, clear_ingest_jobs, get_ingest_job, list_ingest_jobs,
    recover_stale_ingest_jobs, run_ingest_worker, start_ingest_job, IngestSource,
};
use std::error::Error;
use uuid::Uuid;

pub async fn run_reddit(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if maybe_handle_ingest_subcommand(cfg).await? {
        return Ok(());
    }

    let target = cfg
        .positional
        .first()
        .cloned()
        .ok_or("reddit requires <TARGET> (subreddit name or thread URL)")?;

    let source = IngestSource::Reddit { target };

    if !cfg.wait {
        return enqueue_ingest_job(cfg, source).await;
    }

    run_ingest_sync(cfg, source).await
}

async fn maybe_handle_ingest_subcommand(cfg: &Config) -> Result<bool, Box<dyn Error>> {
    let Some(subcmd) = cfg.positional.first().map(|s| s.as_str()) else {
        return Ok(false);
    };

    match subcmd {
        "status" => handle_ingest_status(cfg).await?,
        "cancel" => handle_ingest_cancel(cfg).await?,
        "list" => handle_ingest_list(cfg).await?,
        "cleanup" => handle_ingest_cleanup(cfg).await?,
        "clear" => handle_ingest_clear(cfg).await?,
        "worker" => run_ingest_worker(cfg).await?,
        "recover" => handle_ingest_recover(cfg).await?,
        _ => return Ok(false),
    }

    Ok(true)
}

fn parse_ingest_job_id(cfg: &Config, action: &str) -> Result<Uuid, Box<dyn Error>> {
    let id = cfg
        .positional
        .get(1)
        .ok_or_else(|| format!("reddit {action} requires <job-id>"))?;
    Ok(Uuid::parse_str(id)?)
}

async fn handle_ingest_status(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let id = parse_ingest_job_id(cfg, "status")?;
    match get_ingest_job(cfg, id).await? {
        Some(job) => {
            if cfg.json_output {
                println!("{}", serde_json::to_string_pretty(&job)?);
            } else {
                println!(
                    "{} {}",
                    primary("Ingest Status for"),
                    accent(&job.id.to_string())
                );
                println!(
                    "  {} {}",
                    symbol_for_status(&job.status),
                    status_text(&job.status)
                );
                println!(
                    "  {} {} / {}",
                    muted("Source:"),
                    job.source_type,
                    job.target
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

async fn handle_ingest_cancel(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let id = parse_ingest_job_id(cfg, "cancel")?;
    let canceled = cancel_ingest_job(cfg, id).await?;
    if cfg.json_output {
        println!("{}", serde_json::json!({"id": id, "canceled": canceled}));
    } else if canceled {
        println!(
            "{} canceled ingest job {}",
            symbol_for_status("canceled"),
            accent(&id.to_string())
        );
        println!("Job ID: {id}");
    } else {
        println!(
            "{} no cancellable ingest job found for {}",
            symbol_for_status("error"),
            accent(&id.to_string())
        );
    }
    Ok(())
}

async fn handle_ingest_list(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let jobs = list_ingest_jobs(cfg, 50).await?;
    if cfg.json_output {
        println!("{}", serde_json::to_string_pretty(&jobs)?);
        return Ok(());
    }

    println!("{}", primary("Ingest Jobs"));
    if jobs.is_empty() {
        println!("  {}", muted("No ingest jobs found."));
        return Ok(());
    }
    for job in jobs {
        println!(
            "  {} {} {} {}/{}",
            symbol_for_status(&job.status),
            accent(&job.id.to_string()),
            status_text(&job.status),
            job.source_type,
            job.target
        );
    }
    Ok(())
}

async fn handle_ingest_cleanup(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let removed = cleanup_ingest_jobs(cfg).await?;
    if cfg.json_output {
        println!("{}", serde_json::json!({"removed": removed}));
    } else {
        println!(
            "{} removed {} ingest jobs",
            symbol_for_status("completed"),
            removed
        );
    }
    Ok(())
}

async fn handle_ingest_clear(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if !confirm_destructive(cfg, "Clear all ingest jobs and purge ingest queue?")? {
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
    let removed = clear_ingest_jobs(cfg).await?;
    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({"removed": removed, "queue_purged": true})
        );
    } else {
        println!(
            "{} cleared {} ingest jobs and purged queue",
            symbol_for_status("completed"),
            removed
        );
    }
    Ok(())
}

async fn handle_ingest_recover(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let reclaimed = recover_stale_ingest_jobs(cfg).await?;
    if cfg.json_output {
        println!("{}", serde_json::json!({"reclaimed": reclaimed}));
    } else {
        println!(
            "{} reclaimed {} stale ingest jobs",
            symbol_for_status("completed"),
            reclaimed
        );
    }
    Ok(())
}

async fn enqueue_ingest_job(cfg: &Config, source: IngestSource) -> Result<(), Box<dyn Error>> {
    let job_id = start_ingest_job(cfg, source).await?;
    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({"job_id": job_id, "status": "pending"})
        );
    } else {
        println!(
            "  {} {}",
            primary("Ingest Job"),
            accent(&job_id.to_string())
        );
        println!("  {}", muted("Status: pending"));
        println!("Job ID: {job_id}");
    }
    Ok(())
}

async fn run_ingest_sync(cfg: &Config, source: IngestSource) -> Result<(), Box<dyn Error>> {
    use crate::axon_cli::crates::ingest;

    let IngestSource::Reddit { target } = &source else {
        return enqueue_ingest_job(cfg, source).await;
    };

    let chunks = ingest::reddit::ingest_reddit(cfg, target).await?;
    log_done(&format!("reddit ingest complete: {chunks} chunks embedded"));
    if cfg.json_output {
        println!("{}", serde_json::json!({"chunks_embedded": chunks}));
    } else {
        println!(
            "{} {} chunks embedded from {}",
            symbol_for_status("completed"),
            accent(&chunks.to_string()),
            muted(target)
        );
    }
    Ok(())
}
