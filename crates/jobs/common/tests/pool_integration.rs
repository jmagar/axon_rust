use super::super::*;
use chrono::Utc;
use uuid::Uuid;

// ── make_pool_creates_pool_and_executes_ping ────────────────────────────────

#[tokio::test]
async fn make_pool_creates_pool_and_executes_ping() -> Result<()> {
    let Some(pg_url) = resolve_test_pg_url() else {
        return Ok(());
    };
    let cfg = test_config(&pg_url);

    let pool = make_pool(&cfg).await?;

    // Postgres literal `1` is INT4; cast explicitly to avoid type mismatch.
    let result: i64 = sqlx::query_scalar("SELECT 1::int8 AS ping")
        .fetch_one(&pool)
        .await?;

    assert_eq!(result, 1i64);
    Ok(())
}

// ── claim_next_pending_claims_oldest_job_first ─────────────────────────────

#[tokio::test]
async fn claim_next_pending_claims_oldest_job_first() -> Result<()> {
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

    let id_old = Uuid::new_v4();
    let id_mid = Uuid::new_v4();
    let id_new = Uuid::new_v4();

    // Insert oldest first (T-3s), then T-2s, then T-1s.
    sqlx::query(
        "INSERT INTO axon_embed_jobs (id, status, input_text, config_json, created_at) \
         VALUES ($1, 'pending', 'fifo-test', '{}'::jsonb, $2)",
    )
    .bind(id_old)
    .bind(Utc::now() - chrono::Duration::seconds(3))
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO axon_embed_jobs (id, status, input_text, config_json, created_at) \
         VALUES ($1, 'pending', 'fifo-test', '{}'::jsonb, $2)",
    )
    .bind(id_mid)
    .bind(Utc::now() - chrono::Duration::seconds(2))
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO axon_embed_jobs (id, status, input_text, config_json, created_at) \
         VALUES ($1, 'pending', 'fifo-test', '{}'::jsonb, $2)",
    )
    .bind(id_new)
    .bind(Utc::now() - chrono::Duration::seconds(1))
    .execute(&pool)
    .await?;

    let first = claim_next_pending(&pool, JobTable::Embed).await?;
    let second = claim_next_pending(&pool, JobTable::Embed).await?;
    let third = claim_next_pending(&pool, JobTable::Embed).await?;

    assert_eq!(
        first,
        Some(id_old),
        "first claim should return the oldest job"
    );
    assert_eq!(
        second,
        Some(id_mid),
        "second claim should return the middle job"
    );
    assert_eq!(
        third,
        Some(id_new),
        "third claim should return the newest job"
    );

    let _ = sqlx::query("DELETE FROM axon_embed_jobs WHERE id IN ($1, $2, $3)")
        .bind(id_old)
        .bind(id_mid)
        .bind(id_new)
        .execute(&pool)
        .await;
    Ok(())
}

// ── count_stale_and_pending_jobs_with_pool_returns_zero_for_empty_tables ───

#[tokio::test]
async fn count_stale_and_pending_jobs_with_pool_returns_zero_for_empty_tables() -> Result<()> {
    let Some(pg_url) = resolve_test_pg_url() else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&pg_url)
        .await?;

    // Create axon_crawl_jobs
    let mut tx = begin_schema_migration_tx(&pool, 0xA804_0001i64).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS axon_crawl_jobs (
            id UUID PRIMARY KEY,
            status TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            started_at TIMESTAMPTZ,
            finished_at TIMESTAMPTZ,
            error_text TEXT,
            url TEXT NOT NULL,
            result_json JSONB,
            config_json JSONB NOT NULL
        )
        "#,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    // Create axon_embed_jobs
    let mut tx = begin_schema_migration_tx(&pool, 0xA804_0002i64).await?;
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

    // Create axon_extract_jobs
    let mut tx = begin_schema_migration_tx(&pool, 0xA804_0003i64).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS axon_extract_jobs (
            id UUID PRIMARY KEY,
            status TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            started_at TIMESTAMPTZ,
            finished_at TIMESTAMPTZ,
            error_text TEXT,
            urls_json JSONB NOT NULL,
            result_json JSONB,
            config_json JSONB NOT NULL
        )
        "#,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    // Create axon_ingest_jobs
    let mut tx = begin_schema_migration_tx(&pool, 0xA804_0004i64).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS axon_ingest_jobs (
            id UUID PRIMARY KEY,
            status TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            started_at TIMESTAMPTZ,
            finished_at TIMESTAMPTZ,
            error_text TEXT,
            source_type TEXT NOT NULL,
            target TEXT NOT NULL,
            result_json JSONB,
            config_json JSONB NOT NULL
        )
        "#,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let result = count_stale_and_pending_jobs_with_pool(&pool, 5).await;

    assert_eq!(
        result,
        Some((0, 0)),
        "expected (0 stale, 0 pending) for empty tables"
    );
    Ok(())
}
