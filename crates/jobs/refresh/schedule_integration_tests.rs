//! Integration tests for `claim_due_refresh_schedules_with_pool`.
//!
//! Exercises the core SQL query against a live Postgres database.
//! Skips gracefully when `AXON_TEST_PG_URL` is not set.

use super::*;
use crate::crates::jobs::common::{make_pool, resolve_test_pg_url};
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use std::error::Error;
use uuid::Uuid;

/// Insert a minimal `axon_refresh_schedules` row directly via SQL.
/// Returns the inserted `id`.
async fn insert_schedule_row(
    pool: &PgPool,
    name: &str,
    enabled: bool,
    next_run_at: DateTime<Utc>,
) -> Result<Uuid, Box<dyn Error>> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO axon_refresh_schedules
            (id, name, every_seconds, enabled, next_run_at)
        VALUES ($1, $2, 60, $3, $4)
        "#,
    )
    .bind(id)
    .bind(name)
    .bind(enabled)
    .bind(next_run_at)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Delete test rows by name to clean up after each test.
async fn delete_schedule_by_name(pool: &PgPool, name: &str) -> Result<(), Box<dyn Error>> {
    sqlx::query("DELETE FROM axon_refresh_schedules WHERE name = $1")
        .bind(name)
        .execute(pool)
        .await?;
    Ok(())
}

/// Asserts that `claim_due_refresh_schedules_with_pool` returns exactly the
/// 2 enabled+due rows and excludes the not-yet-due row.
///
/// Row fixture:
///   - Row A: `next_run_at = NOW() - 2 min`  → due, enabled   → claimed
///   - Row B: `next_run_at = NOW() - 1 min`  → due, enabled   → claimed
///   - Row C: `next_run_at = NOW() + 1 hour` → not due        → NOT claimed
#[tokio::test]
async fn claim_due_returns_exactly_two_due_rows_and_excludes_future_row()
-> Result<(), Box<dyn Error>> {
    let Some(pg_url) = resolve_test_pg_url() else {
        return Ok(());
    };

    // Use test_config so make_pool picks up the right connection string.
    let cfg = crate::crates::jobs::common::test_config(&pg_url);
    let pool = match make_pool(&cfg).await {
        Ok(pool) => pool,
        Err(_) => return Ok(()),
    };
    ensure_schema(&pool).await?;

    let now = Utc::now();
    let suffix = Uuid::new_v4();
    let name_a = format!("it-due-2m-{suffix}");
    let name_b = format!("it-due-1m-{suffix}");
    let name_c = format!("it-future-1h-{suffix}");

    // Insert 3 controlled rows.
    let id_a = insert_schedule_row(&pool, &name_a, true, now - Duration::minutes(2)).await?;
    let id_b = insert_schedule_row(&pool, &name_b, true, now - Duration::minutes(1)).await?;
    let _id_c = insert_schedule_row(&pool, &name_c, true, now + Duration::hours(1)).await?;

    // Run assertions in a closure so cleanup runs even on panic.
    let result = async {
        // Claim with a high limit so we don't accidentally cap at 1.
        let claimed = claim_due_refresh_schedules_with_pool(&pool, 1_000).await?;

        // Filter claimed IDs to only those belonging to this test (parallel tests may claim rows).
        let our_claimed: Vec<Uuid> = claimed
            .iter()
            .filter(|s| s.id == id_a || s.id == id_b)
            .map(|s| s.id)
            .collect();

        assert_eq!(
            our_claimed.len(),
            2,
            "Expected exactly 2 due rows to be claimed (got {our_claimed:?})"
        );
        assert!(
            our_claimed.contains(&id_a),
            "Row A (due 2 min ago) must be in claimed set"
        );
        assert!(
            our_claimed.contains(&id_b),
            "Row B (due 1 min ago) must be in claimed set"
        );

        // Confirm the not-due row was NOT returned.
        let future_in_claimed = claimed.iter().any(|s| s.name == name_c);
        assert!(
            !future_in_claimed,
            "Row C (due in 1 hour) must NOT appear in claimed set"
        );

        // Also verify row C is still in the future via direct DB read.
        let row_c: RefreshSchedule =
            sqlx::query_as("SELECT * FROM axon_refresh_schedules WHERE name = $1")
                .bind(&name_c)
                .fetch_one(&pool)
                .await?;
        assert!(
            row_c.next_run_at > now,
            "Row C next_run_at must remain in the future after claim"
        );

        Ok::<(), Box<dyn Error>>(())
    }
    .await;

    // Cleanup runs regardless of assertion outcome.
    let _ = delete_schedule_by_name(&pool, &name_a).await;
    let _ = delete_schedule_by_name(&pool, &name_b).await;
    let _ = delete_schedule_by_name(&pool, &name_c).await;

    result
}
