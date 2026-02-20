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
    Ingest,
}

impl JobTable {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Crawl => "axon_crawl_jobs",
            Self::Batch => "axon_batch_jobs",
            Self::Extract => "axon_extract_jobs",
            Self::Embed => "axon_embed_jobs",
            Self::Ingest => "axon_ingest_jobs",
        }
    }
}

#[cfg(test)]
pub(crate) fn test_config(pg_url: &str) -> Config {
    use crate::axon_cli::crates::core::config::{
        CommandKind, PerformanceProfile, RenderMode, ScrapeFormat,
    };
    use std::path::PathBuf;

    Config {
        command: CommandKind::Status,
        start_url: "https://example.com".to_string(),
        positional: Vec::new(),
        urls_csv: None,
        url_glob: Vec::new(),
        query: None,
        search_limit: 5,
        max_pages: 10,
        max_depth: 2,
        include_subdomains: true,
        exclude_path_prefix: Vec::new(),
        output_dir: PathBuf::from(".cache/test-worker-jobs"),
        output_path: None,
        render_mode: RenderMode::AutoSwitch,
        chrome_remote_url: None,
        chrome_proxy: None,
        chrome_user_agent: None,
        chrome_headless: true,
        chrome_anti_bot: true,
        chrome_intercept: true,
        chrome_stealth: true,
        chrome_bootstrap: false,
        chrome_bootstrap_timeout_ms: 45_000,
        chrome_bootstrap_retries: 1,
        webdriver_url: None,
        respect_robots: false,
        min_markdown_chars: 50,
        drop_thin_markdown: false,
        discover_sitemaps: true,
        cache: true,
        cache_skip_browser: false,
        format: ScrapeFormat::Markdown,
        collection: "test".to_string(),
        embed: false,
        batch_concurrency: 2,
        wait: false,
        yes: true,
        performance_profile: PerformanceProfile::Balanced,
        crawl_concurrency_limit: Some(2),
        sitemap_concurrency_limit: Some(2),
        backfill_concurrency_limit: Some(2),
        max_sitemaps: 10,
        delay_ms: 0,
        request_timeout_ms: Some(5_000),
        fetch_retries: 0,
        retry_backoff_ms: 0,
        shared_queue: true,
        pg_url: pg_url.to_string(),
        redis_url: "redis://127.0.0.1:1".to_string(),
        amqp_url: "amqp://guest:guest@127.0.0.1:1/%2f".to_string(),
        crawl_queue: "axon.test.crawl".to_string(),
        batch_queue: "axon.test.batch".to_string(),
        extract_queue: "axon.test.extract".to_string(),
        embed_queue: "axon.test.embed".to_string(),
        ingest_queue: "axon.test.ingest".to_string(),
        github_token: None,
        github_include_source: false,
        reddit_client_id: None,
        reddit_client_secret: None,
        tei_url: "http://127.0.0.1:1".to_string(),
        qdrant_url: "http://127.0.0.1:1".to_string(),
        openai_base_url: "http://127.0.0.1:1/v1".to_string(),
        openai_api_key: "test".to_string(),
        openai_model: "test-model".to_string(),
        ask_max_context_chars: 12_000,
        ask_candidate_limit: 64,
        ask_chunk_limit: 10,
        ask_full_docs: 4,
        ask_backfill_chunks: 3,
        ask_doc_fetch_concurrency: 4,
        ask_doc_chunk_limit: 192,
        ask_min_relevance_score: 0.45,
        ask_diagnostics: false,
        cron_every_seconds: None,
        cron_max_runs: None,
        watchdog_stale_timeout_secs: 300,
        watchdog_confirm_secs: 60,
        json_output: false,
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
    use crate::axon_cli::crates::core::logging::log_warn;
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

#[cfg(test)]
mod tests;
