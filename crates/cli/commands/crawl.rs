use crate::axon_cli::crates::cli::commands::run_doctor;
use crate::axon_cli::crates::core::config::{Config, RenderMode};
use crate::axon_cli::crates::core::http::validate_url;
use crate::axon_cli::crates::core::logging::log_done;
use crate::axon_cli::crates::core::ui::{
    accent, confirm_destructive, muted, primary, print_kv, print_option, print_phase, status_text,
    symbol_for_status, Spinner,
};
use crate::axon_cli::crates::crawl::engine::{
    append_sitemap_backfill, crawl_sitemap_urls, run_crawl_once,
};
use crate::axon_cli::crates::jobs::crawl_jobs::{
    cancel_job, cleanup_jobs, clear_jobs, get_job, list_jobs, run_worker, start_crawl_job,
};
use crate::axon_cli::crates::jobs::embed_jobs::start_embed_job;
use std::error::Error;
use uuid::Uuid;

pub async fn run_crawl(cfg: &Config, start_url: &str) -> Result<(), Box<dyn Error>> {
    if let Some(subcmd) = cfg.positional.first().map(|s| s.as_str()) {
        match subcmd {
            "status" => {
                let id = cfg
                    .positional
                    .get(1)
                    .ok_or("crawl status requires <job-id>")?;
                let id = Uuid::parse_str(id)?;
                match get_job(cfg, id).await? {
                    Some(job) => {
                        if cfg.json_output {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&serde_json::json!({
                                        "id": job.id,
                                        "url": job.url,
                                        "status": job.status,
                                        "created_at": job.created_at,
                                    "updated_at": job.updated_at,
                                    "started_at": job.started_at,
                                    "finished_at": job.finished_at,
                                    "error": job.error_text,
                                    "metrics": job.result_json,
                                }))?
                            );
                        } else {
                            print_kv("Crawl Status for", &job.id.to_string());
                            println!(
                                "  {} {}",
                                symbol_for_status(&job.status),
                                status_text(&job.status)
                            );
                            println!("  {} {}", muted("URL:"), job.url);
                            println!("  {} {}", muted("Created:"), job.created_at);
                            println!("  {} {}", muted("Updated:"), job.updated_at);
                            if let Some(err) = job.error_text.as_deref() {
                                println!("  {} {}", muted("Error:"), err);
                            }
                            if let Some(metrics) = job.result_json.as_ref() {
                                let md_created = metrics
                                    .get("md_created")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let thin_md =
                                    metrics.get("thin_md").and_then(|v| v.as_u64()).unwrap_or(0);
                                let filtered_urls = metrics
                                    .get("filtered_urls")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let pages_crawled = metrics
                                    .get("pages_crawled")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let pages_discovered = metrics
                                    .get("pages_discovered")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                println!("  {} {}", muted("md created:"), md_created);
                                println!("  {} {}", muted("thin md:"), thin_md);
                                println!("  {} {}", muted("filtered urls:"), filtered_urls);
                                println!("  {} {}", muted("pages crawled:"), pages_crawled);
                                println!("  {} {}", muted("pages discovered:"), pages_discovered);
                            }
                            println!();
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
                    .ok_or("crawl cancel requires <job-id>")?;
                let id = Uuid::parse_str(id)?;
                let canceled = cancel_job(cfg, id).await?;
                if cfg.json_output {
                    println!(
                        "{}",
                        serde_json::json!({"id": id, "canceled": canceled, "source": "rust"})
                    );
                } else if canceled {
                    println!(
                        "{} canceled crawl job {}",
                        symbol_for_status("canceled"),
                        accent(&id.to_string())
                    );
                    println!("Job ID: {id}");
                } else {
                    println!(
                        "{} no cancellable crawl job found for {}",
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
                    .ok_or("crawl errors requires <job-id>")?;
                let id = Uuid::parse_str(id)?;
                match get_job(cfg, id).await? {
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
                let jobs = list_jobs(cfg, 50).await?;
                if cfg.json_output {
                    println!("{}", serde_json::to_string_pretty(&jobs)?);
                } else {
                    println!("{}", primary("Crawl Jobs"));
                    if jobs.is_empty() {
                        println!("  {}", muted("No crawl jobs found."));
                    } else {
                        for job in jobs {
                            println!(
                                "  {} {} {} {}",
                                symbol_for_status(&job.status),
                                accent(&job.id.to_string()),
                                status_text(&job.status),
                                muted(&job.url)
                            );
                        }
                    }
                }
                return Ok(());
            }
            "cleanup" => {
                let removed = cleanup_jobs(cfg).await?;
                if cfg.json_output {
                    println!("{}", serde_json::json!({"removed": removed}));
                } else {
                    println!(
                        "{} removed {} crawl jobs",
                        symbol_for_status("completed"),
                        removed
                    );
                }
                return Ok(());
            }
            "clear" => {
                if !confirm_destructive(cfg, "Clear all crawl jobs and purge crawl queue?")? {
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
                let removed = clear_jobs(cfg).await?;
                if cfg.json_output {
                    println!(
                        "{}",
                        serde_json::json!({"removed": removed, "queue_purged": true})
                    );
                } else {
                    println!(
                        "{} cleared {} crawl jobs and purged queue",
                        symbol_for_status("completed"),
                        removed
                    );
                }
                return Ok(());
            }
            "worker" => {
                run_worker(cfg).await?;
                return Ok(());
            }
            "doctor" => {
                eprintln!("{}", muted("`crawl doctor` is deprecated; use `doctor`."));
                run_doctor(cfg).await?;
                return Ok(());
            }
            _ => {}
        }
    }

    validate_url(start_url)?;

    if !cfg.wait {
        let job_id = start_crawl_job(cfg, start_url).await?;

        print_phase("◐", "Crawling", start_url);
        println!("  {}", primary("Options:"));
        print_option("maxDepth", &cfg.max_depth.to_string());
        print_option("allowSubdomains", &cfg.include_subdomains.to_string());
        print_option("respectRobotsTxt", &cfg.respect_robots.to_string());
        print_option("renderMode", &format!("{:?}", cfg.render_mode));
        print_option("discoverSitemaps", &cfg.discover_sitemaps.to_string());
        print_option("embed", &cfg.embed.to_string());
        print_option("wait", &cfg.wait.to_string());
        println!();

        match crawl_sitemap_urls(cfg, start_url).await {
            Ok(urls) => println!(
                "{} Preflight map found {} URLs.",
                muted("[Guardrail]"),
                urls.len()
            ),
            Err(err) => println!("{} Preflight map failed: {err}", muted("[Guardrail]")),
        }
        println!();
        if cfg.embed {
            println!(
                "  {}",
                muted("Embedding job will be queued automatically after crawl completion.")
            );
        }
        println!("  {} {}", primary("Crawl Job"), accent(&job_id.to_string()));
        println!(
            "  {}",
            muted(&format!("Check status: cortex crawl status {job_id}"))
        );
        println!();
        println!("Job ID: {job_id}");

        return Ok(());
    }

    let initial_mode = match cfg.render_mode {
        RenderMode::AutoSwitch => RenderMode::Http,
        m => m,
    };

    let spinner = Spinner::new("running crawl");
    let (summary, seen_urls) =
        run_crawl_once(cfg, start_url, initial_mode, &cfg.output_dir, None).await?;
    spinner.finish(&format!(
        "crawl phase complete (pages={}, markdown={})",
        summary.pages_seen, summary.markdown_files
    ));
    let mut final_summary = summary;

    if cfg.discover_sitemaps {
        let spinner = Spinner::new("running sitemap backfill");
        let _ = append_sitemap_backfill(
            cfg,
            start_url,
            &cfg.output_dir,
            &seen_urls,
            &mut final_summary,
        )
        .await?;
        spinner.finish("sitemap backfill complete");
    }

    if cfg.embed {
        let markdown_dir = cfg.output_dir.join("markdown");
        let embed_job_id = start_embed_job(cfg, &markdown_dir.to_string_lossy()).await?;
        println!(
            "{} {}",
            muted("Queued embed job:"),
            accent(&embed_job_id.to_string())
        );
    }

    log_done(&format!(
        "command=crawl pages_seen={} markdown_files={} thin_pages={} elapsed_ms={} output_dir={}",
        final_summary.pages_seen,
        final_summary.markdown_files,
        final_summary.thin_pages,
        final_summary.elapsed_ms,
        cfg.output_dir.to_string_lossy()
    ));

    Ok(())
}
