use super::*;
use chrono::Duration;
use uuid::Uuid;

fn watchdog_json(observed: DateTime<Utc>, first_seen: &str) -> Value {
    serde_json::json!({
        "_watchdog": {
            "observed_updated_at": observed.to_rfc3339(),
            "first_seen_stale_at": first_seen
        }
    })
}

#[test]
fn stale_watchdog_payload_adds_metadata_and_normalizes_shape() {
    let observed = Utc::now() - Duration::seconds(45);
    let payload = stale_watchdog_payload(serde_json::json!("not-an-object"), observed);
    let watchdog = payload.get("_watchdog").expect("missing _watchdog");
    let observed_value = watchdog
        .get("observed_updated_at")
        .and_then(|v| v.as_str())
        .expect("missing observed_updated_at");
    assert_eq!(observed_value, observed.to_rfc3339());
    let first_seen = watchdog
        .get("first_seen_stale_at")
        .and_then(|v| v.as_str())
        .expect("missing first_seen_stale_at");
    assert!(DateTime::parse_from_rfc3339(first_seen).is_ok());
}

#[test]
fn stale_watchdog_confirmed_requires_watchdog_metadata() {
    let observed = Utc::now() - Duration::seconds(10);
    assert!(!stale_watchdog_confirmed(
        &serde_json::json!({}),
        observed,
        30
    ));
}

#[test]
fn stale_watchdog_confirmed_rejects_observed_timestamp_mismatch() {
    let observed = Utc::now() - Duration::seconds(90);
    let mismatched = observed + Duration::seconds(1);
    let payload = watchdog_json(
        observed,
        &(Utc::now() - Duration::seconds(120)).to_rfc3339(),
    );
    assert!(!stale_watchdog_confirmed(&payload, mismatched, 60));
}

#[test]
fn stale_watchdog_confirmed_rejects_malformed_first_seen() {
    let observed = Utc::now() - Duration::seconds(120);
    let payload = watchdog_json(observed, "not-a-timestamp");
    assert!(!stale_watchdog_confirmed(&payload, observed, 60));
}

#[test]
fn stale_watchdog_confirmed_requires_confirmation_window() {
    let observed = Utc::now() - Duration::seconds(10);
    let payload = watchdog_json(observed, &Utc::now().to_rfc3339());
    assert!(!stale_watchdog_confirmed(&payload, observed, 60));
}

#[test]
fn stale_watchdog_confirmed_true_after_confirmation_window_elapsed() {
    let observed = Utc::now() - Duration::seconds(120);
    let payload = watchdog_json(
        observed,
        &(Utc::now() - Duration::seconds(180)).to_rfc3339(),
    );
    assert!(stale_watchdog_confirmed(&payload, observed, 60));
}

#[tokio::test]
async fn reclaim_stale_running_jobs_two_pass_flow_marks_then_reclaims() -> Result<()> {
    let Some(pg_url) = resolve_test_pg_url() else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&pg_url)
        .await?;

    // Use the same advisory lock key as embed::ensure_schema (0xA804_0002) to avoid
    // concurrent-DDL races with the embed schema migration that runs in parallel tests.
    let mut tx = begin_schema_migration_tx(&pool, 0xA804_0002).await?;
    sqlx::query(
        r#"
            CREATE TABLE IF NOT EXISTS axon_embed_jobs (
                id UUID PRIMARY KEY,
                status TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                started_at TIMESTAMPTZ,
                finished_at TIMESTAMPTZ,
                error_text TEXT,
                input_text TEXT NOT NULL,
                result_json JSONB,
                config_json JSONB NOT NULL
            )
            "#,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let job_id = Uuid::new_v4();
    sqlx::query(
            r#"
            INSERT INTO axon_embed_jobs (id, status, updated_at, input_text, result_json, config_json)
            VALUES ($1, 'running', NOW() - INTERVAL '20 minutes', 'integration-test', '{}'::jsonb, '{}'::jsonb)
            "#,
        )
        .bind(job_id)
        .execute(&pool)
        .await?;

    let first =
        reclaim_stale_running_jobs(&pool, JobTable::Embed, "embed", 300, 60, "test").await?;
    assert_eq!(first.stale_candidates, 1);
    assert_eq!(first.marked_candidates, 1);
    assert_eq!(first.reclaimed_jobs, 0);

    let marked_json: Value =
        sqlx::query_scalar("SELECT result_json FROM axon_embed_jobs WHERE id = $1")
            .bind(job_id)
            .fetch_one(&pool)
            .await?;
    assert!(marked_json.get("_watchdog").is_some());

    let second =
        reclaim_stale_running_jobs(&pool, JobTable::Embed, "embed", 300, 0, "test").await?;
    assert_eq!(second.reclaimed_jobs, 1);

    let status: String = sqlx::query_scalar("SELECT status FROM axon_embed_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(status, "failed");

    let _ = sqlx::query("DELETE FROM axon_embed_jobs WHERE id = $1")
        .bind(job_id)
        .execute(&pool)
        .await;
    Ok(())
}

#[tokio::test]
async fn claim_and_fail_lifecycle_transitions_are_state_guarded() -> Result<()> {
    let Some(pg_url) = resolve_test_pg_url() else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&pg_url)
        .await?;

    // Use the same advisory lock key as embed::ensure_schema (0xA804_0002) to avoid
    // concurrent-DDL races with the embed schema migration that runs in parallel tests.
    let mut tx = begin_schema_migration_tx(&pool, 0xA804_0002).await?;
    sqlx::query(
        r#"
            CREATE TABLE IF NOT EXISTS axon_embed_jobs (
                id UUID PRIMARY KEY,
                status TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                started_at TIMESTAMPTZ,
                finished_at TIMESTAMPTZ,
                error_text TEXT,
                input_text TEXT NOT NULL,
                result_json JSONB,
                config_json JSONB NOT NULL
            )
            "#,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let pending_id = Uuid::new_v4();
    let already_running_id = Uuid::new_v4();
    sqlx::query(
            "INSERT INTO axon_embed_jobs (id, status, input_text, config_json) VALUES ($1, 'pending', 'claim-test', '{}'::jsonb)",
        )
        .bind(pending_id)
        .execute(&pool)
        .await?;
    sqlx::query(
            "INSERT INTO axon_embed_jobs (id, status, input_text, config_json, started_at) VALUES ($1, 'running', 'run-test', '{}'::jsonb, NOW())",
        )
        .bind(already_running_id)
        .execute(&pool)
        .await?;

    assert!(claim_pending_by_id(&pool, JobTable::Embed, pending_id).await?);
    assert!(!claim_pending_by_id(&pool, JobTable::Embed, pending_id).await?);

    mark_job_failed(&pool, JobTable::Embed, pending_id, "synthetic-failure").await?;
    mark_job_failed(
        &pool,
        JobTable::Embed,
        already_running_id,
        "running-failure",
    )
    .await?;

    let pending_status: String =
        sqlx::query_scalar("SELECT status FROM axon_embed_jobs WHERE id=$1")
            .bind(pending_id)
            .fetch_one(&pool)
            .await?;
    let pending_error: Option<String> =
        sqlx::query_scalar("SELECT error_text FROM axon_embed_jobs WHERE id=$1")
            .bind(pending_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(pending_status, "failed");
    assert_eq!(pending_error.as_deref(), Some("synthetic-failure"));

    let running_status: String =
        sqlx::query_scalar("SELECT status FROM axon_embed_jobs WHERE id=$1")
            .bind(already_running_id)
            .fetch_one(&pool)
            .await?;
    let running_error: Option<String> =
        sqlx::query_scalar("SELECT error_text FROM axon_embed_jobs WHERE id=$1")
            .bind(already_running_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(running_status, "failed");
    assert_eq!(running_error.as_deref(), Some("running-failure"));

    let _ = sqlx::query("DELETE FROM axon_embed_jobs WHERE id = $1 OR id = $2")
        .bind(pending_id)
        .bind(already_running_id)
        .execute(&pool)
        .await;
    Ok(())
}
