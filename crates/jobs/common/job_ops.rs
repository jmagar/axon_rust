//! Job lifecycle operations: claim, mark failed.

use crate::crates::jobs::status::JobStatus;
use anyhow::{Context, Result};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::JobTable;

/// Atomically claim the next pending job from the given table.
/// Uses `FOR UPDATE SKIP LOCKED` for safe concurrent worker access.
pub async fn claim_next_pending(pool: &PgPool, table: JobTable) -> Result<Option<Uuid>> {
    let table = table.as_str();
    let query = format!(
        r#"WITH n AS (
            SELECT id FROM {table} WHERE status='{pending}' ORDER BY created_at ASC FOR UPDATE SKIP LOCKED LIMIT 1
        )
        UPDATE {table} j SET status='{running}', updated_at=NOW(), started_at=COALESCE(started_at, NOW())
        FROM n WHERE j.id=n.id RETURNING j.id"#,
        pending = JobStatus::Pending.as_str(),
        running = JobStatus::Running.as_str(),
    );
    let row = sqlx::query_as::<_, (Uuid,)>(&query)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(id,)| id))
}

/// Claim a specific pending job by ID.
pub async fn claim_pending_by_id(pool: &PgPool, table: JobTable, id: Uuid) -> Result<bool> {
    let table = table.as_str();
    let query = format!(
        "UPDATE {table} SET status='{running}', updated_at=NOW(), started_at=COALESCE(started_at, NOW()), error_text=NULL WHERE id=$1 AND status='{pending}'",
        running = JobStatus::Running.as_str(),
        pending = JobStatus::Pending.as_str(),
    );
    let updated = sqlx::query(&query)
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(updated > 0)
}

/// Mark a running job as failed with an error message.
///
/// # Errors
///
/// Returns `Err` if the database query fails (e.g. pool exhausted, connection dropped).
/// Callers that want fire-and-forget behavior can use
/// `.unwrap_or_else(|e| log_warn(&format!("mark_job_failed: {e}")))`.
pub async fn mark_job_failed(
    pool: &PgPool,
    table: JobTable,
    id: Uuid,
    error_text: &str,
) -> Result<()> {
    let table_name = table.as_str();
    let query = format!(
        "UPDATE {table_name} SET status='{failed}', updated_at=NOW(), finished_at=NOW(), error_text=$2 WHERE id=$1 AND status='{running}'",
        failed = JobStatus::Failed.as_str(),
        running = JobStatus::Running.as_str(),
    );
    sqlx::query(&query)
        .bind(id)
        .bind(error_text)
        .execute(pool)
        .await
        .with_context(|| format!("mark_job_failed for job {id} in {table_name}"))?;
    Ok(())
}

/// Mark a running job as completed.
///
/// If `result_json` is provided, it is written to the job row; otherwise the
/// existing result payload is preserved.
pub async fn mark_job_completed(
    pool: &PgPool,
    table: JobTable,
    id: Uuid,
    result_json: Option<&Value>,
) -> Result<bool> {
    let table_name = table.as_str();
    let query = format!(
        "UPDATE {table_name} \
         SET status='{completed}', updated_at=NOW(), finished_at=NOW(), error_text=NULL, result_json=COALESCE($2, result_json) \
         WHERE id=$1 AND status='{running}'",
        completed = JobStatus::Completed.as_str(),
        running = JobStatus::Running.as_str(),
    );
    let rows = sqlx::query(&query)
        .bind(id)
        .bind(result_json)
        .execute(pool)
        .await
        .with_context(|| format!("mark_job_completed for job {id} in {table_name}"))?
        .rows_affected();
    Ok(rows > 0)
}

/// Cancel a pending or running job.
pub async fn cancel_pending_or_running_job(
    pool: &PgPool,
    table: JobTable,
    id: Uuid,
) -> Result<bool> {
    let table_name = table.as_str();
    let query = format!(
        "UPDATE {table_name} \
         SET status='{canceled}', updated_at=NOW(), finished_at=NOW() \
         WHERE id=$1 AND status IN ('{pending}','{running}')",
        canceled = JobStatus::Canceled.as_str(),
        pending = JobStatus::Pending.as_str(),
        running = JobStatus::Running.as_str(),
    );
    let rows = sqlx::query(&query)
        .bind(id)
        .execute(pool)
        .await
        .with_context(|| format!("cancel_pending_or_running_job for job {id} in {table_name}"))?
        .rows_affected();
    Ok(rows > 0)
}

/// Touch a running job heartbeat by updating `updated_at`.
pub async fn touch_running_job(pool: &PgPool, table: JobTable, id: Uuid) -> Result<()> {
    let table_name = table.as_str();
    let query = format!(
        "UPDATE {table_name} SET updated_at=NOW() WHERE id=$1 AND status='{running}'",
        running = JobStatus::Running.as_str(),
    );
    sqlx::query(&query)
        .bind(id)
        .execute(pool)
        .await
        .with_context(|| format!("touch_running_job for job {id} in {table_name}"))?;
    Ok(())
}
