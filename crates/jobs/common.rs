use crate::crates::core::config::Config;
use crate::crates::core::content::redact_url;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use lapin::options::QueueDeclareOptions;
use lapin::types::FieldTable;
use lapin::{Channel, Connection, ConnectionProperties};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use std::time::Duration;
use tokio;
use tokio_executor_trait::Tokio as TokioExecutor;
use tokio_reactor_trait::Tokio as TokioReactor;
use uuid::Uuid;

fn durable_queue_options() -> QueueDeclareOptions {
    QueueDeclareOptions {
        durable: true,
        auto_delete: false,
        exclusive: false,
        nowait: false,
        passive: false,
    }
}

#[derive(Debug, Clone, Copy)]
pub enum JobTable {
    Crawl,
    Extract,
    Embed,
    Ingest,
}

impl JobTable {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Crawl => "axon_crawl_jobs",
            Self::Extract => "axon_extract_jobs",
            Self::Embed => "axon_embed_jobs",
            Self::Ingest => "axon_ingest_jobs",
        }
    }
}

#[cfg(test)]
pub(crate) fn test_config(pg_url: &str) -> Config {
    use std::path::PathBuf;
    Config {
        pg_url: pg_url.to_string(),
        redis_url: "redis://127.0.0.1:1".to_string(),
        amqp_url: "amqp://guest:guest@127.0.0.1:1/%2f".to_string(),
        collection: "test".to_string(),
        output_dir: PathBuf::from(".cache/test-worker-jobs"),
        crawl_queue: "axon.test.crawl".to_string(),
        extract_queue: "axon.test.extract".to_string(),
        embed_queue: "axon.test.embed".to_string(),
        ingest_queue: "axon.test.ingest".to_string(),
        tei_url: "http://127.0.0.1:1".to_string(),
        qdrant_url: "http://127.0.0.1:1".to_string(),
        openai_base_url: "http://127.0.0.1:1/v1".to_string(),
        openai_api_key: "test".to_string(),
        openai_model: "test-model".to_string(),
        // Test-specific overrides from the original literal
        search_limit: 5,
        max_pages: 10,
        max_depth: 2,
        min_markdown_chars: 50,
        drop_thin_markdown: false,
        embed: false,
        batch_concurrency: 2,
        yes: true,
        crawl_concurrency_limit: Some(2),
        backfill_concurrency_limit: Some(2),
        request_timeout_ms: Some(5_000),
        fetch_retries: 0,
        retry_backoff_ms: 0,
        ask_max_context_chars: 12_000,
        ..Config::default()
    }
}

/// Create a shared PgPool with a 5-second connection timeout and up to 5 connections.
/// Call once at startup and pass the pool to all functions.
pub async fn make_pool(cfg: &Config) -> Result<PgPool> {
    let p = tokio::time::timeout(
        Duration::from_secs(5),
        PgPoolOptions::new().max_connections(5).connect(&cfg.pg_url),
    )
    .await
    .map_err(|_| {
        anyhow::anyhow!(
            "postgres connect timeout: {} (if running in Docker without published ports, run from same Docker network or expose postgres)",
            redact_url(&cfg.pg_url)
        )
    })?
    .context("postgres connect failed")?;
    Ok(p)
}

/// Open an AMQP channel with a 5-second connection timeout and declare the given queue.
///
/// **Warning:** This drops the `Connection`, so the returned channel's backing TCP
/// connection will close asynchronously. Only use this for short-lived operations
/// (health checks, queue_purge). For long-lived consumers, use
/// `open_amqp_connection_and_channel` and keep the `Connection` in scope.
pub async fn open_amqp_channel(cfg: &Config, queue_name: &str) -> Result<Channel> {
    let (_, ch) = open_amqp_connection_and_channel(cfg, queue_name).await?;
    Ok(ch)
}

pub(crate) async fn open_amqp_connection_and_channel(
    cfg: &Config,
    queue_name: &str,
) -> Result<(Connection, Channel)> {
    let props = ConnectionProperties::default()
        .with_executor(TokioExecutor::current())
        .with_reactor(TokioReactor);
    let conn = tokio::time::timeout(
        Duration::from_secs(5),
        Connection::connect(&cfg.amqp_url, props),
    )
    .await
    .map_err(|_| {
        anyhow::anyhow!(
            "amqp connect timeout: {} (if running in Docker without published ports, run from same Docker network or expose rabbitmq)",
            redact_url(&cfg.amqp_url)
        )
    })?
    .context("amqp connect failed")?;
    let ch = tokio::time::timeout(Duration::from_secs(5), async {
        let ch = conn.create_channel().await?;
        ch.queue_declare(queue_name, durable_queue_options(), FieldTable::default())
            .await?;
        Ok::<Channel, lapin::Error>(ch)
    })
    .await
    .map_err(|_| anyhow::anyhow!("amqp channel/queue declare timeout for queue={queue_name}"))?
    .context("amqp create channel/declare queue failed")?;
    Ok((conn, ch))
}

/// Atomically claim the next pending job from the given table.
/// Uses `FOR UPDATE SKIP LOCKED` for safe concurrent worker access.
pub async fn claim_next_pending(pool: &PgPool, table: JobTable) -> Result<Option<Uuid>> {
    let table = table.as_str();
    let query = format!(
        r#"WITH n AS (
            SELECT id FROM {table} WHERE status='pending' ORDER BY created_at ASC FOR UPDATE SKIP LOCKED LIMIT 1
        )
        UPDATE {table} j SET status='running', updated_at=NOW(), started_at=COALESCE(started_at, NOW())
        FROM n WHERE j.id=n.id RETURNING j.id"#
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
        "UPDATE {table} SET status='running', updated_at=NOW(), started_at=COALESCE(started_at, NOW()), error_text=NULL WHERE id=$1 AND status='pending'"
    );
    let updated = sqlx::query(&query)
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(updated > 0)
}

/// Mark a running job as failed with an error message.
pub async fn mark_job_failed(pool: &PgPool, table: JobTable, id: Uuid, error_text: &str) {
    use crate::crates::core::logging::log_warn;
    let table_name = table.as_str();
    let query = format!(
        "UPDATE {table_name} SET status='failed', updated_at=NOW(), finished_at=NOW(), error_text=$2 WHERE id=$1 AND status='running'"
    );
    if let Err(err) = sqlx::query(&query)
        .bind(id)
        .bind(error_text)
        .execute(pool)
        .await
    {
        log_warn(&format!(
            "mark_job_failed db error for job {id} in {table_name}: {err}"
        ));
    }
}

/// Publish a job ID to an AMQP queue.
pub async fn enqueue_job(cfg: &Config, queue_name: &str, job_id: Uuid) -> Result<()> {
    use lapin::options::BasicPublishOptions;
    use lapin::BasicProperties;

    let (conn, ch) = open_amqp_connection_and_channel(cfg, queue_name).await?;

    let payload = job_id.to_string();
    ch.basic_publish(
        "",
        queue_name,
        BasicPublishOptions::default(),
        payload.as_bytes(),
        BasicProperties::default(),
    )
    .await?
    .await?;

    // Explicitly close channel then connection so lapin's AMQP CLOSE handshake
    // completes synchronously. Without this, lapin defers cleanup to background
    // tokio tasks that race with #[tokio::main] shutdown.
    // Using ch.close() instead of drop(ch) avoids the "invalid channel state: Closing"
    // log noise that occurs when conn.close() races with an in-flight channel-close frame.
    let _ = ch.close(0, "").await;
    let _ = conn.close(200, "").await;

    Ok(())
}

/// Publish multiple job IDs to an AMQP queue over a single connection.
///
/// More efficient than calling [`enqueue_job`] in a loop — one TCP handshake,
/// N publishes, one CLOSE. Uses publisher confirms so the broker acks every
/// message before we close — follows the official lapin `publisher_confirms` example.
pub async fn batch_enqueue_jobs(cfg: &Config, queue_name: &str, job_ids: &[Uuid]) -> Result<()> {
    use lapin::options::{BasicPublishOptions, ConfirmSelectOptions};
    use lapin::BasicProperties;

    if job_ids.is_empty() {
        return Ok(());
    }

    let (conn, ch) = open_amqp_connection_and_channel(cfg, queue_name).await?;

    // Enable publisher confirms so wait_for_confirms actually tracks acks.
    ch.confirm_select(ConfirmSelectOptions::default())
        .await
        .context("confirm_select failed")?;

    for id in job_ids {
        ch.basic_publish(
            "",
            queue_name,
            BasicPublishOptions::default(),
            id.to_string().as_bytes(),
            BasicProperties::default(),
        )
        .await?;
        // Don't await the confirm here — collect them all at once below.
    }

    // Wait for all broker acks in one pass instead of awaiting each publish individually.
    ch.wait_for_confirms()
        .await
        .context("wait_for_confirms failed")?;
    let _ = ch.close(0, "").await;
    let _ = conn.close(200, "").await;

    Ok(())
}

/// Purge all messages from the named AMQP queue, then explicitly close the
/// channel and connection.
///
/// This is the correct way to purge a queue — unlike [`open_amqp_channel`], it
/// keeps the `Connection` alive for the full duration of the operation.
pub(crate) async fn purge_queue_safe(cfg: &Config, queue_name: &str) -> Result<()> {
    use lapin::options::QueuePurgeOptions;

    let (conn, ch) = open_amqp_connection_and_channel(cfg, queue_name).await?;
    ch.queue_purge(queue_name, QueuePurgeOptions::default())
        .await
        .context("queue_purge failed")?;
    let _ = ch.close(0, "").await;
    let _ = conn.close(200, "").await;
    Ok(())
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WatchdogSweepStats {
    pub stale_candidates: u64,
    pub marked_candidates: u64,
    pub reclaimed_jobs: u64,
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
        WHERE status = 'running'
          AND updated_at < NOW() - make_interval(secs => $1::int)
        ORDER BY updated_at ASC
        LIMIT 50
        "#,
        table.as_str()
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
                "UPDATE {} SET status='failed', updated_at=NOW(), finished_at=NOW(), error_text=$2 WHERE id=$1 AND status='running'",
                table.as_str()
            );
            let affected = sqlx::query(&fail_query)
                .bind(id)
                .bind(msg)
                .execute(pool)
                .await?
                .rows_affected();
            stats.reclaimed_jobs += affected;
            continue;
        }

        let marked = stale_watchdog_payload(current_json, updated_at);
        let mark_query = format!(
            "UPDATE {} SET result_json=$2 WHERE id=$1 AND status='running'",
            table.as_str()
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

/// Count jobs stuck in `running` state beyond `stale_minutes` and jobs in `pending` state,
/// across all five job tables. Returns `(stale, pending)` counts, or `None` if Postgres
/// is unreachable.
pub async fn count_stale_and_pending_jobs(cfg: &Config, stale_minutes: i64) -> Option<(i64, i64)> {
    let pool = match make_pool(cfg).await {
        Ok(p) => p,
        Err(_) => return None,
    };

    let query = r#"
        WITH all_jobs AS (
            SELECT status, started_at FROM axon_crawl_jobs
            UNION ALL
            SELECT status, started_at FROM axon_extract_jobs
            UNION ALL
            SELECT status, started_at FROM axon_embed_jobs
            UNION ALL
            SELECT status, started_at FROM axon_ingest_jobs
        )
        SELECT
            COUNT(*) FILTER (
                WHERE status = 'running'
                  AND started_at < NOW() - make_interval(mins => $1::int)
            ) AS stale,
            COUNT(*) FILTER (WHERE status = 'pending') AS pending
        FROM all_jobs
    "#;

    let stale_mins = stale_minutes.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    match sqlx::query_as::<_, (i64, i64)>(query)
        .bind(stale_mins)
        .fetch_one(&pool)
        .await
    {
        Ok((stale, pending)) => Some((stale, pending)),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests;
