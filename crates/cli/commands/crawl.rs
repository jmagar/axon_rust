use crate::axon_cli::crates::cli::commands::run_doctor;
use crate::axon_cli::crates::core::config::{Config, RenderMode};
use crate::axon_cli::crates::core::content::{
    canonicalize_url, extract_loc_values, extract_robots_sitemaps, is_excluded_url_path,
    to_markdown, url_to_filename,
};
use crate::axon_cli::crates::core::http::validate_url;
use crate::axon_cli::crates::core::logging::log_done;
use crate::axon_cli::crates::core::ui::{
    accent, confirm_destructive, muted, primary, print_kv, print_option, print_phase, status_text,
    symbol_for_status, Spinner,
};
use crate::axon_cli::crates::crawl::engine::{append_sitemap_backfill, run_crawl_once};
use crate::axon_cli::crates::jobs::crawl_jobs_dispatch::{
    cancel_job, cleanup_jobs, clear_jobs, get_job, list_jobs, recover_stale_crawl_jobs, run_worker,
    start_crawl_job,
};
use crate::axon_cli::crates::jobs::embed_jobs::start_embed_job;
use serde::{Deserialize, Serialize};
use spider::url::Url;
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use uuid::Uuid;

#[derive(Debug, Serialize)]
struct CrawlAuditDiff {
    generated_at_epoch_ms: u128,
    start_url: String,
    previous_count: usize,
    current_count: usize,
    added_count: usize,
    removed_count: usize,
    unchanged_count: usize,
    cache_hit: bool,
    cache_source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChromeRuntimeMode {
    Chrome,
    WebDriverFallback,
}

#[derive(Debug, Clone)]
struct ChromeBootstrapOutcome {
    mode: ChromeRuntimeMode,
    probe_url: Option<String>,
    remote_ready: bool,
    warnings: Vec<String>,
}

fn chrome_runtime_requested(cfg: &Config) -> bool {
    !cfg.cache_skip_browser
        && matches!(cfg.render_mode, RenderMode::Chrome | RenderMode::AutoSwitch)
}

fn to_devtools_probe_url(remote_url: &str) -> Option<String> {
    let parsed = Url::parse(remote_url).ok()?;
    let host = parsed.host_str()?;
    let port = parsed.port_or_known_default()?;
    let scheme = match parsed.scheme() {
        "ws" => "http",
        "wss" => "https",
        "http" | "https" => parsed.scheme(),
        _ => return None,
    };
    Some(format!("{scheme}://{host}:{port}/json/version"))
}

async fn remote_chrome_ready(probe_url: &str, timeout_ms: u64) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms.max(250)))
        .build()
    {
        Ok(client) => client,
        Err(_) => return false,
    };
    match client.get(probe_url).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

async fn bootstrap_chrome_runtime(cfg: &Config) -> ChromeBootstrapOutcome {
    let mut outcome = ChromeBootstrapOutcome {
        mode: ChromeRuntimeMode::Chrome,
        probe_url: None,
        remote_ready: false,
        warnings: Vec::new(),
    };

    if !chrome_runtime_requested(cfg) {
        return outcome;
    }
    if !cfg.chrome_bootstrap {
        return outcome;
    }

    let Some(remote_url) = cfg.chrome_remote_url.as_deref() else {
        outcome.warnings.push(
            "no --chrome-remote-url provided; using Spider local Chrome launcher".to_string(),
        );
        return outcome;
    };

    let Some(probe_url) = to_devtools_probe_url(remote_url) else {
        outcome.warnings.push(format!(
            "unable to parse --chrome-remote-url `{remote_url}`; proceeding with local launcher"
        ));
        return outcome;
    };
    outcome.probe_url = Some(probe_url.clone());

    for attempt in 0..=cfg.chrome_bootstrap_retries {
        if remote_chrome_ready(&probe_url, cfg.chrome_bootstrap_timeout_ms).await {
            outcome.remote_ready = true;
            return outcome;
        }
        if attempt < cfg.chrome_bootstrap_retries {
            tokio::time::sleep(Duration::from_millis(200 * (attempt as u64 + 1))).await;
        }
    }

    if cfg.webdriver_url.is_some() {
        outcome.mode = ChromeRuntimeMode::WebDriverFallback;
        outcome.warnings.push(
            "remote chrome probe failed; WebDriver fallback selected for engine handoff"
                .to_string(),
        );
    } else {
        outcome
            .warnings
            .push("remote chrome probe failed; falling back to local Chrome launcher".to_string());
    }

    outcome
}

fn resolve_initial_mode(cfg: &Config) -> RenderMode {
    if cfg.cache_skip_browser {
        return RenderMode::Http;
    }
    match cfg.render_mode {
        RenderMode::AutoSwitch => RenderMode::Http,
        m => m,
    }
}

async fn read_manifest_urls(path: &Path) -> Result<HashSet<String>, Box<dyn Error>> {
    if !path.exists() {
        return Ok(HashSet::new());
    }
    let content = tokio::fs::read_to_string(path).await?;
    let mut out = HashSet::new();
    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(url) = json.get("url").and_then(|v| v.as_str()) else {
            continue;
        };
        out.insert(url.to_string());
    }
    Ok(out)
}

async fn write_audit_diff(
    output_dir: &Path,
    start_url: &str,
    previous: &HashSet<String>,
    current: &HashSet<String>,
    cache_hit: bool,
    cache_source: Option<String>,
) -> Result<PathBuf, Box<dyn Error>> {
    let now = now_epoch_ms();
    let unchanged_count = previous.intersection(current).count();
    let added_count = current.difference(previous).count();
    let removed_count = previous.difference(current).count();
    let report = CrawlAuditDiff {
        generated_at_epoch_ms: now,
        start_url: start_url.to_string(),
        previous_count: previous.len(),
        current_count: current.len(),
        added_count,
        removed_count,
        unchanged_count,
        cache_hit,
        cache_source,
    };

    let audit_dir = output_dir.join("reports").join("crawl-diff");
    tokio::fs::create_dir_all(&audit_dir).await?;
    let report_path = audit_dir.join(format!("diff-report-{now}.json"));
    let payload = serde_json::to_string_pretty(&report)?;
    tokio::fs::write(&report_path, payload).await?;
    Ok(report_path)
}

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
                                let thin_md =
                                    metrics.get("thin_md").and_then(|v| v.as_u64()).unwrap_or(0);
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
            "recover" => {
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
                return Ok(());
            }
            "doctor" => {
                eprintln!("{}", muted("`crawl doctor` is deprecated; use `doctor`."));
                run_doctor(cfg).await?;
                return Ok(());
            }
            "audit" => {
                run_crawl_audit(cfg, start_url).await?;
                return Ok(());
            }
            "diff" => {
                run_crawl_audit_diff(cfg).await?;
                return Ok(());
            }
            _ => {}
        }
    }

    validate_url(start_url)?;

    if !cfg.wait {
        let chrome_bootstrap = bootstrap_chrome_runtime(cfg).await;
        let job_id = start_crawl_job(cfg, start_url).await?;

        print_phase("◐", "Crawling", start_url);
        println!("  {}", primary("Options:"));
        print_option("maxDepth", &cfg.max_depth.to_string());
        print_option("allowSubdomains", &cfg.include_subdomains.to_string());
        print_option("respectRobotsTxt", &cfg.respect_robots.to_string());
        print_option("renderMode", &format!("{:?}", cfg.render_mode));
        print_option("discoverSitemaps", &cfg.discover_sitemaps.to_string());
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
            "webdriverFallbackUrl",
            cfg.webdriver_url.as_deref().unwrap_or("none"),
        );
        print_option("embed", &cfg.embed.to_string());
        print_option("wait", &cfg.wait.to_string());
        if chrome_runtime_requested(cfg) {
            print_option(
                "chromeBootstrapReady",
                &chrome_bootstrap.remote_ready.to_string(),
            );
            print_option(
                "chromeRuntimeMode",
                match chrome_bootstrap.mode {
                    ChromeRuntimeMode::Chrome => "chrome",
                    ChromeRuntimeMode::WebDriverFallback => "webdriver-fallback",
                },
            );
        }
        println!();
        for warning in chrome_bootstrap.warnings {
            println!("{} {}", muted("[Chrome Bootstrap]"), warning);
        }

        println!(
            "  {}",
            muted("Async enqueue mode skips sitemap preflight; worker performs discovery during crawl.")
        );
        if cfg.embed {
            println!(
                "  {}",
                muted("Embedding job will be queued automatically after crawl completion.")
            );
        }
        println!("  {} {}", primary("Crawl Job"), accent(&job_id.to_string()));
        println!(
            "  {}",
            muted(&format!("Check status: axon crawl status {job_id}"))
        );
        println!();
        println!("Job ID: {job_id}");

        return Ok(());
    }

    let manifest_path = cfg.output_dir.join("manifest.jsonl");
    let previous_urls = if cfg.cache {
        read_manifest_urls(&manifest_path).await?
    } else {
        HashSet::new()
    };
    let cache_ttl_secs: u64 = 24 * 60 * 60;
    let cache_stale = manifest_path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|mtime| SystemTime::now().duration_since(mtime).ok())
        .is_some_and(|age| age.as_secs() > cache_ttl_secs);
    if cfg.cache && !previous_urls.is_empty() && !cache_stale {
        let report_path = write_audit_diff(
            &cfg.output_dir,
            start_url,
            &previous_urls,
            &previous_urls,
            true,
            Some(manifest_path.to_string_lossy().to_string()),
        )
        .await?;
        log_done(&format!(
            "command=crawl cache_hit=true cached_urls={} output_dir={} audit_report={}",
            previous_urls.len(),
            cfg.output_dir.to_string_lossy(),
            report_path.to_string_lossy()
        ));
        return Ok(());
    }

    let initial_mode = resolve_initial_mode(cfg);
    let chrome_bootstrap = bootstrap_chrome_runtime(cfg).await;
    for warning in &chrome_bootstrap.warnings {
        println!("{} {}", muted("[Chrome Bootstrap]"), warning);
    }

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
        let robots_stats = append_robots_backfill(
            cfg,
            start_url,
            &cfg.output_dir,
            &seen_urls,
            &mut final_summary,
        )
        .await?;
        spinner.finish(&format!(
            "sitemap backfill complete (robots_extra_written={})",
            robots_stats.written
        ));
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

    let current_urls = read_manifest_urls(&manifest_path).await?;
    let report_path = write_audit_diff(
        &cfg.output_dir,
        start_url,
        &previous_urls,
        &current_urls,
        false,
        None,
    )
    .await?;

    log_done(&format!(
        "command=crawl pages_seen={} markdown_files={} thin_pages={} elapsed_ms={} output_dir={} audit_report={}",
        final_summary.pages_seen,
        final_summary.markdown_files,
        final_summary.thin_pages,
        final_summary.elapsed_ms,
        cfg.output_dir.to_string_lossy(),
        report_path.to_string_lossy(),
    ));

    Ok(())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SitemapDiscoveryStats {
    pub robots_declared_sitemaps: usize,
    pub seeded_default_sitemaps: usize,
    pub discovered_sitemap_documents: usize,
    pub parsed_sitemap_documents: usize,
    pub discovered_urls: usize,
    pub filtered_out_of_scope_host: usize,
    pub filtered_out_of_scope_path: usize,
    pub filtered_excluded_prefix: usize,
    pub failed_fetches: usize,
    pub parse_errors: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SitemapDiscoveryResult {
    pub urls: Vec<String>,
    pub stats: SitemapDiscoveryStats,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RobotsBackfillStats {
    discovered_urls: usize,
    candidates: usize,
    fetched_ok: usize,
    written: usize,
    failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestAuditEntry {
    url: String,
    file_path: String,
    markdown_chars: usize,
    fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CrawlAuditSnapshot {
    generated_at_epoch_ms: u128,
    start_url: String,
    output_dir: String,
    exclude_path_prefix: Vec<String>,
    sitemap: SitemapDiscoveryStats,
    discovered_url_count: usize,
    manifest_entry_count: usize,
    manifest_entries: Vec<ManifestAuditEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CrawlAuditSnapshotDiff {
    generated_at_epoch_ms: u128,
    previous_report: String,
    current_report: String,
    discovered_added: usize,
    discovered_removed: usize,
    manifest_added: usize,
    manifest_removed: usize,
    manifest_changed: usize,
}

fn now_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

async fn fetch_text_with_retry(
    client: &reqwest::Client,
    url: &str,
    retries: usize,
    backoff_ms: u64,
) -> Option<String> {
    for attempt in 0..=retries {
        let response = client.get(url).send().await;
        if let Ok(resp) = response {
            if resp.status().is_success() {
                if let Ok(text) = resp.text().await {
                    return Some(text);
                }
            }
        }
        if attempt < retries {
            let delay = backoff_ms.saturating_mul((attempt + 1) as u64);
            if delay > 0 {
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
        }
    }
    None
}

pub(crate) async fn discover_sitemap_urls_with_robots(
    cfg: &Config,
    start_url: &str,
) -> Result<SitemapDiscoveryResult, Box<dyn Error>> {
    let parsed = Url::parse(start_url)?;
    let scheme = parsed.scheme().to_string();
    let host = parsed.host_str().ok_or("missing host")?.to_string();
    let root_path = parsed.path().trim_end_matches('/').to_string();
    let scoped_to_root = root_path.is_empty();
    let timeout = Duration::from_millis(cfg.request_timeout_ms.unwrap_or(30_000));
    let client = reqwest::Client::builder().timeout(timeout).build()?;

    let mut stats = SitemapDiscoveryStats {
        seeded_default_sitemaps: 3,
        ..SitemapDiscoveryStats::default()
    };
    let mut queue: VecDeque<String> = VecDeque::from(vec![
        format!("{scheme}://{host}/sitemap.xml"),
        format!("{scheme}://{host}/sitemap_index.xml"),
        format!("{scheme}://{host}/sitemap-index.xml"),
    ]);
    let robots_url = format!("{scheme}://{host}/robots.txt");
    if let Some(robots_txt) = fetch_text_with_retry(
        &client,
        &robots_url,
        cfg.fetch_retries,
        cfg.retry_backoff_ms,
    )
    .await
    {
        let robots_sitemaps = extract_robots_sitemaps(&robots_txt);
        stats.robots_declared_sitemaps = robots_sitemaps.len();
        for sitemap in robots_sitemaps {
            queue.push_back(sitemap);
        }
    }

    let mut seen_sitemaps = HashSet::new();
    let mut urls = HashSet::new();
    let max_sitemaps = cfg.max_sitemaps.max(1);
    while let Some(next_sitemap) = queue.pop_front() {
        if seen_sitemaps.len() >= max_sitemaps {
            break;
        }
        let Some(canonical_sitemap) = canonicalize_url(&next_sitemap) else {
            stats.parse_errors += 1;
            continue;
        };
        if !seen_sitemaps.insert(canonical_sitemap.clone()) {
            continue;
        }
        stats.discovered_sitemap_documents = seen_sitemaps.len();
        let Some(xml) = fetch_text_with_retry(
            &client,
            &canonical_sitemap,
            cfg.fetch_retries,
            cfg.retry_backoff_ms,
        )
        .await
        else {
            stats.failed_fetches += 1;
            continue;
        };
        stats.parsed_sitemap_documents += 1;
        let is_index = xml.to_ascii_lowercase().contains("<sitemapindex");
        for loc in extract_loc_values(&xml) {
            let Ok(url) = Url::parse(&loc) else {
                stats.parse_errors += 1;
                continue;
            };
            let Some(url_host) = url.host_str() else {
                stats.parse_errors += 1;
                continue;
            };
            let host_ok = if cfg.include_subdomains {
                url_host == host || url_host.ends_with(&format!(".{host}"))
            } else {
                url_host == host
            };
            if !host_ok {
                stats.filtered_out_of_scope_host += 1;
                continue;
            }
            if !scoped_to_root {
                let p = url.path();
                let scoped_prefix = format!("{root_path}/");
                if p != root_path && !p.starts_with(&scoped_prefix) {
                    stats.filtered_out_of_scope_path += 1;
                    continue;
                }
            }
            if is_excluded_url_path(&loc, &cfg.exclude_path_prefix) {
                stats.filtered_excluded_prefix += 1;
                continue;
            }
            let Some(canonical_loc) = canonicalize_url(&loc) else {
                stats.parse_errors += 1;
                continue;
            };
            if is_index {
                queue.push_back(canonical_loc);
            } else {
                urls.insert(canonical_loc);
            }
        }
    }

    let mut discovered_urls: Vec<String> = urls.into_iter().collect();
    discovered_urls.sort();
    stats.discovered_urls = discovered_urls.len();
    Ok(SitemapDiscoveryResult {
        urls: discovered_urls,
        stats,
    })
}

async fn append_robots_backfill(
    cfg: &Config,
    start_url: &str,
    output_dir: &Path,
    seen_urls: &HashSet<String>,
    summary: &mut crate::axon_cli::crates::crawl::engine::CrawlSummary,
) -> Result<RobotsBackfillStats, Box<dyn Error>> {
    let discovery = discover_sitemap_urls_with_robots(cfg, start_url).await?;
    let manifest_path = output_dir.join("manifest.jsonl");
    let manifest_urls = read_manifest_urls(&manifest_path).await?;
    let candidates: Vec<String> = discovery
        .urls
        .iter()
        .filter(|url| !seen_urls.contains(*url) && !manifest_urls.contains(*url))
        .cloned()
        .collect();
    if candidates.is_empty() {
        return Ok(RobotsBackfillStats {
            discovered_urls: discovery.urls.len(),
            ..RobotsBackfillStats::default()
        });
    }

    let markdown_dir = output_dir.join("markdown");
    let timeout = Duration::from_millis(cfg.request_timeout_ms.unwrap_or(30_000));
    let client = reqwest::Client::builder().timeout(timeout).build()?;
    let mut manifest = BufWriter::new(
        tokio::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&manifest_path)
            .await?,
    );
    let mut idx = summary.markdown_files;
    let mut stats = RobotsBackfillStats {
        discovered_urls: discovery.urls.len(),
        candidates: candidates.len(),
        ..RobotsBackfillStats::default()
    };

    for url in candidates {
        let Some(html) =
            fetch_text_with_retry(&client, &url, cfg.fetch_retries, cfg.retry_backoff_ms).await
        else {
            stats.failed += 1;
            continue;
        };
        stats.fetched_ok += 1;
        let md = to_markdown(&html);
        let markdown_chars = md.chars().count();
        if markdown_chars < cfg.min_markdown_chars {
            summary.thin_pages += 1;
        }
        if markdown_chars < cfg.min_markdown_chars && cfg.drop_thin_markdown {
            continue;
        }

        idx += 1;
        let file = markdown_dir.join(url_to_filename(&url, idx));
        tokio::fs::write(&file, md).await?;
        let rec = serde_json::json!({
            "url": url,
            "file_path": file.to_string_lossy(),
            "markdown_chars": markdown_chars,
            "source": "robots_sitemap_backfill"
        });
        let mut line = rec.to_string();
        line.push('\n');
        manifest.write_all(line.as_bytes()).await?;
        summary.markdown_files += 1;
        stats.written += 1;
    }
    manifest.flush().await?;
    Ok(stats)
}

async fn read_manifest_entries(
    output_dir: &Path,
) -> Result<Vec<ManifestAuditEntry>, Box<dyn Error>> {
    let manifest_path = output_dir.join("manifest.jsonl");
    if !manifest_path.exists() {
        return Ok(Vec::new());
    }
    let file = tokio::fs::File::open(&manifest_path).await?;
    let mut reader = BufReader::new(file).lines();
    let mut entries = Vec::new();
    while let Some(line) = reader.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        let Some(url) = json.get("url").and_then(|v| v.as_str()) else {
            continue;
        };
        let file_path = json
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let markdown_chars = json
            .get("markdown_chars")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let fingerprint = if file_path.is_empty() {
            "no-file-path".to_string()
        } else {
            match tokio::fs::read(&file_path).await {
                Ok(bytes) => fnv1a64_hex(&bytes),
                Err(_) => "file-not-found".to_string(),
            }
        };
        entries.push(ManifestAuditEntry {
            url: url.to_string(),
            file_path,
            markdown_chars,
            fingerprint,
        });
    }
    Ok(entries)
}

fn fnv1a64_hex(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

async fn persist_audit_snapshot(
    cfg: &Config,
    start_url: &str,
) -> Result<(PathBuf, CrawlAuditSnapshot), Box<dyn Error>> {
    let now = now_epoch_ms();
    let discovery = discover_sitemap_urls_with_robots(cfg, start_url).await?;
    let manifest_entries = read_manifest_entries(&cfg.output_dir).await?;
    let snapshot = CrawlAuditSnapshot {
        generated_at_epoch_ms: now,
        start_url: start_url.to_string(),
        output_dir: cfg.output_dir.to_string_lossy().to_string(),
        exclude_path_prefix: cfg.exclude_path_prefix.clone(),
        sitemap: discovery.stats,
        discovered_url_count: discovery.urls.len(),
        manifest_entry_count: manifest_entries.len(),
        manifest_entries,
    };
    let audit_dir = cfg.output_dir.join("reports").join("crawl-audit");
    tokio::fs::create_dir_all(&audit_dir).await?;
    let path = audit_dir.join(format!("audit-{now}.json"));
    tokio::fs::write(&path, serde_json::to_string_pretty(&snapshot)?).await?;
    Ok((path, snapshot))
}

async fn list_audit_reports(output_dir: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let audit_dir = output_dir.join("reports").join("crawl-audit");
    if !audit_dir.exists() {
        return Ok(Vec::new());
    }
    let mut entries = tokio::fs::read_dir(audit_dir).await?;
    let mut out = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|name| name.starts_with("audit-") && name.ends_with(".json"))
        {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

async fn read_audit_snapshot(path: &Path) -> Result<CrawlAuditSnapshot, Box<dyn Error>> {
    let bytes = tokio::fs::read(path).await?;
    Ok(serde_json::from_slice::<CrawlAuditSnapshot>(&bytes)?)
}

fn build_snapshot_diff(
    previous_report: &Path,
    current_report: &Path,
    previous: &CrawlAuditSnapshot,
    current: &CrawlAuditSnapshot,
) -> CrawlAuditSnapshotDiff {
    let previous_discovered: HashSet<&String> =
        previous.manifest_entries.iter().map(|e| &e.url).collect();
    let current_discovered: HashSet<&String> =
        current.manifest_entries.iter().map(|e| &e.url).collect();
    let manifest_added = current_discovered.difference(&previous_discovered).count();
    let manifest_removed = previous_discovered.difference(&current_discovered).count();

    let prev_map: HashMap<&str, &str> = previous
        .manifest_entries
        .iter()
        .map(|entry| (entry.url.as_str(), entry.fingerprint.as_str()))
        .collect();
    let mut manifest_changed = 0usize;
    for entry in &current.manifest_entries {
        if let Some(prev_fp) = prev_map.get(entry.url.as_str()) {
            if *prev_fp != entry.fingerprint.as_str() {
                manifest_changed += 1;
            }
        }
    }

    let prev_sitemap_urls = previous.discovered_url_count;
    let curr_sitemap_urls = current.discovered_url_count;
    CrawlAuditSnapshotDiff {
        generated_at_epoch_ms: now_epoch_ms(),
        previous_report: previous_report.to_string_lossy().to_string(),
        current_report: current_report.to_string_lossy().to_string(),
        discovered_added: curr_sitemap_urls.saturating_sub(prev_sitemap_urls),
        discovered_removed: prev_sitemap_urls.saturating_sub(curr_sitemap_urls),
        manifest_added,
        manifest_removed,
        manifest_changed,
    }
}

async fn run_crawl_audit(cfg: &Config, start_url: &str) -> Result<(), Box<dyn Error>> {
    validate_url(start_url)?;
    let (path, snapshot) = persist_audit_snapshot(cfg, start_url).await?;
    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "audit_report_path": path,
                "snapshot": snapshot,
            }))?
        );
    } else {
        println!("{}", primary("Crawl Audit"));
        println!("  {} {}", muted("Report:"), path.to_string_lossy());
        println!(
            "  {} {}",
            muted("Discovered URLs:"),
            snapshot.discovered_url_count
        );
        println!(
            "  {} {}",
            muted("Manifest entries:"),
            snapshot.manifest_entry_count
        );
    }
    Ok(())
}

async fn run_crawl_audit_diff(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let reports = list_audit_reports(&cfg.output_dir).await?;
    if reports.len() < 2 {
        return Err("crawl diff requires at least two persisted crawl audit reports".into());
    }
    let previous_report = reports[reports.len() - 2].clone();
    let current_report = reports[reports.len() - 1].clone();
    let previous = read_audit_snapshot(&previous_report).await?;
    let current = read_audit_snapshot(&current_report).await?;
    let diff = build_snapshot_diff(&previous_report, &current_report, &previous, &current);
    let diff_path = cfg
        .output_dir
        .join("reports")
        .join("crawl-audit")
        .join(format!("diff-{}.json", now_epoch_ms()));
    tokio::fs::write(&diff_path, serde_json::to_string_pretty(&diff)?).await?;

    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "diff_report_path": diff_path,
                "diff": diff,
            }))?
        );
    } else {
        println!("{}", primary("Crawl Audit Diff"));
        println!("  {} {}", muted("Report:"), diff_path.to_string_lossy());
        println!("  {} {}", muted("Manifest added:"), diff.manifest_added);
        println!("  {} {}", muted("Manifest removed:"), diff.manifest_removed);
        println!("  {} {}", muted("Manifest changed:"), diff.manifest_changed);
    }
    Ok(())
}
