use crate::crates::core::config::Config;
use crate::crates::core::logging::{log_info, log_warn};
use crate::crates::jobs::common::make_pool;
use crate::crates::jobs::refresh::start_refresh_job_with_pool;
use crate::crates::jobs::watch::{
    WATCH_RUN_STATUS_COMPLETED, WATCH_RUN_STATUS_FAILED, claim_due_watches_with_pool,
    create_watch_run_with_pool, ensure_schema_once, mark_watch_run_finished_with_pool,
};
use std::error::Error;
use tokio::time::Duration;

const WATCH_WORKER_DEFAULT_TICK_SECS: u64 = 30;

async fn dispatch_watch(
    cfg: &Config,
    pool: &sqlx::PgPool,
    watch: &crate::crates::jobs::watch::WatchDef,
) -> Result<Option<uuid::Uuid>, Box<dyn Error>> {
    match watch.task_type.as_str() {
        "refresh" => {
            let urls = watch
                .task_payload
                .get("urls")
                .and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok())
                .unwrap_or_default();
            if urls.is_empty() {
                return Ok(None);
            }
            Ok(Some(
                start_refresh_job_with_pool(pool, cfg, &urls, true).await?,
            ))
        }
        _ => Err(format!("unsupported watch task_type: {}", watch.task_type).into()),
    }
}

pub async fn run_watch_scheduler_tick(cfg: &Config) -> Result<usize, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema_once(&pool).await?;
    run_watch_tick_with_pool(cfg, &pool).await
}

async fn run_watch_tick_with_pool(
    cfg: &Config,
    pool: &sqlx::PgPool,
) -> Result<usize, Box<dyn Error>> {
    let claimed = claim_due_watches_with_pool(pool, 25).await?;
    let mut processed = 0usize;

    for watch in claimed {
        let run = create_watch_run_with_pool(pool, watch.id, None).await?;
        match dispatch_watch(cfg, pool, &watch).await {
            Ok(dispatched_job_id) => {
                let result = serde_json::json!({
                    "task_type": watch.task_type,
                    "dispatched_job_id": dispatched_job_id,
                });
                let _ = mark_watch_run_finished_with_pool(
                    pool,
                    watch.id,
                    run.id,
                    WATCH_RUN_STATUS_COMPLETED,
                    Some(&result),
                    None,
                )
                .await?;
                processed += 1;
            }
            Err(err) => {
                let _ = mark_watch_run_finished_with_pool(
                    pool,
                    watch.id,
                    run.id,
                    WATCH_RUN_STATUS_FAILED,
                    None,
                    Some(&err.to_string()),
                )
                .await?;
                log_warn(&format!(
                    "watch worker failed watch_id={} run_id={} err={err}",
                    watch.id, run.id
                ));
            }
        }
    }

    Ok(processed)
}

pub async fn run_watch_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    log_info("watch scheduler worker started");
    let pool = make_pool(cfg).await?;
    ensure_schema_once(&pool).await?;
    loop {
        match run_watch_tick_with_pool(cfg, &pool).await {
            Ok(processed) => {
                if processed > 0 {
                    log_info(&format!("watch scheduler dispatched {} run(s)", processed));
                }
            }
            Err(err) => log_warn(&format!("watch scheduler tick failed: {err}")),
        }
        tokio::time::sleep(Duration::from_secs(WATCH_WORKER_DEFAULT_TICK_SECS)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crates::jobs::common::resolve_test_pg_url;
    use crate::crates::jobs::watch::{
        WatchDefCreate, create_watch_def_with_pool, ensure_schema_once,
    };
    use chrono::Utc;
    use sqlx::PgPool;
    use uuid::Uuid;

    #[tokio::test]
    async fn watch_worker_claims_due_defs_and_dispatches_jobs() -> Result<(), Box<dyn Error>> {
        let Some(pg_url) = resolve_test_pg_url() else {
            return Ok(());
        };
        let mut cfg = Config::test_default();
        cfg.pg_url = pg_url.clone();
        let pool = match PgPool::connect(&pg_url).await {
            Ok(p) => p,
            Err(_) => return Ok(()),
        };
        ensure_schema_once(&pool).await?;

        let name = format!("watch-worker-{}", Uuid::new_v4());
        let _ = create_watch_def_with_pool(
            &pool,
            &WatchDefCreate {
                name: name.clone(),
                task_type: "refresh".to_string(),
                task_payload: serde_json::json!({"urls":["https://example.com/docs"]}),
                every_seconds: 60,
                enabled: true,
                next_run_at: Utc::now() - chrono::Duration::minutes(1),
            },
        )
        .await?;

        let processed = run_watch_scheduler_tick(&cfg).await?;
        assert!(processed >= 1);

        let _ = sqlx::query("DELETE FROM axon_watch_defs WHERE name=$1")
            .bind(name)
            .execute(&pool)
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn watch_worker_records_run_result_on_success_and_failure() -> Result<(), Box<dyn Error>>
    {
        let Some(pg_url) = resolve_test_pg_url() else {
            return Ok(());
        };
        let mut cfg = Config::test_default();
        cfg.pg_url = pg_url.clone();
        let pool = match PgPool::connect(&pg_url).await {
            Ok(p) => p,
            Err(_) => return Ok(()),
        };
        ensure_schema_once(&pool).await?;

        let success_name = format!("watch-worker-success-{}", Uuid::new_v4());
        let fail_name = format!("watch-worker-fail-{}", Uuid::new_v4());
        let _ = create_watch_def_with_pool(
            &pool,
            &WatchDefCreate {
                name: success_name.clone(),
                task_type: "refresh".to_string(),
                task_payload: serde_json::json!({"urls":["https://example.com/docs"]}),
                every_seconds: 60,
                enabled: true,
                next_run_at: Utc::now() - chrono::Duration::minutes(1),
            },
        )
        .await?;
        let _ = create_watch_def_with_pool(
            &pool,
            &WatchDefCreate {
                name: fail_name.clone(),
                task_type: "unsupported".to_string(),
                task_payload: serde_json::json!({}),
                every_seconds: 60,
                enabled: true,
                next_run_at: Utc::now() - chrono::Duration::minutes(1),
            },
        )
        .await?;

        let _ = run_watch_scheduler_tick(&cfg).await?;
        let statuses: Vec<String> = sqlx::query_scalar(
            "SELECT status FROM axon_watch_runs ORDER BY created_at DESC LIMIT 4",
        )
        .fetch_all(&pool)
        .await?;
        assert!(statuses.iter().any(|s| s == "completed"));
        assert!(statuses.iter().any(|s| s == "failed"));

        let _ = sqlx::query("DELETE FROM axon_watch_defs WHERE name IN ($1,$2)")
            .bind(success_name)
            .bind(fail_name)
            .execute(&pool)
            .await?;
        Ok(())
    }
}
