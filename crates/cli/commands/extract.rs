use crate::axon_cli::crates::cli::commands::common::parse_urls;
use crate::axon_cli::crates::cli::commands::run_doctor;
use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::logging::log_done;
use crate::axon_cli::crates::core::ui::{
    accent, confirm_destructive, muted, primary, status_text, symbol_for_status,
};
use crate::axon_cli::crates::extract::remote_extract::run_remote_extract;
use crate::axon_cli::crates::jobs::extract_jobs::{
    cancel_extract_job, cleanup_extract_jobs, clear_extract_jobs, get_extract_job,
    list_extract_jobs, run_extract_worker, start_extract_job,
};
use std::error::Error;
use uuid::Uuid;

pub async fn run_extract(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if let Some(subcmd) = cfg.positional.first().map(|s| s.as_str()) {
        match subcmd {
            "status" => {
                let id = cfg
                    .positional
                    .get(1)
                    .ok_or("extract status requires <job-id>")?;
                let id = Uuid::parse_str(id)?;
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
                return Ok(());
            }
            "cancel" => {
                let id = cfg
                    .positional
                    .get(1)
                    .ok_or("extract cancel requires <job-id>")?;
                let id = Uuid::parse_str(id)?;
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
                return Ok(());
            }
            "errors" => {
                let id = cfg
                    .positional
                    .get(1)
                    .ok_or("extract errors requires <job-id>")?;
                let id = Uuid::parse_str(id)?;
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
                return Ok(());
            }
            "list" => {
                let jobs = list_extract_jobs(cfg, 50).await?;
                if cfg.json_output {
                    println!("{}", serde_json::to_string_pretty(&jobs)?);
                } else {
                    println!("{}", primary("Extract Jobs"));
                    if jobs.is_empty() {
                        println!("  {}", muted("No extract jobs found."));
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
                return Ok(());
            }
            "clear" => {
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
                return Ok(());
            }
            "worker" => {
                run_extract_worker(cfg).await?;
                return Ok(());
            }
            "doctor" => {
                eprintln!("{}", muted("`extract doctor` is deprecated; use `doctor`."));
                run_doctor(cfg).await?;
                return Ok(());
            }
            _ => {}
        }
    }

    let urls = parse_urls(cfg);
    if urls.is_empty() {
        return Err("extract requires at least one URL (positional or --urls)".into());
    }
    let prompt = cfg
        .query
        .as_ref()
        .ok_or("extract requires --query <prompt>")?
        .to_string();

    if !cfg.wait {
        let job_id = start_extract_job(cfg, &urls, Some(prompt)).await?;
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
        return Ok(());
    }

    let mut runs = Vec::new();
    let mut all_results = Vec::new();
    let mut pages_visited = 0usize;
    let mut pages_with_data = 0usize;

    for url in &urls {
        let run = run_remote_extract(
            url,
            &prompt,
            cfg.max_pages,
            &cfg.openai_base_url,
            &cfg.openai_api_key,
            &cfg.openai_model,
        )
        .await?;
        pages_visited += run.pages_visited;
        pages_with_data += run.pages_with_data;
        all_results.extend(run.results.clone());
        runs.push(serde_json::json!({
            "url": run.start_url,
            "pages_visited": run.pages_visited,
            "pages_with_data": run.pages_with_data,
            "total_items": run.results.len(),
            "results": run.results
        }));
    }

    let output_path = cfg
        .output_path
        .clone()
        .unwrap_or_else(|| cfg.output_dir.join("extract.json"));
    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let output = serde_json::json!({
        "urls": urls,
        "prompt": prompt,
        "model": cfg.openai_model,
        "pages_visited": pages_visited,
        "pages_with_data": pages_with_data,
        "total_items": all_results.len(),
        "runs": runs,
        "results": all_results
    });
    tokio::fs::write(&output_path, serde_json::to_string_pretty(&output)?).await?;

    if cfg.json_output {
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("{}", primary("Extract Results"));
        println!("  {} {}", muted("Pages visited:"), pages_visited);
        println!("  {} {}", muted("Pages with data:"), pages_with_data);
        println!("  {} {}", muted("Total items:"), output["total_items"]);
        println!("  {} {}", muted("Saved:"), output_path.display());
    }

    log_done("command=extract complete");
    Ok(())
}
