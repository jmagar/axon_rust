use super::common::parse_urls;
use crate::crates::core::config::Config;
use crate::crates::core::content::url_to_domain;
use crate::crates::core::http::validate_url;
use crate::crates::core::logging::{log_info, log_warn};
use crate::crates::core::ui::{
    accent, confirm_destructive, muted, primary, status_text, symbol_for_status,
};
use crate::crates::crawl::manifest::read_manifest_urls;
use crate::crates::jobs::refresh::{
    RefreshScheduleCreate, cancel_refresh_job, claim_due_refresh_schedules, cleanup_refresh_jobs,
    clear_refresh_jobs, create_refresh_schedule, delete_refresh_schedule, get_refresh_job,
    list_refresh_jobs, list_refresh_schedules, mark_refresh_schedule_ran,
    recover_stale_refresh_jobs, run_refresh_once, run_refresh_worker, set_refresh_schedule_enabled,
    start_refresh_job,
};
use chrono::{Duration, Utc};
use std::collections::HashSet;
use std::error::Error;
use std::path::PathBuf;
use tokio::time::Duration as TokioDuration;
use uuid::Uuid;

const REFRESH_TIER_HIGH_SECONDS: i64 = 1800;
const REFRESH_TIER_MEDIUM_SECONDS: i64 = 21600;
const REFRESH_TIER_LOW_SECONDS: i64 = 86400;
const REFRESH_SCHEDULE_WORKER_DEFAULT_TICK_SECS: u64 = 30;
const REFRESH_SCHEDULE_WORKER_TICK_ENV: &str = "AXON_REFRESH_SCHEDULER_TICK_SECS";

struct RefreshScheduleDueSweep {
    claimed_count: usize,
    dispatched_count: usize,
    skipped_count: usize,
    failed_count: usize,
    jobs: Vec<serde_json::Value>,
}

fn tier_to_seconds(tier: &str) -> Option<i64> {
    match tier.trim().to_ascii_lowercase().as_str() {
        "high" => Some(REFRESH_TIER_HIGH_SECONDS),
        "medium" => Some(REFRESH_TIER_MEDIUM_SECONDS),
        "low" => Some(REFRESH_TIER_LOW_SECONDS),
        _ => None,
    }
}

fn refresh_schedule_tick_secs_default() -> u64 {
    REFRESH_SCHEDULE_WORKER_DEFAULT_TICK_SECS
}

fn refresh_schedule_tick_secs() -> u64 {
    std::env::var(REFRESH_SCHEDULE_WORKER_TICK_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|secs| *secs > 0)
        .unwrap_or_else(refresh_schedule_tick_secs_default)
}

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
        "schedule" => handle_refresh_schedule(cfg).await?,
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

async fn handle_refresh_schedule(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let action = cfg
        .positional
        .get(1)
        .map(|s| s.as_str())
        .ok_or("refresh schedule requires a subcommand")?;
    match action {
        "add" => handle_refresh_schedule_add(cfg).await?,
        "list" => handle_refresh_schedule_list(cfg).await?,
        "enable" => handle_refresh_schedule_enable(cfg).await?,
        "disable" => handle_refresh_schedule_disable(cfg).await?,
        "delete" => handle_refresh_schedule_delete(cfg).await?,
        "run-due" => handle_refresh_schedule_run_due(cfg).await?,
        "worker" => handle_refresh_schedule_worker(cfg).await?,
        _ => return Err(format!("unknown refresh schedule subcommand: {action}").into()),
    }
    Ok(())
}

fn schedule_name_arg(cfg: &Config, action: &str, index: usize) -> Result<String, Box<dyn Error>> {
    cfg.positional
        .get(index)
        .cloned()
        .ok_or_else(|| format!("refresh schedule {action} requires <name>").into())
}

async fn handle_refresh_schedule_add(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let name = schedule_name_arg(cfg, "add", 2)?;
    let mut seed_url: Option<String> = None;
    let mut every_seconds: Option<i64> = None;
    let mut tier_seconds: Option<i64> = None;
    let mut urls: Option<Vec<String>> = None;

    let mut idx = 3usize;
    while idx < cfg.positional.len() {
        match cfg.positional[idx].as_str() {
            "--every-seconds" => {
                let value = cfg
                    .positional
                    .get(idx + 1)
                    .ok_or("refresh schedule add requires value after --every-seconds")?;
                let parsed = value
                    .parse::<i64>()
                    .map_err(|_| "refresh schedule add --every-seconds must be an integer")?;
                if parsed > 0 {
                    every_seconds = Some(parsed);
                }
                idx += 2;
            }
            "--tier" => {
                let value = cfg
                    .positional
                    .get(idx + 1)
                    .ok_or("refresh schedule add requires value after --tier")?;
                tier_seconds = Some(
                    tier_to_seconds(value)
                        .ok_or("refresh schedule add --tier must be one of: high, medium, low")?,
                );
                idx += 2;
            }
            "--urls" => {
                let value = cfg
                    .positional
                    .get(idx + 1)
                    .ok_or("refresh schedule add requires value after --urls")?;
                let parsed_urls = value
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>();
                if parsed_urls.is_empty() {
                    return Err("refresh schedule add --urls cannot be empty".into());
                }
                for url in &parsed_urls {
                    validate_url(url)?;
                }
                urls = Some(parsed_urls);
                idx += 2;
            }
            token => {
                if token.starts_with("--") {
                    return Err(format!("unknown refresh schedule add flag: {token}").into());
                }
                if seed_url.is_some() {
                    return Err("refresh schedule add accepts at most one [seed_url]".into());
                }
                validate_url(token)?;
                seed_url = Some(token.to_string());
                idx += 1;
            }
        }
    }

    let every_seconds = every_seconds
        .or(tier_seconds)
        .unwrap_or(REFRESH_TIER_MEDIUM_SECONDS);
    if seed_url.is_none() && urls.is_none() {
        return Err(
            "refresh schedule add requires at least one of [seed_url] or --urls <csv>".into(),
        );
    }
    let next_run_at = Utc::now() + Duration::seconds(every_seconds);
    let schedule = RefreshScheduleCreate {
        name: name.clone(),
        seed_url,
        urls,
        every_seconds,
        enabled: true,
        next_run_at,
    };
    let created = create_refresh_schedule(cfg, &schedule).await?;

    if cfg.json_output {
        println!("{}", serde_json::to_string_pretty(&created)?);
    } else {
        println!(
            "{} created refresh schedule {}",
            symbol_for_status("completed"),
            accent(&created.name)
        );
        println!("  {} {}", muted("Every:"), created.every_seconds);
        println!("  {} {}", muted("Enabled:"), created.enabled);
    }
    Ok(())
}

async fn handle_refresh_schedule_list(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let schedules = list_refresh_schedules(cfg, 200).await?;
    if cfg.json_output {
        println!("{}", serde_json::to_string_pretty(&schedules)?);
        return Ok(());
    }

    println!("{}", primary("Refresh Schedules"));
    if schedules.is_empty() {
        println!("  {}", muted("No refresh schedules found."));
        return Ok(());
    }

    for schedule in schedules {
        let status = if schedule.enabled {
            status_text("running")
        } else {
            status_text("paused")
        };
        println!(
            "  {} {} {}",
            symbol_for_status("pending"),
            accent(&schedule.name),
            status
        );
    }
    Ok(())
}

async fn handle_refresh_schedule_enable(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let name = schedule_name_arg(cfg, "enable", 2)?;
    let updated = set_refresh_schedule_enabled(cfg, &name, true).await?;
    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({"name": name, "enabled": true, "updated": updated})
        );
    } else if updated {
        println!(
            "{} enabled refresh schedule {}",
            symbol_for_status("completed"),
            accent(&name)
        );
    } else {
        println!(
            "{} refresh schedule not found: {}",
            symbol_for_status("error"),
            accent(&name)
        );
    }
    Ok(())
}

async fn handle_refresh_schedule_disable(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let name = schedule_name_arg(cfg, "disable", 2)?;
    let updated = set_refresh_schedule_enabled(cfg, &name, false).await?;
    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({"name": name, "enabled": false, "updated": updated})
        );
    } else if updated {
        println!(
            "{} disabled refresh schedule {}",
            symbol_for_status("completed"),
            accent(&name)
        );
    } else {
        println!(
            "{} refresh schedule not found: {}",
            symbol_for_status("error"),
            accent(&name)
        );
    }
    Ok(())
}

async fn handle_refresh_schedule_delete(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let name = schedule_name_arg(cfg, "delete", 2)?;
    let deleted = delete_refresh_schedule(cfg, &name).await?;
    if cfg.json_output {
        println!("{}", serde_json::json!({"name": name, "deleted": deleted}));
    } else if deleted {
        println!(
            "{} deleted refresh schedule {}",
            symbol_for_status("completed"),
            accent(&name)
        );
    } else {
        println!(
            "{} refresh schedule not found: {}",
            symbol_for_status("error"),
            accent(&name)
        );
    }
    Ok(())
}

async fn handle_refresh_schedule_run_due(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let mut batch: usize = 25;
    let mut idx = 2usize;
    while idx < cfg.positional.len() {
        match cfg.positional[idx].as_str() {
            "--batch" => {
                let value = cfg
                    .positional
                    .get(idx + 1)
                    .ok_or("refresh schedule run-due requires value after --batch")?;
                batch = value
                    .parse::<usize>()
                    .map_err(|_| "refresh schedule run-due --batch must be an integer")?;
                if batch == 0 {
                    return Err("refresh schedule run-due --batch must be greater than 0".into());
                }
                idx += 2;
            }
            token => {
                return Err(format!("unknown refresh schedule run-due flag: {token}").into());
            }
        }
    }

    let sweep = run_refresh_schedule_due_sweep(cfg, batch).await?;
    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({
                "claimed": sweep.claimed_count,
                "dispatched": sweep.dispatched_count,
                "skipped": sweep.skipped_count,
                "failed": sweep.failed_count,
                "jobs": sweep.jobs,
            })
        );
    } else {
        println!(
            "{} claimed={} dispatched={} skipped={} failed={}",
            symbol_for_status("completed"),
            sweep.claimed_count,
            sweep.dispatched_count,
            sweep.skipped_count,
            sweep.failed_count
        );
    }
    Ok(())
}

async fn run_refresh_schedule_due_sweep(
    cfg: &Config,
    batch: usize,
) -> Result<RefreshScheduleDueSweep, Box<dyn Error>> {
    let claimed = claim_due_refresh_schedules(cfg, batch as i64).await?;
    let now = Utc::now();
    let mut dispatched = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;
    let mut jobs = Vec::new();

    for schedule in &claimed {
        let urls = resolve_schedule_urls(cfg, schedule).await?;
        if urls.is_empty() {
            skipped += 1;
            continue;
        }

        match start_refresh_job(cfg, &urls).await {
            Ok(job_id) => {
                let next_run_at = now + Duration::seconds(schedule.every_seconds);
                let _ = mark_refresh_schedule_ran(cfg, schedule.id, next_run_at).await?;
                dispatched += 1;
                jobs.push(serde_json::json!({
                    "schedule_id": schedule.id,
                    "name": schedule.name,
                    "job_id": job_id,
                    "target_count": urls.len(),
                    "next_run_at": next_run_at,
                }));
            }
            Err(_) => {
                failed += 1;
            }
        }
    }

    Ok(RefreshScheduleDueSweep {
        claimed_count: claimed.len(),
        dispatched_count: dispatched,
        skipped_count: skipped,
        failed_count: failed,
        jobs,
    })
}

async fn handle_refresh_schedule_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let tick_secs = refresh_schedule_tick_secs();
    log_info(&format!(
        "refresh schedule worker started tick_secs={tick_secs} (env={REFRESH_SCHEDULE_WORKER_TICK_ENV})"
    ));

    loop {
        log_info("refresh schedule worker running due sweep");
        match run_refresh_schedule_due_sweep(cfg, 25).await {
            Ok(sweep) => {
                log_info(&format!(
                    "refresh schedule worker sweep complete claimed={} dispatched={} skipped={} failed={}",
                    sweep.claimed_count,
                    sweep.dispatched_count,
                    sweep.skipped_count,
                    sweep.failed_count
                ));
            }
            Err(err) => {
                log_warn(&format!("refresh schedule worker sweep failed: {err}"));
            }
        }

        tokio::time::sleep(TokioDuration::from_secs(tick_secs)).await;
    }
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

async fn resolve_schedule_urls(
    cfg: &Config,
    schedule: &crate::crates::jobs::refresh::RefreshSchedule,
) -> Result<Vec<String>, Box<dyn Error>> {
    let mut urls = match schedule.urls_json.as_ref() {
        Some(value) => serde_json::from_value::<Vec<String>>(value.clone()).unwrap_or_default(),
        None => Vec::new(),
    };

    if urls.is_empty()
        && let Some(seed_url) = schedule.seed_url.as_deref()
    {
        urls = urls_from_manifest_seed(cfg, seed_url).await?;
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

#[cfg(test)]
mod tests {
    use super::{
        handle_refresh_schedule_run_due, refresh_schedule_tick_secs_default, tier_to_seconds,
    };
    use crate::crates::jobs::common::{make_pool, test_config};
    use crate::crates::jobs::refresh::{
        RefreshScheduleCreate, create_refresh_schedule, delete_refresh_schedule, list_refresh_jobs,
    };
    use chrono::{Duration, Utc};
    use std::env;
    use std::error::Error;
    use tempfile::TempDir;
    use uuid::Uuid;

    #[test]
    fn refresh_tier_maps_to_expected_seconds() {
        assert_eq!(tier_to_seconds("high"), Some(1800));
        assert_eq!(tier_to_seconds("medium"), Some(21600));
        assert_eq!(tier_to_seconds("low"), Some(86400));
    }

    #[test]
    fn refresh_schedule_worker_default_tick_is_30_seconds() {
        assert_eq!(refresh_schedule_tick_secs_default(), 30);
    }

    fn pg_url() -> Option<String> {
        env::var("AXON_TEST_PG_URL")
            .ok()
            .or_else(|| env::var("AXON_PG_URL").ok())
            .filter(|v| !v.trim().is_empty())
    }

    #[tokio::test]
    async fn schedule_run_due_uses_seed_manifest_when_urls_missing() -> Result<(), Box<dyn Error>> {
        let Some(pg_url) = pg_url() else {
            return Ok(());
        };

        let temp_dir = TempDir::new()?;
        let mut cfg = test_config(&pg_url);
        cfg.output_dir = temp_dir.path().to_path_buf();

        let seed_url = "https://example.com";
        let manifest_urls = vec![
            "https://example.com/docs/a".to_string(),
            "https://example.com/docs/b".to_string(),
        ];
        let manifest_path = cfg
            .output_dir
            .join("domains")
            .join("example.com")
            .join("latest")
            .join("manifest.jsonl");
        tokio::fs::create_dir_all(
            manifest_path
                .parent()
                .ok_or("manifest path missing parent directory")?,
        )
        .await?;
        let manifest_body = manifest_urls
            .iter()
            .enumerate()
            .map(|(idx, url)| {
                serde_json::json!({
                    "url": url,
                    "relative_path": format!("markdown/{idx}.md"),
                    "markdown_chars": 100,
                    "content_hash": format!("hash-{idx}"),
                    "changed": true,
                })
                .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        tokio::fs::write(&manifest_path, manifest_body).await?;

        let schedule_name = format!("refresh-seed-fallback-{}", Uuid::new_v4());
        let _ = create_refresh_schedule(
            &cfg,
            &RefreshScheduleCreate {
                name: schedule_name.clone(),
                seed_url: Some(seed_url.to_string()),
                urls: None,
                every_seconds: 300,
                enabled: true,
                next_run_at: Utc::now() - Duration::minutes(1),
            },
        )
        .await?;

        handle_refresh_schedule_run_due(&cfg).await?;

        let jobs = list_refresh_jobs(&cfg, 50).await?;
        let matching_job = jobs.iter().find(|job| {
            serde_json::from_value::<Vec<String>>(job.urls_json.clone())
                .map(|urls| urls == manifest_urls)
                .unwrap_or(false)
        });
        assert!(matching_job.is_some());

        let pool = make_pool(&cfg).await?;
        if let Some(job) = matching_job {
            let _ = sqlx::query("DELETE FROM axon_refresh_jobs WHERE id = $1")
                .bind(job.id)
                .execute(&pool)
                .await?;
        }
        let _ = delete_refresh_schedule(&cfg, &schedule_name).await?;
        Ok(())
    }
}
