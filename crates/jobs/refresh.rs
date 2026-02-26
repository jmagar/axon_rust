use crate::crates::core::config::Config;
use crate::crates::core::content::{to_markdown, url_to_filename};
use crate::crates::core::http::{http_client, validate_url};
use crate::crates::core::logging::{log_info, log_warn};
use crate::crates::crawl::manifest::ManifestEntry;
use crate::crates::jobs::common::{
    JobTable, enqueue_job, make_pool, mark_job_failed, purge_queue_safe, reclaim_stale_running_jobs,
};
use crate::crates::jobs::status::JobStatus;
use crate::crates::jobs::worker_lane::{ProcessFn, WorkerConfig, run_job_worker};
use crate::crates::vector::ops::embed_text_with_metadata;
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use reqwest::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::time::Duration;
use uuid::Uuid;

static SCHEMA_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

const TABLE: JobTable = JobTable::Refresh;
const REFRESH_HEARTBEAT_INTERVAL_SECS: u64 = 15;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RefreshJobConfig {
    urls: Vec<String>,
    embed: bool,
    output_dir: String,
}

#[derive(Debug, Clone)]
struct RefreshTargetState {
    etag: Option<String>,
    last_modified: Option<String>,
    content_hash: Option<String>,
}

#[derive(Debug)]
struct RefreshPageResult {
    status_code: u16,
    etag: Option<String>,
    last_modified: Option<String>,
    content_hash: Option<String>,
    markdown_chars: Option<usize>,
    markdown: Option<String>,
    changed: bool,
    not_modified: bool,
}

#[derive(Debug, Default, Serialize)]
struct RefreshRunSummary {
    checked: usize,
    changed: usize,
    unchanged: usize,
    not_modified: usize,
    failed: usize,
    embedded_chunks: usize,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct RefreshJob {
    pub id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_text: Option<String>,
    pub urls_json: serde_json::Value,
    pub result_json: Option<serde_json::Value>,
    pub config_json: serde_json::Value,
}

async fn touch_running_refresh_job(pool: &PgPool, id: Uuid) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(&format!(
        "UPDATE axon_refresh_jobs SET updated_at=NOW() WHERE id=$1 AND status='{running}'",
        running = JobStatus::Running.as_str(),
    ))
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected())
}

async fn ensure_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS axon_refresh_jobs (
            id UUID PRIMARY KEY,
            status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed', 'canceled')),
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
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_axon_refresh_jobs_pending ON axon_refresh_jobs(created_at ASC) WHERE status = 'pending'",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS axon_refresh_targets (
            url TEXT PRIMARY KEY,
            etag TEXT,
            last_modified TEXT,
            content_hash TEXT,
            markdown_chars INTEGER,
            last_status INTEGER,
            last_checked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            last_changed_at TIMESTAMPTZ,
            error_text TEXT
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn start_refresh_job(cfg: &Config, urls: &[String]) -> Result<Uuid, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }

    let id = Uuid::new_v4();
    let urls_json = serde_json::to_value(urls)?;
    let cfg_json = serde_json::to_value(RefreshJobConfig {
        urls: urls.to_vec(),
        embed: cfg.embed,
        output_dir: cfg.output_dir.to_string_lossy().to_string(),
    })?;

    sqlx::query(&format!(
        "INSERT INTO axon_refresh_jobs (id, status, urls_json, config_json) VALUES ($1, '{pending}', $2, $3)",
        pending = JobStatus::Pending.as_str(),
    ))
    .bind(id)
    .bind(urls_json)
    .bind(cfg_json)
    .execute(&pool)
    .await?;

    if let Err(err) = enqueue_job(cfg, &cfg.refresh_queue, id).await {
        log_warn(&format!(
            "refresh enqueue failed for {id}; polling fallback will pick up: {err}"
        ));
    }

    Ok(id)
}

pub async fn get_refresh_job(cfg: &Config, id: Uuid) -> Result<Option<RefreshJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    Ok(sqlx::query_as::<_, RefreshJob>(
        r#"SELECT id,status,created_at,updated_at,started_at,finished_at,error_text,urls_json,result_json,config_json FROM axon_refresh_jobs WHERE id=$1"#,
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?)
}

pub async fn list_refresh_jobs(
    cfg: &Config,
    limit: i64,
) -> Result<Vec<RefreshJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    Ok(sqlx::query_as::<_, RefreshJob>(
        r#"SELECT id,status,created_at,updated_at,started_at,finished_at,error_text,urls_json,result_json,config_json FROM axon_refresh_jobs ORDER BY created_at DESC LIMIT $1"#,
    )
    .bind(limit)
    .fetch_all(&pool)
    .await?)
}

pub async fn cancel_refresh_job(cfg: &Config, id: Uuid) -> Result<bool, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }

    let rows = sqlx::query(
        "UPDATE axon_refresh_jobs SET status=$2,updated_at=NOW(),finished_at=NOW() WHERE id=$1 AND status IN ($3,$4)",
    )
    .bind(id)
    .bind(JobStatus::Canceled.as_str())
    .bind(JobStatus::Pending.as_str())
    .bind(JobStatus::Running.as_str())
    .execute(&pool)
    .await?
    .rows_affected();

    Ok(rows > 0)
}

pub async fn cleanup_refresh_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    let mut total = 0u64;
    loop {
        let deleted = sqlx::query(
            "DELETE FROM axon_refresh_jobs WHERE id IN (SELECT id FROM axon_refresh_jobs WHERE status IN ($1,$2) LIMIT 1000)",
        )
        .bind(JobStatus::Failed.as_str())
        .bind(JobStatus::Canceled.as_str())
        .execute(&pool)
        .await?
        .rows_affected();
        total += deleted;
        if deleted == 0 {
            break;
        }
    }
    Ok(total)
}

pub async fn clear_refresh_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    let rows = sqlx::query("DELETE FROM axon_refresh_jobs")
        .execute(&pool)
        .await?
        .rows_affected();
    let _ = purge_queue_safe(cfg, &cfg.refresh_queue).await;
    Ok(rows)
}

pub async fn recover_stale_refresh_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    let stats = reclaim_stale_running_jobs(
        &pool,
        TABLE,
        "refresh",
        cfg.watchdog_stale_timeout_secs,
        cfg.watchdog_confirm_secs,
        "manual",
    )
    .await?;
    Ok(stats.reclaimed_jobs)
}

async fn load_target_states(
    pool: &PgPool,
    urls: &[String],
) -> Result<HashMap<String, RefreshTargetState>, Box<dyn Error>> {
    let mut states = HashMap::new();
    for url in urls {
        let row = sqlx::query_as::<_, (Option<String>, Option<String>, Option<String>)>(
            "SELECT etag,last_modified,content_hash FROM axon_refresh_targets WHERE url=$1",
        )
        .bind(url)
        .fetch_optional(pool)
        .await?;
        if let Some((etag, last_modified, content_hash)) = row {
            states.insert(
                url.clone(),
                RefreshTargetState {
                    etag,
                    last_modified,
                    content_hash,
                },
            );
        }
    }
    Ok(states)
}

async fn upsert_target_state(
    pool: &PgPool,
    url: &str,
    result: &RefreshPageResult,
    error_text: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    sqlx::query(
        r#"
        INSERT INTO axon_refresh_targets (
            url, etag, last_modified, content_hash, markdown_chars, last_status, last_checked_at, last_changed_at, error_text
        ) VALUES (
            $1, $2, $3, $4, $5, $6, NOW(), CASE WHEN $7 THEN NOW() ELSE NULL END, $8
        )
        ON CONFLICT (url)
        DO UPDATE SET
            etag = COALESCE(EXCLUDED.etag, axon_refresh_targets.etag),
            last_modified = COALESCE(EXCLUDED.last_modified, axon_refresh_targets.last_modified),
            content_hash = COALESCE(EXCLUDED.content_hash, axon_refresh_targets.content_hash),
            markdown_chars = COALESCE(EXCLUDED.markdown_chars, axon_refresh_targets.markdown_chars),
            last_status = EXCLUDED.last_status,
            last_checked_at = NOW(),
            last_changed_at = CASE
                WHEN $7 THEN NOW()
                ELSE axon_refresh_targets.last_changed_at
            END,
            error_text = EXCLUDED.error_text
        "#,
    )
    .bind(url)
    .bind(result.etag.as_deref())
    .bind(result.last_modified.as_deref())
    .bind(result.content_hash.as_deref())
    .bind(result.markdown_chars.map(|v| v as i32))
    .bind(result.status_code as i32)
    .bind(result.changed)
    .bind(error_text)
    .execute(pool)
    .await?;
    Ok(())
}

async fn refresh_one_url(
    client: &reqwest::Client,
    url: &str,
    previous: Option<&RefreshTargetState>,
) -> Result<RefreshPageResult, Box<dyn Error>> {
    validate_url(url)?;

    let mut request = client.get(url);
    if let Some(prev) = previous {
        if let Some(etag) = prev.etag.as_deref() {
            request = request.header(IF_NONE_MATCH, etag);
        }
        if let Some(last_modified) = prev.last_modified.as_deref() {
            request = request.header(IF_MODIFIED_SINCE, last_modified);
        }
    }

    let response = request.send().await?;
    let status = response.status();
    let status_code = status.as_u16();

    if status == StatusCode::NOT_MODIFIED {
        return Ok(RefreshPageResult {
            status_code,
            etag: None,
            last_modified: None,
            content_hash: previous.and_then(|s| s.content_hash.clone()),
            markdown_chars: None,
            markdown: None,
            changed: false,
            not_modified: true,
        });
    }

    let headers = response.headers().clone();
    let etag = headers
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string());
    let last_modified = headers
        .get(LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string());

    if !status.is_success() {
        return Ok(RefreshPageResult {
            status_code,
            etag,
            last_modified,
            content_hash: previous.and_then(|s| s.content_hash.clone()),
            markdown_chars: None,
            markdown: None,
            changed: false,
            not_modified: false,
        });
    }

    let body = response.text().await?;
    let markdown = to_markdown(&body);
    let trimmed = markdown.trim().to_string();
    let mut hasher = Sha256::new();
    hasher.update(trimmed.as_bytes());
    let content_hash = hex::encode(hasher.finalize());
    let markdown_chars = trimmed.len();
    let changed = previous
        .and_then(|s| s.content_hash.as_deref())
        .is_none_or(|hash| hash != content_hash);

    Ok(RefreshPageResult {
        status_code,
        etag,
        last_modified,
        content_hash: Some(content_hash),
        markdown_chars: Some(markdown_chars),
        markdown: Some(trimmed),
        changed,
        not_modified: false,
    })
}

async fn process_refresh_job(cfg: Config, pool: PgPool, id: Uuid) {
    let cfg_row = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT config_json FROM axon_refresh_jobs WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await;

    let job_cfg: RefreshJobConfig = match cfg_row {
        Ok(Some(v)) => match serde_json::from_value(v) {
            Ok(c) => c,
            Err(e) => {
                let _ =
                    mark_job_failed(&pool, TABLE, id, &format!("invalid config_json: {e}")).await;
                return;
            }
        },
        Ok(None) => {
            let _ = mark_job_failed(&pool, TABLE, id, "job not found in DB").await;
            return;
        }
        Err(e) => {
            let _ = mark_job_failed(&pool, TABLE, id, &format!("DB read error: {e}")).await;
            return;
        }
    };

    let run_dir = std::path::PathBuf::from(&job_cfg.output_dir)
        .join("refresh")
        .join(id.to_string());
    let markdown_dir = run_dir.join("markdown");

    if let Err(err) = tokio::fs::create_dir_all(&markdown_dir).await {
        let _ = mark_job_failed(
            &pool,
            TABLE,
            id,
            &format!("create refresh output dir failed: {err}"),
        )
        .await;
        return;
    }

    let manifest_path = run_dir.join("manifest.jsonl");
    let manifest_file = match tokio::fs::File::create(&manifest_path).await {
        Ok(file) => file,
        Err(err) => {
            let _ = mark_job_failed(
                &pool,
                TABLE,
                id,
                &format!("create refresh manifest failed: {err}"),
            )
            .await;
            return;
        }
    };
    let mut manifest = tokio::io::BufWriter::new(manifest_file);

    let (heartbeat_stop_tx, mut heartbeat_stop_rx) = tokio::sync::watch::channel(false);
    let heartbeat_pool = pool.clone();
    let heartbeat_task = tokio::spawn(async move {
        let mut ticker =
            tokio::time::interval(Duration::from_secs(REFRESH_HEARTBEAT_INTERVAL_SECS));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let _ = touch_running_refresh_job(&heartbeat_pool, id).await;
                }
                changed = heartbeat_stop_rx.changed() => {
                    if changed.is_err() || *heartbeat_stop_rx.borrow() {
                        break;
                    }
                }
            }
        }
    });

    let mut summary = RefreshRunSummary::default();
    let client = match http_client() {
        Ok(c) => c,
        Err(err) => {
            let _ = heartbeat_stop_tx.send(true);
            let _ = heartbeat_task.await;
            let _ =
                mark_job_failed(&pool, TABLE, id, &format!("http client unavailable: {err}")).await;
            return;
        }
    };

    let states = match load_target_states(&pool, &job_cfg.urls).await {
        Ok(s) => s,
        Err(err) => {
            let _ = heartbeat_stop_tx.send(true);
            let _ = heartbeat_task.await;
            let _ = mark_job_failed(
                &pool,
                TABLE,
                id,
                &format!("load refresh target state failed: {err}"),
            )
            .await;
            return;
        }
    };

    let mut changed_idx: u32 = 0;

    for url in &job_cfg.urls {
        summary.checked += 1;
        let previous = states.get(url);

        match refresh_one_url(client, url, previous).await {
            Ok(result) => {
                if result.status_code >= 400 {
                    summary.failed += 1;
                    let error_text = format!("HTTP {}", result.status_code);
                    let _ = upsert_target_state(&pool, url, &result, Some(&error_text)).await;
                    continue;
                }

                if result.not_modified {
                    summary.not_modified += 1;
                    summary.unchanged += 1;
                    let _ = upsert_target_state(&pool, url, &result, None).await;
                    continue;
                }

                if result.changed {
                    changed_idx += 1;
                    summary.changed += 1;

                    if let Some(markdown) = result.markdown.as_deref() {
                        let filename = url_to_filename(url, changed_idx);
                        let file_path = markdown_dir.join(&filename);
                        if let Err(err) = tokio::fs::write(&file_path, markdown.as_bytes()).await {
                            summary.failed += 1;
                            let _ = upsert_target_state(
                                &pool,
                                url,
                                &result,
                                Some(&format!("write markdown failed: {err}")),
                            )
                            .await;
                            continue;
                        }

                        let entry = ManifestEntry {
                            url: url.clone(),
                            relative_path: format!("markdown/{filename}"),
                            markdown_chars: result.markdown_chars.unwrap_or(0),
                            content_hash: result.content_hash.clone(),
                            changed: true,
                        };
                        if let Ok(mut line) = serde_json::to_string(&entry) {
                            line.push('\n');
                            let _ = manifest.write_all(line.as_bytes()).await;
                        }

                        if job_cfg.embed {
                            match embed_text_with_metadata(&cfg, markdown, url, "refresh", None)
                                .await
                            {
                                Ok(chunks) => {
                                    summary.embedded_chunks += chunks;
                                }
                                Err(err) => {
                                    log_warn(&format!(
                                        "refresh embed failed for url={} job_id={}: {}",
                                        url, id, err
                                    ));
                                }
                            }
                        }
                    }
                } else {
                    summary.unchanged += 1;
                }

                let _ = upsert_target_state(&pool, url, &result, None).await;
            }
            Err(err) => {
                summary.failed += 1;
                let fallback = RefreshPageResult {
                    status_code: 0,
                    etag: previous.and_then(|s| s.etag.clone()),
                    last_modified: previous.and_then(|s| s.last_modified.clone()),
                    content_hash: previous.and_then(|s| s.content_hash.clone()),
                    markdown_chars: None,
                    markdown: None,
                    changed: false,
                    not_modified: false,
                };
                let _ = upsert_target_state(&pool, url, &fallback, Some(&err.to_string())).await;
            }
        }

        let _ = sqlx::query(&format!(
            "UPDATE axon_refresh_jobs SET updated_at=NOW(), result_json=$2 WHERE id=$1 AND status='{running}'",
            running = JobStatus::Running.as_str(),
        ))
        .bind(id)
        .bind(serde_json::json!({
            "phase": "refreshing",
            "checked": summary.checked,
            "changed": summary.changed,
            "unchanged": summary.unchanged,
            "not_modified": summary.not_modified,
            "failed": summary.failed,
            "embedded_chunks": summary.embedded_chunks,
            "total": job_cfg.urls.len(),
            "output_dir": run_dir.to_string_lossy(),
        }))
        .execute(&pool)
        .await;
    }

    let _ = manifest.flush().await;

    let _ = heartbeat_stop_tx.send(true);
    if let Err(err) = heartbeat_task.await {
        log_warn(&format!(
            "command=refresh_worker heartbeat_task_panicked job_id={id} err={err:?}"
        ));
    }

    let final_result = serde_json::json!({
        "phase": "completed",
        "checked": summary.checked,
        "changed": summary.changed,
        "unchanged": summary.unchanged,
        "not_modified": summary.not_modified,
        "failed": summary.failed,
        "embedded_chunks": summary.embedded_chunks,
        "total": job_cfg.urls.len(),
        "output_dir": run_dir.to_string_lossy(),
        "manifest_path": manifest_path.to_string_lossy(),
    });

    match sqlx::query(&format!(
        "UPDATE axon_refresh_jobs SET status='{completed}',updated_at=NOW(),finished_at=NOW(),error_text=NULL,result_json=$2 WHERE id=$1 AND status='{running}'",
        completed = JobStatus::Completed.as_str(),
        running = JobStatus::Running.as_str(),
    ))
    .bind(id)
    .bind(final_result)
    .execute(&pool)
    .await
    {
        Ok(done) => {
            if done.rows_affected() == 0 {
                log_warn(&format!(
                    "command=refresh_worker completion_update_skipped job_id={id} reason=not_running_state"
                ));
            }
        }
        Err(err) => {
            let _ = mark_job_failed(&pool, TABLE, id, &format!("mark completed failed: {err}")).await;
        }
    }
}

pub async fn run_refresh_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }

    let wc = WorkerConfig {
        table: TABLE,
        queue_name: cfg.refresh_queue.clone(),
        job_kind: "refresh",
        consumer_tag_prefix: "refresh-worker",
        lane_count: 2,
    };

    let process_fn: ProcessFn =
        Arc::new(|cfg, pool, id| Box::pin(process_refresh_job(cfg, pool, id)));

    run_job_worker(cfg, pool, &wc, process_fn).await
}

pub async fn run_refresh_once(
    cfg: &Config,
    urls: &[String],
) -> Result<serde_json::Value, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }

    let id = Uuid::new_v4();
    let urls_json = serde_json::to_value(urls)?;
    let cfg_json = serde_json::to_value(RefreshJobConfig {
        urls: urls.to_vec(),
        embed: cfg.embed,
        output_dir: cfg.output_dir.to_string_lossy().to_string(),
    })?;

    sqlx::query(
        "INSERT INTO axon_refresh_jobs (id, status, urls_json, config_json, started_at) VALUES ($1, $2, $3, $4, NOW())",
    )
    .bind(id)
    .bind(JobStatus::Running.as_str())
    .bind(urls_json)
    .bind(cfg_json)
    .execute(&pool)
    .await?;

    process_refresh_job(cfg.clone(), pool.clone(), id).await;

    let result_json = sqlx::query_scalar::<_, Option<serde_json::Value>>(
        "SELECT result_json FROM axon_refresh_jobs WHERE id=$1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await?
    .unwrap_or_else(|| serde_json::json!({}));

    Ok(result_json)
}

pub async fn recover_stale_refresh_jobs_startup(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    let stats = reclaim_stale_running_jobs(
        &pool,
        TABLE,
        "refresh",
        cfg.watchdog_stale_timeout_secs,
        cfg.watchdog_confirm_secs,
        "startup",
    )
    .await?;
    log_info(&format!(
        "refresh watchdog startup reclaimed={} candidates={}",
        stats.reclaimed_jobs, stats.stale_candidates
    ));
    Ok(stats.reclaimed_jobs)
}
