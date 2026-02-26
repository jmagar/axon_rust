use super::common::parse_urls;
use crate::crates::core::config::Config;
use crate::crates::core::content::url_to_domain;
use crate::crates::core::http::validate_url;
use crate::crates::core::ui::{
    accent, confirm_destructive, muted, primary, status_text, symbol_for_status,
};
use crate::crates::crawl::manifest::read_manifest_urls;
use crate::crates::jobs::refresh::{
    cancel_refresh_job, cleanup_refresh_jobs, clear_refresh_jobs, get_refresh_job,
    list_refresh_jobs, recover_stale_refresh_jobs, run_refresh_once, run_refresh_worker,
    start_refresh_job,
};
use std::collections::HashSet;
use std::error::Error;
use std::path::PathBuf;
use uuid::Uuid;

pub async fn run_refresh(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if maybe_handle_refresh_subcommand(cfg).await? {
        return Ok(());
    }

    let urls = resolve_refresh_urls(cfg).await?;
    if urls.is_empty() {
        return Err("refresh requires at least one URL or a crawl manifest seed URL".into());
    }

    if cfg.wait {
        let result = run_refresh_once(cfg, &urls).await?;
        if cfg.json_output {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            let checked = result.get("checked").and_then(|v| v.as_u64()).unwrap_or(0);
            let changed = result.get("changed").and_then(|v| v.as_u64()).unwrap_or(0);
            let unchanged = result
                .get("unchanged")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let failed = result.get("failed").and_then(|v| v.as_u64()).unwrap_or(0);
            println!(
                "{} checked={} changed={} unchanged={} failed={}",
                symbol_for_status("completed"),
                checked,
                changed,
                unchanged,
                failed
            );
            if let Some(path) = result.get("manifest_path").and_then(|v| v.as_str()) {
                println!("  {} {}", muted("Manifest:"), path);
            }
        }
        return Ok(());
    }

    let job_id = start_refresh_job(cfg, &urls).await?;
    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({
                "job_id": job_id,
                "status": "pending",
                "urls": urls,
            })
        );
    } else {
        println!(
            "  {} {}",
            primary("Refresh Job"),
            accent(&job_id.to_string())
        );
        println!("  {} {}", muted("Targets:"), urls.len());
        println!("Job ID: {job_id}");
    }
    Ok(())
}

async fn maybe_handle_refresh_subcommand(cfg: &Config) -> Result<bool, Box<dyn Error>> {
    let Some(subcmd) = cfg.positional.first().map(|s| s.as_str()) else {
        return Ok(false);
    };

    match subcmd {
        "status" => handle_refresh_status(cfg).await?,
        "cancel" => handle_refresh_cancel(cfg).await?,
        "errors" => handle_refresh_errors(cfg).await?,
        "list" => handle_refresh_list(cfg).await?,
        "cleanup" => handle_refresh_cleanup(cfg).await?,
        "clear" => handle_refresh_clear(cfg).await?,
        "worker" => run_refresh_worker(cfg).await?,
        "recover" => handle_refresh_recover(cfg).await?,
        _ => return Ok(false),
    }

    Ok(true)
}

fn parse_refresh_job_id(cfg: &Config, action: &str) -> Result<Uuid, Box<dyn Error>> {
    let id = cfg
        .positional
        .get(1)
        .ok_or_else(|| format!("refresh {action} requires <job-id>"))?;
    Ok(Uuid::parse_str(id)?)
}

async fn handle_refresh_status(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let id = parse_refresh_job_id(cfg, "status")?;
    match get_refresh_job(cfg, id).await? {
        Some(job) => {
            if cfg.json_output {
                println!("{}", serde_json::to_string_pretty(&job)?);
            } else {
                println!(
                    "{} {}",
                    primary("Refresh Status for"),
                    accent(&job.id.to_string())
                );
                println!(
                    "  {} {}",
                    symbol_for_status(&job.status),
                    status_text(&job.status)
                );
                if let Some(result) = job.result_json.as_ref() {
                    let checked = result.get("checked").and_then(|v| v.as_u64()).unwrap_or(0);
                    let changed = result.get("changed").and_then(|v| v.as_u64()).unwrap_or(0);
                    let unchanged = result
                        .get("unchanged")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let failed = result.get("failed").and_then(|v| v.as_u64()).unwrap_or(0);
                    println!(
                        "  {} checked={} changed={} unchanged={} failed={}",
                        muted("Progress:"),
                        checked,
                        changed,
                        unchanged,
                        failed
                    );
                }
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

async fn handle_refresh_cancel(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let id = parse_refresh_job_id(cfg, "cancel")?;
    let canceled = cancel_refresh_job(cfg, id).await?;
    if cfg.json_output {
        println!("{}", serde_json::json!({"id": id, "canceled": canceled}));
    } else if canceled {
        println!(
            "{} canceled refresh job {}",
            symbol_for_status("canceled"),
            accent(&id.to_string())
        );
        println!("Job ID: {id}");
    } else {
        println!(
            "{} no cancellable refresh job found for {}",
            symbol_for_status("error"),
            accent(&id.to_string())
        );
        println!("Job ID: {id}");
    }
    Ok(())
}

async fn handle_refresh_errors(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let id = parse_refresh_job_id(cfg, "errors")?;
    match get_refresh_job(cfg, id).await? {
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

async fn handle_refresh_list(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let jobs = list_refresh_jobs(cfg, 50).await?;
    if cfg.json_output {
        println!("{}", serde_json::to_string_pretty(&jobs)?);
        return Ok(());
    }

    println!("{}", primary("Refresh Jobs"));
    if jobs.is_empty() {
        println!("  {}", muted("No refresh jobs found."));
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

async fn handle_refresh_cleanup(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let removed = cleanup_refresh_jobs(cfg).await?;
    if cfg.json_output {
        println!("{}", serde_json::json!({"removed": removed}));
    } else {
        println!(
            "{} removed {} refresh jobs",
            symbol_for_status("completed"),
            removed
        );
    }
    Ok(())
}

async fn handle_refresh_clear(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if !confirm_destructive(cfg, "Clear all refresh jobs and purge refresh queue?")? {
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

    let removed = clear_refresh_jobs(cfg).await?;
    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({"removed": removed, "queue_purged": true})
        );
    } else {
        println!(
            "{} cleared {} refresh jobs and purged queue",
            symbol_for_status("completed"),
            removed
        );
    }
    Ok(())
}

async fn handle_refresh_recover(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let reclaimed = recover_stale_refresh_jobs(cfg).await?;
    if cfg.json_output {
        println!("{}", serde_json::json!({"reclaimed": reclaimed}));
    } else {
        println!(
            "{} reclaimed {} stale refresh jobs",
            symbol_for_status("completed"),
            reclaimed
        );
    }
    Ok(())
}

fn manifest_candidate_paths(cfg: &Config, seed_url: &str) -> Vec<PathBuf> {
    let domain = url_to_domain(seed_url);
    let base = cfg.output_dir.join("domains").join(domain);
    vec![
        base.join("latest").join("manifest.jsonl"),
        base.join("sync").join("manifest.jsonl"),
    ]
}

async fn urls_from_manifest_seed(
    cfg: &Config,
    seed_url: &str,
) -> Result<Vec<String>, Box<dyn Error>> {
    for path in manifest_candidate_paths(cfg, seed_url) {
        if !path.exists() {
            continue;
        }
        let urls = read_manifest_urls(&path).await?;
        if !urls.is_empty() {
            let mut sorted: Vec<String> = urls.into_iter().collect();
            sorted.sort();
            return Ok(sorted);
        }
    }
    Ok(Vec::new())
}

fn looks_like_domain_seed(url: &str) -> bool {
    let Ok(parsed) = spider::url::Url::parse(url) else {
        return false;
    };
    parsed.path() == "/" && parsed.query().is_none() && parsed.fragment().is_none()
}

async fn resolve_refresh_urls(cfg: &Config) -> Result<Vec<String>, Box<dyn Error>> {
    let mut urls = parse_urls(cfg);

    if urls.is_empty() && !cfg.start_url.trim().is_empty() {
        let seeded = urls_from_manifest_seed(cfg, &cfg.start_url).await?;
        if !seeded.is_empty() {
            urls = seeded;
        }
    } else if urls.len() == 1 && looks_like_domain_seed(&urls[0]) {
        let seeded = urls_from_manifest_seed(cfg, &urls[0]).await?;
        if !seeded.is_empty() {
            urls = seeded;
        }
    }

    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for url in urls {
        validate_url(&url)?;
        if seen.insert(url.clone()) {
            deduped.push(url);
        }
    }

    Ok(deduped)
}
