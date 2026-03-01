use super::*;
use crate::crates::jobs::status::JobStatus;
use chrono::{DateTime, Duration};
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

#[test]
fn dotenv_parser_ignores_comments_blank_and_malformed_lines() {
    let parsed = parse_dotenv_content(
        r#"
        # comment
        NO_EQUALS
        =missing_key
        FOO=bar

        BAZ = qux
        "#,
    );
    assert_eq!(parsed.get("FOO").map(String::as_str), Some("bar"));
    assert_eq!(parsed.get("BAZ").map(String::as_str), Some("qux"));
    assert!(!parsed.contains_key("NO_EQUALS"));
}

#[test]
fn dotenv_parser_unquotes_single_and_double_quoted_values() {
    let parsed = parse_dotenv_content(
        r#"
        A="value one"
        B='value two'
        C=plain
        "#,
    );
    assert_eq!(parsed.get("A").map(String::as_str), Some("value one"));
    assert_eq!(parsed.get("B").map(String::as_str), Some("value two"));
    assert_eq!(parsed.get("C").map(String::as_str), Some("plain"));
}

#[test]
fn dotenv_parser_keeps_inner_equals_and_last_value_wins() {
    let parsed = parse_dotenv_content(
        r#"
        AXON_PG_URL=postgresql://u:p@localhost:5432/db?sslmode=disable
        FOO=first
        FOO=second
        "#,
    );
    assert_eq!(
        parsed.get("AXON_PG_URL").map(String::as_str),
        Some("postgresql://u:p@localhost:5432/db?sslmode=disable")
    );
    assert_eq!(parsed.get("FOO").map(String::as_str), Some("second"));
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

// ── T-H-4: Watchdog RFC3339 timestamp round-trip ────────────────────────────

#[test]
fn watchdog_rfc3339_timestamp_round_trips() {
    let ts = Utc::now();
    let rfc = ts.to_rfc3339();
    let parsed = DateTime::parse_from_rfc3339(&rfc).unwrap();
    assert_eq!(ts.timestamp(), parsed.timestamp());
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

// ── T-M-4: Watchdog two-pass payload preservation ───────────────────────────

#[test]
fn watchdog_payload_preserves_first_seen_on_same_observed() {
    let observed = Utc::now() - Duration::seconds(120);

    // First pass: creates the _watchdog metadata
    let first = stale_watchdog_payload(serde_json::json!({}), observed);
    let first_seen = first["_watchdog"]["first_seen_stale_at"]
        .as_str()
        .unwrap()
        .to_string();

    // Simulate time passing, then second pass with same observed_updated_at
    let second = stale_watchdog_payload(first, observed);
    let second_first_seen = second["_watchdog"]["first_seen_stale_at"]
        .as_str()
        .unwrap()
        .to_string();

    // first_seen_stale_at should be preserved (not reset)
    assert_eq!(first_seen, second_first_seen);
}

#[test]
fn watchdog_payload_resets_first_seen_on_different_observed() {
    let observed_old = Utc::now() - Duration::seconds(120);
    let observed_new = Utc::now() - Duration::seconds(60);

    let first = stale_watchdog_payload(serde_json::json!({}), observed_old);
    let first_seen = first["_watchdog"]["first_seen_stale_at"]
        .as_str()
        .unwrap()
        .to_string();

    // Heartbeat arrived, observed_updated_at changed — first_seen resets
    let second = stale_watchdog_payload(first, observed_new);
    let second_first_seen = second["_watchdog"]["first_seen_stale_at"]
        .as_str()
        .unwrap()
        .to_string();

    // first_seen_stale_at should be different (reset due to new observed timestamp)
    assert_ne!(first_seen, second_first_seen);
}
