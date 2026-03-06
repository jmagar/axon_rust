//! Integration test for `spawn_heartbeat_task`.
//!
//! Verifies that the background heartbeat task advances `updated_at` on a
//! running job row within its configured tick interval. Skips automatically
//! when `AXON_TEST_PG_URL` is not set (no live Postgres available).

use super::super::*;
use serial_test::serial;
use tokio::time::Duration;
use uuid::Uuid;

#[tokio::test]
#[serial]
async fn spawn_heartbeat_task_advances_updated_at() -> Result<()> {
    let Some(pg_url) = resolve_test_pg_url() else {
        // No test database configured — skip gracefully.
        return Ok(());
    };

    let pool = match PgPoolOptions::new()
        .max_connections(2)
        .connect(&pg_url)
        .await
    {
        Ok(pool) => pool,
        Err(_) => return Ok(()),
    };

    // Ensure the embed table exists (idempotent DDL via advisory lock).
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

    // Insert a running job with updated_at pinned to 10 seconds ago so any
    // heartbeat tick will produce a measurably newer timestamp.
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO axon_embed_jobs \
         (id, status, started_at, updated_at, input_text, config_json) \
         VALUES ($1, 'running', NOW(), NOW() - INTERVAL '10 seconds', \
                 'heartbeat-test', '{}'::jsonb)",
    )
    .bind(id)
    .execute(&pool)
    .await?;

    // Capture baseline updated_at before the heartbeat fires.
    let baseline: chrono::DateTime<Utc> =
        sqlx::query_scalar("SELECT updated_at FROM axon_embed_jobs WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await?;

    // Spawn heartbeat with a 1-second interval so the test completes quickly.
    let (stop_tx, heartbeat) = spawn_heartbeat_task(pool.clone(), JobTable::Embed, id, 1);

    // Wait long enough for at least one tick to fire.
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Signal the heartbeat to stop and wait for the task to exit cleanly.
    let _ = stop_tx.send(true);
    let _ = heartbeat.await;

    // Read the updated timestamp back from the DB.
    let after: chrono::DateTime<Utc> =
        sqlx::query_scalar("SELECT updated_at FROM axon_embed_jobs WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await?;

    assert!(
        after > baseline,
        "spawn_heartbeat_task must advance updated_at: baseline={baseline}, after={after}"
    );

    // Clean up — best-effort, ignore errors.
    let _ = sqlx::query("DELETE FROM axon_embed_jobs WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await;

    Ok(())
}
