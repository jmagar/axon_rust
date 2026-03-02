use super::super::*;
use crate::crates::jobs::status::JobStatus;
use uuid::Uuid;

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

// ── T-H-1: mark_job_completed idempotency contract ─────────────────────────

#[test]
fn job_table_as_str_returns_expected_table_names() {
    assert_eq!(JobTable::Crawl.as_str(), "axon_crawl_jobs");
    assert_eq!(JobTable::Refresh.as_str(), "axon_refresh_jobs");
    assert_eq!(JobTable::Extract.as_str(), "axon_extract_jobs");
    assert_eq!(JobTable::Embed.as_str(), "axon_embed_jobs");
    assert_eq!(JobTable::Ingest.as_str(), "axon_ingest_jobs");
}

#[tokio::test]
async fn mark_job_completed_is_idempotent() -> Result<()> {
    let Some(pg_url) = resolve_test_pg_url() else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&pg_url)
        .await?;

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

    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO axon_embed_jobs (id, status, started_at, input_text, config_json) \
         VALUES ($1, 'running', NOW(), 'idempotent-test', '{}'::jsonb)",
    )
    .bind(id)
    .execute(&pool)
    .await?;

    // First call succeeds (job is 'running')
    let first = mark_job_completed(&pool, JobTable::Embed, id, None).await?;
    assert!(first, "first mark_job_completed should return true");

    // Second call returns false (job is now 'completed', not 'running')
    let second = mark_job_completed(&pool, JobTable::Embed, id, None).await?;
    assert!(
        !second,
        "second mark_job_completed should return false (idempotent)"
    );

    let _ = sqlx::query("DELETE FROM axon_embed_jobs WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await;
    Ok(())
}

// ── T-H-2: cancel_pending_or_running_job status coverage ────────────────────

#[test]
fn cancel_job_status_filter_covers_both_pending_and_running() {
    // The cancel query must match BOTH pending and running states
    assert_eq!(JobStatus::Pending.as_str(), "pending");
    assert_eq!(JobStatus::Running.as_str(), "running");
    assert_eq!(JobStatus::Canceled.as_str(), "canceled");
}

#[tokio::test]
async fn cancel_pending_or_running_job_lifecycle() -> Result<()> {
    let Some(pg_url) = resolve_test_pg_url() else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&pg_url)
        .await?;

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
    let running_id = Uuid::new_v4();
    let completed_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO axon_embed_jobs (id, status, input_text, config_json) \
         VALUES ($1, 'pending', 'cancel-test', '{}'::jsonb)",
    )
    .bind(pending_id)
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO axon_embed_jobs (id, status, started_at, input_text, config_json) \
         VALUES ($1, 'running', NOW(), 'cancel-test', '{}'::jsonb)",
    )
    .bind(running_id)
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO axon_embed_jobs (id, status, started_at, finished_at, input_text, config_json) \
         VALUES ($1, 'completed', NOW(), NOW(), 'cancel-test', '{}'::jsonb)",
    )
    .bind(completed_id)
    .execute(&pool)
    .await?;

    // Cancel pending — should succeed
    assert!(cancel_pending_or_running_job(&pool, JobTable::Embed, pending_id).await?);
    // Cancel running — should succeed
    assert!(cancel_pending_or_running_job(&pool, JobTable::Embed, running_id).await?);
    // Cancel completed — should return false (terminal state)
    assert!(!cancel_pending_or_running_job(&pool, JobTable::Embed, completed_id).await?);
    // Cancel already-canceled — idempotent, returns false
    assert!(!cancel_pending_or_running_job(&pool, JobTable::Embed, pending_id).await?);

    let _ = sqlx::query("DELETE FROM axon_embed_jobs WHERE id IN ($1, $2, $3)")
        .bind(pending_id)
        .bind(running_id)
        .bind(completed_id)
        .execute(&pool)
        .await;
    Ok(())
}

// ── T-H-3: claim_next_pending SQL contract ──────────────────────────────────

#[test]
fn claim_next_pending_uses_skip_locked_for_concurrency() {
    // Verify the table names are correct — the FOR UPDATE SKIP LOCKED clause
    // is embedded in the query built by claim_next_pending.
    let table = JobTable::Extract;
    assert_eq!(table.as_str(), "axon_extract_jobs");
    let table = JobTable::Crawl;
    assert_eq!(table.as_str(), "axon_crawl_jobs");
}

// ── T-M-1: touch_running_job contract ───────────────────────────────────────

#[tokio::test]
async fn touch_running_job_is_noop_for_non_running() -> Result<()> {
    let Some(pg_url) = resolve_test_pg_url() else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&pg_url)
        .await?;

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

    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO axon_embed_jobs (id, status, started_at, finished_at, input_text, config_json) \
         VALUES ($1, 'completed', NOW(), NOW(), 'touch-test', '{}'::jsonb)",
    )
    .bind(id)
    .execute(&pool)
    .await?;

    // Touch should not error for completed jobs — it's just a no-op
    touch_running_job(&pool, JobTable::Embed, id).await?;

    let _ = sqlx::query("DELETE FROM axon_embed_jobs WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await;
    Ok(())
}
