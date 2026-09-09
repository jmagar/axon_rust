use axon_api::source::{ApiError, ErrorStage, JobId, SourceWarning};
use axon_core::sqlite::ImmediateTx;
use sqlx::Row;
use std::collections::HashSet;

use crate::boundary::Result;
use crate::unified_codec::{from_json, missing_job, sql_error, to_json};

/// Decode warning payloads without writer admission, then fence the prepared
/// snapshot against both event sequence and job attempt under BEGIN IMMEDIATE.
/// Events from every attempt remain included, matching the lifetime summary.
pub(super) async fn prepare_status_write(
    store: &super::SqliteUnifiedJobStore,
    job_id: JobId,
    terminal: bool,
) -> Result<(ImmediateTx, sqlx::sqlite::SqliteRow, Option<String>)> {
    const SNAPSHOT: &str = "SELECT status, started_at, warnings_json, attempt, last_event_sequence FROM jobs WHERE job_id = ?";
    loop {
        let prepared = if terminal {
            let row = sqlx::query(SNAPSHOT)
                .bind(job_id.0.to_string())
                .fetch_optional(&store.pool)
                .await
                .map_err(sql_error)?
                .ok_or_else(|| missing_job(job_id))?;
            let sequence: i64 = row.get("last_event_sequence");
            let attempt: i64 = row.get("attempt");
            let existing: String = row.get("warnings_json");
            let warnings =
                collect_terminal_warnings(&store.pool, job_id, existing.clone(), sequence).await?;
            Some((sequence, attempt, existing, warnings))
        } else {
            None
        };
        let mut tx = ImmediateTx::begin_with_gate(&store.pool, &store.write_gate)
            .await
            .map_err(sql_error)?;
        let row = sqlx::query(SNAPSHOT)
            .bind(job_id.0.to_string())
            .fetch_optional(&mut *tx)
            .await
            .map_err(sql_error)?
            .ok_or_else(|| missing_job(job_id))?;
        if let Some((sequence, attempt, existing, warnings)) = prepared {
            if sequence != row.get::<i64, _>("last_event_sequence")
                || attempt != row.get::<i64, _>("attempt")
                || existing != row.get::<String, _>("warnings_json")
            {
                // Never decode a racing tail under the writer lock. Retry from
                // a new bounded read snapshot; dropping also rolls back safely.
                tx.rollback().await;
                continue;
            }
            return Ok((tx, row, Some(warnings)));
        }
        return Ok((tx, row, None));
    }
}

pub(super) async fn collect_terminal_warnings(
    pool: &sqlx::SqlitePool,
    job_id: JobId,
    existing_json: String,
    through_sequence: i64,
) -> Result<String> {
    #[cfg(test)]
    tests::pause_once(job_id).await;
    let mut warnings = from_json::<Vec<SourceWarning>>(existing_json)?;
    let mut seen = warnings
        .iter()
        .map(to_json)
        .collect::<Result<HashSet<_>>>()?;
    let mut cursor = 0_i64;
    loop {
        // Return only the warning projection, never complete progress payloads.
        // A bounded keyset page avoids materializing lifetime event history.
        let rows = sqlx::query(
            "SELECT sequence, json_extract(details_json, '$.source_progress_event.warning') AS warning
             FROM job_events WHERE job_id = ? AND sequence > ? AND sequence <= ?
               AND json_type(details_json, '$.source_progress_event.warning') IS NOT NULL
               AND json_type(details_json, '$.source_progress_event.warning') <> 'null'
             ORDER BY sequence ASC LIMIT 128")
            .bind(job_id.0.to_string())
            .bind(cursor)
            .bind(through_sequence)
            .fetch_all(pool)
            .await
            .map_err(sql_error)?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            cursor = row.get("sequence");
            let warning = serde_json::from_str::<SourceWarning>(&row.get::<String, _>("warning"))
                .map_err(|error| {
                ApiError::new(
                    "job.warning_decode_failed",
                    ErrorStage::Publishing,
                    format!("decode redacted job warning: {error}"),
                )
            })?;
            if seen.insert(to_json(&warning)?) {
                warnings.push(warning);
            }
        }
    }
    to_json(&warnings)
}

#[cfg(test)]
#[path = "terminal_warnings_tests.rs"]
pub(super) mod tests;
