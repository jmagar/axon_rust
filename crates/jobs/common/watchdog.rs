//! Stale job watchdog: two-pass confirmation model for reclaiming stuck jobs.

use crate::crates::jobs::status::JobStatus;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::JobTable;

#[derive(Debug, Clone, Default)]
pub struct WatchdogSweepStats {
    pub stale_candidates: u64,
    pub marked_candidates: u64,
    pub reclaimed_jobs: u64,
    pub reclaimed_ids: Vec<Uuid>,
}

pub(crate) fn stale_watchdog_payload(
    mut result_json: Value,
    observed_updated_at: DateTime<Utc>,
) -> Value {
    if !result_json.is_object() {
        result_json = serde_json::json!({});
    }
    if let Some(obj) = result_json.as_object_mut() {
        // Preserve first_seen_stale_at if already set for the same observed_updated_at,
        // so the confirmation timer isn't reset on every sweep.
        let existing_first_seen = obj
            .get("_watchdog")
            .and_then(|w| {
                let same_observed = w.get("observed_updated_at").and_then(|v| v.as_str())
                    == Some(observed_updated_at.to_rfc3339().as_str());
                if same_observed {
                    w.get("first_seen_stale_at")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        obj.insert(
            "_watchdog".to_string(),
            serde_json::json!({
                "first_seen_stale_at": existing_first_seen,
                "observed_updated_at": observed_updated_at.to_rfc3339(),
            }),
        );
    }
    result_json
}

pub(crate) fn stale_watchdog_confirmed(
    result_json: &Value,
    observed_updated_at: DateTime<Utc>,
    confirm_secs: i64,
) -> bool {
    let Some(watchdog) = result_json.get("_watchdog") else {
        return false;
    };
    let Some(observed) = watchdog
        .get("observed_updated_at")
        .and_then(|value| value.as_str())
    else {
        return false;
    };
    if observed != observed_updated_at.to_rfc3339() {
        return false;
    }
    let Some(first_seen) = watchdog
        .get("first_seen_stale_at")
        .and_then(|value| value.as_str())
    else {
        return false;
    };
    let Ok(first_seen_at) = DateTime::parse_from_rfc3339(first_seen) else {
        return false;
    };
    let elapsed = Utc::now()
        .signed_duration_since(first_seen_at.with_timezone(&Utc))
        .num_seconds();
    elapsed >= confirm_secs
}

/// Reclaim stale running jobs with a two-pass confirmation model.
///
/// Pass 1: mark stale candidate in `result_json._watchdog` with observed `updated_at`.
/// Pass 2: if still stale and unchanged after `confirm_secs`, mark as failed.
pub async fn reclaim_stale_running_jobs(
    pool: &PgPool,
    table: JobTable,
    job_kind: &str,
    idle_timeout_secs: i64,
    confirm_secs: i64,
    marker: &str,
) -> Result<WatchdogSweepStats> {
    let select_query = format!(
        r#"
        SELECT id, updated_at, result_json
        FROM {}
        WHERE status = '{running}'
          AND updated_at < NOW() - make_interval(secs => $1::int)
        ORDER BY updated_at ASC
        LIMIT 50
        "#,
        table.as_str(),
        running = JobStatus::Running.as_str(),
    );
    let rows = sqlx::query(&select_query)
        .bind(idle_timeout_secs.min(i32::MAX as i64) as i32)
        .fetch_all(pool)
        .await?;

    let mut stats = WatchdogSweepStats {
        stale_candidates: rows.len() as u64,
        ..Default::default()
    };
    for row in rows {
        let id: Uuid = row.try_get("id")?;
        let updated_at: DateTime<Utc> = row.try_get("updated_at")?;
        let result_json: Option<Value> = row.try_get("result_json")?;
        let current_json = result_json.unwrap_or_else(|| serde_json::json!({}));
        let idle_seconds = Utc::now().signed_duration_since(updated_at).num_seconds();

        if stale_watchdog_confirmed(&current_json, updated_at, confirm_secs) {
            let msg = format!(
                "watchdog reclaimed stale running {} job (idle={}s marker={})",
                job_kind, idle_seconds, marker
            );
            let fail_query = format!(
                "UPDATE {} SET status='{failed}', updated_at=NOW(), finished_at=NOW(), error_text=$2 WHERE id=$1 AND status='{running}'",
                table.as_str(),
                failed = JobStatus::Failed.as_str(),
                running = JobStatus::Running.as_str(),
            );
            let affected = sqlx::query(&fail_query)
                .bind(id)
                .bind(msg)
                .execute(pool)
                .await?
                .rows_affected();
            stats.reclaimed_jobs += affected;
            if affected > 0 {
                stats.reclaimed_ids.push(id);
            }
            continue;
        }

        let marked = stale_watchdog_payload(current_json, updated_at);
        let mark_query = format!(
            "UPDATE {} SET result_json=$2 WHERE id=$1 AND status='{running}'",
            table.as_str(),
            running = JobStatus::Running.as_str()
        );
        let _ = sqlx::query(&mark_query)
            .bind(id)
            .bind(marked)
            .execute(pool)
            .await?;
        stats.marked_candidates += 1;
    }

    Ok(stats)
}
