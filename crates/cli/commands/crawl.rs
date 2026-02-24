mod audit;
mod runtime;
mod sync_crawl;

pub(crate) use audit::discover_sitemap_urls_with_robots;

use super::common::parse_urls;
use crate::crates::core::config::Config;
use crate::crates::core::http::validate_url;
use crate::crates::core::ui::{
    accent, confirm_destructive, muted, primary, print_kv, print_option, print_phase, status_text,
    symbol_for_status,
};
use crate::crates::jobs::crawl::{
    cancel_job, cleanup_jobs, clear_jobs, get_job, list_jobs, recover_stale_crawl_jobs, run_worker,
    start_crawl_jobs_batch, CrawlJob,
};
use std::error::Error;
use uuid::Uuid;

pub async fn run_crawl(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if maybe_handle_subcommand(cfg).await? {
        return Ok(());
    }
    let urls = parse_urls(cfg);
    if urls.is_empty() {
        return Err("crawl requires at least one URL (positional or --urls)".into());
    }
    for url in &urls {
        validate_url(url)?;
    }
    if cfg.wait {
        for url in &urls {
            sync_crawl::run_sync_crawl(cfg, url).await?;
        }
        Ok(())
    } else {
        run_async_enqueue_multi(cfg, &urls).await
    }
}

async fn maybe_handle_subcommand(cfg: &Config) -> Result<bool, Box<dyn Error>> {
    let Some(subcmd) = cfg.positional.first().map(|s| s.as_str()) else {
        return Ok(false);
    };
    match subcmd {
        "status" => handle_status_subcommand(cfg).await?,
        "cancel" => handle_cancel_subcommand(cfg).await?,
        "errors" => handle_errors_subcommand(cfg).await?,
        "list" => handle_list_subcommand(cfg).await?,
        "cleanup" => handle_cleanup_subcommand(cfg).await?,
        "clear" => handle_clear_subcommand(cfg).await?,
        "worker" => run_worker(cfg).await?,
        "recover" => handle_recover_subcommand(cfg).await?,
        "audit" => {
            let url = cfg.positional.get(1).map(|s| s.as_str()).unwrap_or("");
            audit::run_crawl_audit(cfg, url).await?;
        }
        "diff" => audit::run_crawl_audit_diff(cfg).await?,
        _ => return Ok(false),
    }
    Ok(true)
}

fn parse_required_job_id(cfg: &Config, action: &str) -> Result<Uuid, Box<dyn Error>> {
    let id = cfg
        .positional
        .get(1)
        .ok_or_else(|| format!("crawl {action} requires <job-id>"))?;
    Ok(Uuid::parse_str(id)?)
}

fn print_status_metrics(metrics: &serde_json::Value) {
    let md_created = metrics
        .get("md_created")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
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
    let sitemap_written = metrics
        .get("sitemap_written")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let sitemap_candidates = metrics
        .get("sitemap_candidates")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let pages_target = pages_discovered.saturating_sub(filtered_urls);
    let thin_md = metrics.get("thin_md").and_then(|v| v.as_u64()).unwrap_or(0);
    let thin_pct = if pages_discovered > 0 {
        (thin_md as f64 / pages_discovered as f64) * 100.0
    } else {
        0.0
    };
    println!("  {} {}", muted("md created:"), md_created);
    println!("  {} {}", muted("pages target:"), pages_target);
    println!("  {} {:.1}%", muted("thin % of discovered:"), thin_pct);
    println!("  {} {}", muted("filtered urls:"), filtered_urls);
    println!("  {} {}", muted("pages crawled:"), pages_crawled);
    println!("  {} {}", muted("pages discovered:"), pages_discovered);
    if sitemap_candidates > 0 || sitemap_written > 0 {
        println!(
            "  {} {}/{}",
            muted("sitemap written/candidates:"),
            sitemap_written,
            sitemap_candidates
        );
    }
}

fn print_job_not_found(id: Uuid) {
    println!(
        "{} {}",
        symbol_for_status("error"),
        muted(&format!("job not found: {id}"))
    );
}

async fn handle_status_subcommand(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let id = parse_required_job_id(cfg, "status")?;
    match get_job(cfg, id).await? {
        Some(job) if cfg.json_output => {
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
        }
        Some(job) => {
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
                print_status_metrics(metrics);
            }
            println!();
            println!("Job ID: {}", job.id);
        }
        None => print_job_not_found(id),
    }
    Ok(())
}

async fn handle_cancel_subcommand(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let id = parse_required_job_id(cfg, "cancel")?;
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
            "{} no cancellable crawl job found for ID: {}",
            symbol_for_status("error"),
            accent(&id.to_string())
        );
        println!("Job ID: {id}");
    }
    Ok(())
}

async fn handle_errors_subcommand(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let id = parse_required_job_id(cfg, "errors")?;
    match get_job(cfg, id).await? {
        Some(job) if cfg.json_output => {
            println!(
                "{}",
                serde_json::json!({"id": id, "status": job.status, "error": job.error_text})
            );
        }
        Some(job) => {
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
        None => print_job_not_found(id),
    }
    Ok(())
}

/// Returns a compact inline progress string for a crawl job list row.
///
/// - running:   "127 crawled · 43 docs"
/// - completed: "342 docs · 5.2s"
/// - failed:    first 60 chars of error_text
/// - other:     None
fn job_progress_summary(job: &CrawlJob) -> Option<String> {
    match job.status.as_str() {
        "running" => {
            let metrics = job.result_json.as_ref()?;
            let crawled = metrics
                .get("pages_crawled")
                .and_then(|v: &serde_json::Value| v.as_u64())
                .unwrap_or(0);
            let docs = metrics
                .get("md_created")
                .and_then(|v: &serde_json::Value| v.as_u64())
                .unwrap_or(0);
            if crawled == 0 && docs == 0 {
                return None;
            }
            if docs > 0 {
                Some(format!("{crawled} crawled · {docs} docs"))
            } else {
                Some(format!("{crawled} crawled"))
            }
        }
        "completed" => {
            let metrics = job.result_json.as_ref()?;
            let docs = metrics
                .get("md_created")
                .and_then(|v: &serde_json::Value| v.as_u64())
                .unwrap_or(0);
            let elapsed_ms = metrics
                .get("elapsed_ms")
                .and_then(|v: &serde_json::Value| v.as_u64())
                .unwrap_or(0);
            let time = if elapsed_ms >= 1000 {
                format!("{:.1}s", elapsed_ms as f64 / 1000.0)
            } else {
                format!("{elapsed_ms}ms")
            };
            Some(format!("{docs} docs · {time}"))
        }
        "failed" => {
            let err = job.error_text.as_deref().unwrap_or("unknown error");
            let truncated = if err.len() > 60 {
                format!("{}…", &err[..60])
            } else {
                err.to_string()
            };
            Some(truncated)
        }
        _ => None,
    }
}

async fn handle_list_subcommand(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let jobs = list_jobs(cfg, 50).await?;
    if cfg.json_output {
        println!("{}", serde_json::to_string_pretty(&jobs)?);
    } else {
        println!("{}", primary("Crawl Jobs"));
        if jobs.is_empty() {
            println!("  {}", muted("No crawl jobs found."));
        } else {
            for job in &jobs {
                let progress = job_progress_summary(job);
                if let Some(p) = progress {
                    println!(
                        "  {} {} {} {}  {}",
                        symbol_for_status(&job.status),
                        accent(&job.id.to_string()),
                        status_text(&job.status),
                        muted(&job.url),
                        muted(&p),
                    );
                } else {
                    println!(
                        "  {} {} {} {}",
                        symbol_for_status(&job.status),
                        accent(&job.id.to_string()),
                        status_text(&job.status),
                        muted(&job.url),
                    );
                }
            }
        }
    }
    Ok(())
}

async fn handle_cleanup_subcommand(cfg: &Config) -> Result<(), Box<dyn Error>> {
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
    Ok(())
}

async fn handle_clear_subcommand(cfg: &Config) -> Result<(), Box<dyn Error>> {
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
    Ok(())
}

async fn handle_recover_subcommand(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let reclaimed = recover_stale_crawl_jobs(cfg).await?;
    if cfg.json_output {
        println!("{}", serde_json::json!({"reclaimed": reclaimed}));
    } else {
        println!(
            "{} reclaimed {} stale crawl jobs",
            symbol_for_status("completed"),
            reclaimed
        );
    }
    Ok(())
}

fn print_async_options(cfg: &Config, start_url: &str) {
    print_phase("◐", "Crawling", start_url);
    println!("  {}", primary("Options:"));
    // Crawl scope
    print_option(
        "maxPages",
        &if cfg.max_pages == 0 {
            "uncapped".to_string()
        } else {
            cfg.max_pages.to_string()
        },
    );
    print_option("maxDepth", &cfg.max_depth.to_string());
    print_option("allowSubdomains", &cfg.include_subdomains.to_string());
    print_option("respectRobotsTxt", &cfg.respect_robots.to_string());
    print_option("discoverSitemaps", &cfg.discover_sitemaps.to_string());
    // Content filtering
    print_option("blockAssets", &cfg.block_assets.to_string());
    print_option(
        "redirectPolicyStrict",
        &cfg.redirect_policy_strict.to_string(),
    );
    print_option(
        "maxPageBytes",
        &cfg.max_page_bytes
            .map(|n| n.to_string())
            .unwrap_or_else(|| "none".to_string()),
    );
    print_option("minMarkdownChars", &cfg.min_markdown_chars.to_string());
    print_option("dropThinMarkdown", &cfg.drop_thin_markdown.to_string());
    if !cfg.url_whitelist.is_empty() {
        print_option("urlWhitelist", &cfg.url_whitelist.join(", "));
    }
    // Render / Chrome
    print_option("renderMode", &cfg.render_mode.to_string());
    print_option("cache", &cfg.cache.to_string());
    print_option("cacheSkipBrowser", &cfg.cache_skip_browser.to_string());
    print_option(
        "chromeRemote",
        cfg.chrome_remote_url.as_deref().unwrap_or("auto/local"),
    );
    print_option("chromeProxy", cfg.chrome_proxy.as_deref().unwrap_or("none"));
    print_option(
        "chromeUserAgent",
        cfg.chrome_user_agent.as_deref().unwrap_or("spider-default"),
    );
    print_option("chromeHeadless", &cfg.chrome_headless.to_string());
    print_option("chromeAntiBot", &cfg.chrome_anti_bot.to_string());
    print_option("chromeStealth", &cfg.chrome_stealth.to_string());
    print_option("chromeIntercept", &cfg.chrome_intercept.to_string());
    print_option("chromeBootstrap", &cfg.chrome_bootstrap.to_string());
    print_option(
        "chromeNetworkIdleTimeoutSecs",
        &cfg.chrome_network_idle_timeout_secs.to_string(),
    );
    print_option(
        "chromeWaitForSelector",
        cfg.chrome_wait_for_selector.as_deref().unwrap_or("none"),
    );
    print_option("chromeScreenshot", &cfg.chrome_screenshot.to_string());
    print_option("bypassCsp", &cfg.bypass_csp.to_string());
    print_option("acceptInvalidCerts", &cfg.accept_invalid_certs.to_string());
    // WebDriver fallback
    print_option(
        "webdriverFallbackUrl",
        cfg.webdriver_url.as_deref().unwrap_or("none"),
    );
    // Output
    print_option("embed", &cfg.embed.to_string());
    print_option("wait", &cfg.wait.to_string());
}

async fn run_async_enqueue_multi(cfg: &Config, urls: &[String]) -> Result<(), Box<dyn Error>> {
    // Chrome bootstrap probe belongs to sync crawl — the worker owns Chrome in async mode.
    // Skipping it here eliminates ~10s of failed probe retries on startup.
    let display = match urls {
        [single] => single.clone(),
        _ => format!("{} (+{} more)", urls[0], urls.len() - 1),
    };
    print_async_options(cfg, &display);
    println!();

    let url_refs: Vec<&str> = urls.iter().map(String::as_str).collect();
    let jobs = start_crawl_jobs_batch(cfg, &url_refs).await?;
    println!(
        "  {}",
        muted(
            "Async enqueue mode skips sitemap preflight; worker performs discovery during crawl."
        )
    );
    if cfg.embed {
        println!(
            "  {}",
            muted("Embedding job will be queued automatically after crawl completion.")
        );
    }
    for (url, job_id) in &jobs {
        if cfg.json_output {
            println!(
                "{}",
                serde_json::json!({"url": url, "job_id": job_id, "status": "pending"})
            );
        } else {
            println!(
                "  {} {} → {}",
                primary("Crawl Job"),
                accent(&job_id.to_string()),
                muted(url)
            );
            println!(
                "  {}",
                muted(&format!("Check status: axon crawl status {job_id}"))
            );
        }
    }
    println!();
    if let Some((_, first_id)) = jobs.first() {
        println!("Job ID: {first_id}");
    }
    Ok(())
}
