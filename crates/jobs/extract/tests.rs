use super::*;
use crate::crates::jobs::common::test_config;
use chrono::{DateTime, Duration, Utc};
use std::env;
use tokio::time::{Duration as TokioDuration, sleep, timeout};

fn pg_url() -> Option<String> {
    env::var("AXON_TEST_PG_URL")
        .ok()
        .or_else(|| env::var("AXON_PG_URL").ok())
        .filter(|v| !v.trim().is_empty())
}

#[tokio::test]
async fn extract_start_job_dedupes_active_pending_job() -> Result<(), Box<dyn Error>> {
    let Some(pg_url) = pg_url() else {
        return Ok(());
    };
    let cfg = test_config(&pg_url);
    let url = format!("https://example.com/extract/{}", Uuid::new_v4());
    let urls = vec![url];

    let first_id = start_extract_job(&cfg, &urls, Some("extract prompt".to_string())).await?;
    let second_id = start_extract_job(&cfg, &urls, Some("extract prompt".to_string())).await?;
    assert_eq!(first_id, second_id);

    let pool = make_pool(&cfg).await?;
    let _ = sqlx::query("DELETE FROM axon_extract_jobs WHERE id = $1")
        .bind(first_id)
        .execute(&pool)
        .await;
    Ok(())
}

#[tokio::test]
async fn extract_recover_reclaims_confirmed_stale_running_job() -> Result<(), Box<dyn Error>> {
    let Some(pg_url) = pg_url() else {
        return Ok(());
    };
    let cfg = test_config(&pg_url);
    let url = format!("https://example.com/recover/{}", Uuid::new_v4());
    let urls = vec![url];
    let id = start_extract_job(&cfg, &urls, None).await?;
    let pool = make_pool(&cfg).await?;

    sqlx::query(
            "UPDATE axon_extract_jobs SET status='running', updated_at=NOW() - INTERVAL '20 minutes' WHERE id=$1",
        )
        .bind(id)
        .execute(&pool)
        .await?;

    let observed_updated_at: DateTime<Utc> =
        sqlx::query_scalar("SELECT updated_at FROM axon_extract_jobs WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await?;
    let watchdog = serde_json::json!({
        "_watchdog": {
            "observed_updated_at": observed_updated_at.to_rfc3339(),
            "first_seen_stale_at": (Utc::now() - Duration::minutes(10)).to_rfc3339()
        }
    });
    sqlx::query("UPDATE axon_extract_jobs SET result_json=$2 WHERE id=$1")
        .bind(id)
        .bind(watchdog)
        .execute(&pool)
        .await?;

    let reclaimed = recover_stale_extract_jobs(&cfg).await?;
    assert!(reclaimed >= 1);

    let status: String = sqlx::query_scalar("SELECT status FROM axon_extract_jobs WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(status, "failed");

    let _ = sqlx::query("DELETE FROM axon_extract_jobs WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn extract_worker_e2e_processes_pending_job_to_terminal_status() -> Result<(), Box<dyn Error>>
{
    let Some(pg_url) = pg_url() else {
        return Ok(());
    };
    let mut cfg = test_config(&pg_url);
    cfg.query = Some("extract worker e2e prompt".to_string());
    let url = format!("https://example.com/extract-worker/{}", Uuid::new_v4());
    let urls = vec![url];
    let id = start_extract_job(&cfg, &urls, cfg.query.clone()).await?;

    let worker_cfg = cfg.clone();
    let worker = tokio::task::spawn_local(async move {
        let _ = run_extract_worker(&worker_cfg).await;
    });

    let pool = make_pool(&cfg).await?;
    let wait = timeout(TokioDuration::from_secs(8), async {
        loop {
            let status: Option<String> =
                sqlx::query_scalar("SELECT status FROM axon_extract_jobs WHERE id=$1")
                    .bind(id)
                    .fetch_optional(&pool)
                    .await
                    .ok()
                    .flatten();
            if matches!(status.as_deref(), Some("completed" | "failed" | "canceled")) {
                break;
            }
            sleep(TokioDuration::from_millis(100)).await;
        }
    })
    .await;
    worker.abort();
    let _ = worker.await;
    assert!(
        wait.is_ok(),
        "extract worker did not reach terminal state in time"
    );

    let status: String = sqlx::query_scalar("SELECT status FROM axon_extract_jobs WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await?;
    assert!(matches!(
        status.as_str(),
        "completed" | "failed" | "canceled"
    ));

    let _ = sqlx::query("DELETE FROM axon_extract_jobs WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await;
    Ok(())
}
