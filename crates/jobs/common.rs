use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::content::redact_url;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use lapin::options::QueueDeclareOptions;
use lapin::types::FieldTable;
use lapin::{Channel, Connection, ConnectionProperties};
use serde_json::Value;
use spider::tokio;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use std::time::Duration;
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
    Batch,
    Extract,
    Embed,
}

impl JobTable {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Crawl => "axon_crawl_jobs",
            Self::Batch => "axon_batch_jobs",
            Self::Extract => "axon_extract_jobs",
            Self::Embed => "axon_embed_jobs",
        }
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
pub async fn open_amqp_channel(cfg: &Config, queue_name: &str) -> Result<Channel> {
    let (_, ch) = open_amqp_connection_and_channel(cfg, queue_name).await?;
    Ok(ch)
}

async fn open_amqp_connection_and_channel(
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
    let table = table.as_str();
    let query = format!(
        "UPDATE {table} SET status='failed', updated_at=NOW(), finished_at=NOW(), error_text=$2 WHERE id=$1 AND status='running'"
    );
    let _ = sqlx::query(&query)
        .bind(id)
        .bind(error_text)
        .execute(pool)
        .await;
}

/// Publish a job ID to an AMQP queue.
pub async fn enqueue_job(cfg: &Config, queue_name: &str, job_id: Uuid) -> Result<()> {
    use lapin::options::BasicPublishOptions;
    use lapin::BasicProperties;

    let (_conn, ch) = open_amqp_connection_and_channel(cfg, queue_name).await?;

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

    Ok(())
}

fn stale_watchdog_payload(
    mut result_json: Value,
    observed_updated_at: DateTime<Utc>,
) -> Value {
    if !result_json.is_object() {
        result_json = serde_json::json!({});
    }
    if let Some(obj) = result_json.as_object_mut() {
        obj.insert(
            "_watchdog".to_string(),
            serde_json::json!({
                "first_seen_stale_at": Utc::now().to_rfc3339(),
                "observed_updated_at": observed_updated_at.to_rfc3339(),
            }),
        );
    }
    result_json
}

fn stale_watchdog_confirmed(
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
) -> Result<u64> {
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
        .bind(idle_timeout_secs as i32)
        .fetch_all(pool)
        .await?;

    let mut reclaimed = 0u64;
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
            reclaimed += affected;
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
    }

    Ok(reclaimed)
}
