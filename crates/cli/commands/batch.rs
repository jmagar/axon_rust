use crate::axon_cli::crates::cli::commands::common::parse_urls;
use crate::axon_cli::crates::cli::commands::run_doctor;
use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::content::{to_markdown, url_to_filename};
use crate::axon_cli::crates::core::http::{build_client, fetch_html};
use crate::axon_cli::crates::core::logging::{log_done, log_warn};
use crate::axon_cli::crates::core::ui::{
    accent, confirm_destructive, muted, primary, status_text, symbol_for_status,
};
use crate::axon_cli::crates::jobs::batch_jobs::{
    cancel_batch_job, cleanup_batch_jobs, clear_batch_jobs, get_batch_job, list_batch_jobs,
    run_batch_worker, start_batch_job,
};
use crate::axon_cli::crates::vector::ops::embed_path_native;
use indicatif::{ProgressBar, ProgressStyle};
use spider::tokio;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

pub async fn run_batch(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if let Some(subcmd) = cfg.positional.first().map(|s| s.as_str()) {
        match subcmd {
            "status" => {
                let id = cfg
                    .positional
                    .get(1)
                    .ok_or("batch status requires <job-id>")?;
                let id = Uuid::parse_str(id)?;
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
                            println!("Job ID: {}", job.id);
                        }
                    }
                    None => println!(
                        "{} {}",
                        symbol_for_status("error"),
                        muted(&format!("job not found: {id}"))
                    ),
                }
                return Ok(());
            }
            "cancel" => {
                let id = cfg
                    .positional
                    .get(1)
                    .ok_or("batch cancel requires <job-id>")?;
                let id = Uuid::parse_str(id)?;
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
                return Ok(());
            }
            "errors" => {
                let id = cfg
                    .positional
                    .get(1)
                    .ok_or("batch errors requires <job-id>")?;
                let id = Uuid::parse_str(id)?;
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
                return Ok(());
            }
            "list" => {
                let jobs = list_batch_jobs(cfg, 50).await?;
                if cfg.json_output {
                    println!("{}", serde_json::to_string_pretty(&jobs)?);
                } else {
                    println!("{}", primary("Batch Jobs"));
                    if jobs.is_empty() {
                        println!("  {}", muted("No batch jobs found."));
                    } else {
                        for job in jobs {
                            println!(
                                "  {} {} {}",
                                symbol_for_status(&job.status),
                                accent(&job.id.to_string()),
                                status_text(&job.status)
                            );
                        }
                    }
                }
                return Ok(());
            }
            "cleanup" => {
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
                return Ok(());
            }
            "clear" => {
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
                return Ok(());
            }
            "worker" => {
                run_batch_worker(cfg).await?;
                return Ok(());
            }
            "doctor" => {
                eprintln!("{}", muted("`batch doctor` is deprecated; use `doctor`."));
                run_doctor(cfg).await?;
                return Ok(());
            }
            _ => {}
        }
    }

    let urls = parse_urls(cfg);
    if urls.is_empty() {
        return Err("batch requires at least one URL (positional or --urls)".into());
    }

    if !cfg.wait {
        let job_id = start_batch_job(cfg, &urls).await?;
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
        return Ok(());
    }

    let batch_dir = cfg.output_dir.join("batch-markdown");
    if batch_dir.exists() {
        if std::env::var("AXON_NO_WIPE").is_ok() {
            log_warn(&format!(
                "AXON_NO_WIPE set — keeping existing batch dir: {}",
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

    let client = build_client(20)?;
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

    if cfg.embed {
        embed_path_native(cfg, &batch_dir.to_string_lossy()).await?;
    }

    log_done("command=batch complete");
    Ok(())
}
