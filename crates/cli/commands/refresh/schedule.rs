use super::resolve::resolve_schedule_urls;
use crate::crates::core::config::Config;
use crate::crates::core::http::validate_url;
use crate::crates::core::logging::{log_info, log_warn};
use crate::crates::core::ui::{accent, muted, status_text, symbol_for_status};
use crate::crates::jobs::common::make_pool;
use crate::crates::jobs::refresh::ensure_schema_once;
use crate::crates::jobs::refresh::{
    RefreshScheduleCreate, claim_due_refresh_schedules_with_pool, create_refresh_schedule,
    delete_refresh_schedule, list_refresh_schedules, mark_refresh_schedule_ran_with_pool,
    set_refresh_schedule_enabled, start_refresh_job_with_pool,
};
use crate::crates::jobs::watch::{WatchDefCreate, create_watch_def, list_watch_defs};
use chrono::{Duration, Utc};
use std::error::Error;
use tokio::time::Duration as TokioDuration;

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

pub fn tier_to_seconds(tier: &str) -> Option<i64> {
    match tier.trim().to_ascii_lowercase().as_str() {
        "high" => Some(REFRESH_TIER_HIGH_SECONDS),
        "medium" => Some(REFRESH_TIER_MEDIUM_SECONDS),
        "low" => Some(REFRESH_TIER_LOW_SECONDS),
        _ => None,
    }
}

pub fn refresh_schedule_tick_secs_default() -> u64 {
    REFRESH_SCHEDULE_WORKER_DEFAULT_TICK_SECS
}

fn refresh_schedule_tick_secs() -> u64 {
    std::env::var(REFRESH_SCHEDULE_WORKER_TICK_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|secs| *secs > 0)
        .unwrap_or_else(refresh_schedule_tick_secs_default)
}

pub async fn handle_refresh_schedule(cfg: &Config) -> Result<(), Box<dyn Error>> {
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
    let watch_payload = serde_json::json!({
        "seed_url": created.seed_url,
        "urls": created.urls_json,
    });
    let _ = create_watch_def(
        cfg,
        &WatchDefCreate {
            name: created.name.clone(),
            task_type: "refresh".to_string(),
            task_payload: watch_payload,
            every_seconds: created.every_seconds,
            enabled: created.enabled,
            next_run_at: created.next_run_at,
        },
    )
    .await;

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
    let watch_defs = list_watch_defs(cfg, 500).await?;
    let refresh_watch_defs = watch_defs
        .into_iter()
        .filter(|w| w.task_type == "refresh")
        .collect::<Vec<_>>();
    let schedules = if refresh_watch_defs.is_empty() {
        list_refresh_schedules(cfg, 200)
            .await?
            .into_iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "enabled": s.enabled,
                    "every_seconds": s.every_seconds,
                })
            })
            .collect::<Vec<_>>()
    } else {
        refresh_watch_defs
            .into_iter()
            .map(|w| {
                serde_json::json!({
                    "name": w.name,
                    "enabled": w.enabled,
                    "every_seconds": w.every_seconds,
                })
            })
            .collect::<Vec<_>>()
    };
    if cfg.json_output {
        println!("{}", serde_json::to_string_pretty(&schedules)?);
        return Ok(());
    }

    println!("{}", crate::crates::core::ui::primary("Refresh Schedules"));
    if schedules.is_empty() {
        println!("  {}", muted("No refresh schedules found."));
        return Ok(());
    }

    for schedule in schedules {
        let enabled = schedule
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let name = schedule
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");
        let status = if enabled {
            status_text("running")
        } else {
            status_text("paused")
        };
        println!(
            "  {} {} {}",
            symbol_for_status("pending"),
            accent(name),
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

pub async fn handle_refresh_schedule_run_due(cfg: &Config) -> Result<(), Box<dyn Error>> {
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
    let pool = make_pool(cfg).await?;
    ensure_schema_once(&pool).await?;
    let claimed = claim_due_refresh_schedules_with_pool(&pool, batch as i64).await?;
    let now = Utc::now();
    let mut dispatched = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;
    let mut jobs = Vec::new();

    for schedule in &claimed {
        let urls = resolve_schedule_urls(cfg, schedule).await?;
        if urls.is_empty() {
            let next_run_at = now + Duration::seconds(schedule.every_seconds);
            if let Err(err) =
                mark_refresh_schedule_ran_with_pool(&pool, schedule.id, next_run_at).await
            {
                log_warn(&format!(
                    "refresh schedule mark_ran failed for skipped schedule={} id={}: {err}",
                    schedule.name, schedule.id
                ));
            }
            skipped += 1;
            continue;
        }

        match start_refresh_job_with_pool(&pool, cfg, &urls, true).await {
            Ok(job_id) => {
                let next_run_at = now + Duration::seconds(schedule.every_seconds);
                if let Err(err) =
                    mark_refresh_schedule_ran_with_pool(&pool, schedule.id, next_run_at).await
                {
                    log_warn(&format!(
                        "refresh schedule mark_ran failed for schedule={} id={}: {err}",
                        schedule.name, schedule.id
                    ));
                }
                dispatched += 1;
                jobs.push(serde_json::json!({
                    "schedule_id": schedule.id,
                    "name": schedule.name,
                    "job_id": job_id,
                    "target_count": urls.len(),
                    "next_run_at": next_run_at,
                }));
            }
            Err(err) => {
                log_warn(&format!(
                    "refresh schedule worker failed to dispatch schedule={} error={err}",
                    schedule.name
                ));
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
