use super::*;
use crate::crates::jobs::common::{
    make_pool, open_amqp_channel, resolve_test_pg_url, stale_watchdog_confirmed,
    stale_watchdog_payload, test_config,
};
use chrono::Duration;
use tokio::time::{Duration as TokioDuration, sleep, timeout};

fn watchdog_json(observed: DateTime<Utc>, first_seen: &str) -> serde_json::Value {
    serde_json::json!({
        "_watchdog": {
            "observed_updated_at": observed.to_rfc3339(),
            "first_seen_stale_at": first_seen
        }
    })
}

#[test]
fn crawl_watchdog_payload_adds_watchdog_fields() {
    let observed = Utc::now() - Duration::seconds(30);
    let payload = stale_watchdog_payload(serde_json::json!({}), observed);
    let watchdog = payload.get("_watchdog").expect("missing _watchdog");
    assert_eq!(
        watchdog
            .get("observed_updated_at")
            .and_then(|v| v.as_str())
            .expect("missing observed_updated_at"),
        observed.to_rfc3339()
    );
    let first_seen = watchdog
        .get("first_seen_stale_at")
        .and_then(|v| v.as_str())
        .expect("missing first_seen_stale_at");
    assert!(DateTime::parse_from_rfc3339(first_seen).is_ok());
}

#[test]
fn crawl_watchdog_confirmed_rejects_mismatch_and_recent_marks() {
    let observed = Utc::now() - Duration::seconds(80);
    let payload = watchdog_json(observed, &Utc::now().to_rfc3339());
    assert!(!stale_watchdog_confirmed(&payload, observed, 60));
    assert!(!stale_watchdog_confirmed(
        &payload,
        observed + Duration::seconds(1),
        60
    ));
}

#[test]
fn crawl_watchdog_confirmed_true_after_confirm_window() {
    let observed = Utc::now() - Duration::seconds(180);
    let payload = watchdog_json(
        observed,
        &(Utc::now() - Duration::seconds(300)).to_rfc3339(),
    );
    assert!(stale_watchdog_confirmed(&payload, observed, 60));
}

fn amqp_url() -> Option<String> {
    std::env::var("AXON_TEST_AMQP_URL")
        .ok()
        .or_else(|| std::env::var("AXON_AMQP_URL").ok())
        .filter(|v| !v.trim().is_empty())
}

#[tokio::test]
async fn crawl_start_job_dedupes_active_pending_job() -> Result<(), Box<dyn Error>> {
    let Some(pg_url) = resolve_test_pg_url() else {
        return Ok(());
    };
    let cfg = test_config(&pg_url);
    let url = format!("https://example.com/crawl/{}", Uuid::new_v4());

    let first_id = start_crawl_job(&cfg, &url).await?;
    let second_id = start_crawl_job(&cfg, &url).await?;
    assert_eq!(first_id, second_id);

    let pool = make_pool(&cfg).await?;
    let _ = sqlx::query("DELETE FROM axon_crawl_jobs WHERE id = $1")
        .bind(first_id)
        .execute(&pool)
        .await;
    Ok(())
}

#[tokio::test]
async fn crawl_recover_reclaims_confirmed_stale_running_job() -> Result<(), Box<dyn Error>> {
    let Some(pg_url) = resolve_test_pg_url() else {
        return Ok(());
    };
    let cfg = test_config(&pg_url);
    let url = format!("https://example.com/crawl-recover/{}", Uuid::new_v4());
    let id = start_crawl_job(&cfg, &url).await?;
    let pool = make_pool(&cfg).await?;

    sqlx::query(
            "UPDATE axon_crawl_jobs SET status='running', updated_at=NOW() - INTERVAL '20 minutes' WHERE id=$1",
        )
        .bind(id)
        .execute(&pool)
        .await?;

    let observed_updated_at: DateTime<Utc> =
        sqlx::query_scalar("SELECT updated_at FROM axon_crawl_jobs WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await?;
    let watchdog = serde_json::json!({
        "_watchdog": {
            "observed_updated_at": observed_updated_at.to_rfc3339(),
            "first_seen_stale_at": (Utc::now() - Duration::minutes(10)).to_rfc3339(),
            "phase": "crawl",
            "pages_crawled": 3
        }
    });
    sqlx::query("UPDATE axon_crawl_jobs SET result_json=$2 WHERE id=$1")
        .bind(id)
        .bind(watchdog)
        .execute(&pool)
        .await?;

    let reclaimed = recover_stale_crawl_jobs(&cfg).await?;
    assert!(reclaimed >= 1);

    let status: String = sqlx::query_scalar("SELECT status FROM axon_crawl_jobs WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(status, "failed");

    let _ = sqlx::query("DELETE FROM axon_crawl_jobs WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await;
    Ok(())
}

#[tokio::test]
async fn crawl_ensure_schema_is_concurrency_safe() -> Result<(), Box<dyn Error>> {
    let Some(pg_url) = resolve_test_pg_url() else {
        return Ok(());
    };
    let cfg = test_config(&pg_url);
    let pool = make_pool(&cfg).await?;
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let pool = pool.clone();
        tasks.push(tokio::spawn(async move { ensure_schema(&pool).await }));
    }
    for task in tasks {
        let result = task.await?;
        result?;
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn crawl_worker_e2e_processes_pending_job_to_terminal_status() -> Result<(), Box<dyn Error>> {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let Some(pg_url) = resolve_test_pg_url() else {
                return Ok(());
            };
            let Some(_) = amqp_url() else {
                return Ok(());
            };
            let cfg = test_config(&pg_url);
            if open_amqp_channel(&cfg, &cfg.crawl_queue).await.is_err() {
                return Ok(());
            }
            let url = format!("https://example.com/crawl-worker/{}", Uuid::new_v4());
            let id = start_crawl_job(&cfg, &url).await?;

            let worker_cfg = cfg.clone();
            let worker = tokio::task::spawn_local(async move {
                let _ = run_worker(&worker_cfg).await;
            });

            let pool = make_pool(&cfg).await?;
            let wait = timeout(TokioDuration::from_secs(90), async {
                loop {
                    let status: Option<String> =
                        sqlx::query_scalar("SELECT status FROM axon_crawl_jobs WHERE id=$1")
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
                "crawl worker did not reach terminal state in time"
            );

            let status: String =
                sqlx::query_scalar("SELECT status FROM axon_crawl_jobs WHERE id = $1")
                    .bind(id)
                    .fetch_one(&pool)
                    .await?;
            assert!(matches!(
                status.as_str(),
                "completed" | "failed" | "canceled"
            ));

            let _ = sqlx::query("DELETE FROM axon_crawl_jobs WHERE id = $1")
                .bind(id)
                .execute(&pool)
                .await;
            Ok::<(), Box<dyn Error>>(())
        })
        .await
}
