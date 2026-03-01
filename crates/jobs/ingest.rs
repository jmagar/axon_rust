//! Ingest job schema uses advisory-lock DDL via `common::schema::begin_schema_migration_tx`.
//! See `common/schema.rs` for the canonical pattern.

use crate::crates::core::config::Config;
use crate::crates::core::logging::{log_info, log_warn};
use crate::crates::jobs::common::{
    JobTable, begin_schema_migration_tx, enqueue_job, make_pool, mark_job_failed, purge_queue_safe,
    reclaim_stale_running_jobs, spawn_heartbeat_task,
};
use crate::crates::jobs::status::JobStatus;
use crate::crates::jobs::worker_lane::{ProcessFn, WorkerConfig, run_job_worker};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::error::Error;
use std::sync::Arc;
use uuid::Uuid;

const TABLE: JobTable = JobTable::Ingest;
const INGEST_HEARTBEAT_INTERVAL_SECS: u64 = 30;

/// Discriminates which ingest source a job targets.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "source_type", rename_all = "lowercase")]
pub enum IngestSource {
    Github {
        repo: String,
        include_source: bool,
    },
    Reddit {
        target: String,
    },
    Youtube {
        target: String,
    },
    Sessions {
        sessions_claude: bool,
        sessions_codex: bool,
        sessions_gemini: bool,
        sessions_project: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestJobConfig {
    pub source: IngestSource,
    pub collection: String,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct IngestJob {
    pub id: Uuid,
    /// Raw status string from the database. Use [`IngestJob::status()`] for
    /// type-safe access when `JobStatus` gains `sqlx::Type` derive.
    pub status: String,
    pub source_type: String,
    pub target: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_text: Option<String>,
    pub result_json: Option<serde_json::Value>,
    pub config_json: serde_json::Value,
}

impl IngestJob {
    /// Parse the raw `status` string into a typed [`JobStatus`].
    ///
    /// Returns `None` if the string doesn't match any known variant (shouldn't
    /// happen with the CHECK constraint, but defensive is correct).
    pub fn status(&self) -> Option<JobStatus> {
        match self.status.as_str() {
            "pending" => Some(JobStatus::Pending),
            "running" => Some(JobStatus::Running),
            "completed" => Some(JobStatus::Completed),
            "failed" => Some(JobStatus::Failed),
            "canceled" => Some(JobStatus::Canceled),
            _ => None,
        }
    }
}

/// Advisory lock key for ingest job schema migrations (unique per table).
const INGEST_SCHEMA_LOCK_KEY: i64 = 0x696e_6765_7374_0000_i64;

async fn ensure_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut tx = begin_schema_migration_tx(pool, INGEST_SCHEMA_LOCK_KEY).await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS axon_ingest_jobs (
            id          UUID PRIMARY KEY,
            status      TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed', 'canceled')),
            source_type TEXT NOT NULL,
            target      TEXT NOT NULL,
            created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            started_at  TIMESTAMPTZ,
            finished_at TIMESTAMPTZ,
            error_text  TEXT,
            result_json JSONB,
            config_json JSONB NOT NULL
        )
        "#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_axon_ingest_jobs_pending \
         ON axon_ingest_jobs(created_at ASC) WHERE status = 'pending'",
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"DO $$ BEGIN
            ALTER TABLE axon_ingest_jobs ADD CONSTRAINT axon_ingest_jobs_status_check
                CHECK (status IN ('pending', 'running', 'completed', 'failed', 'canceled'));
        EXCEPTION WHEN duplicate_object THEN NULL;
        END $$"#,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

fn source_type_label(source: &IngestSource) -> &'static str {
    match source {
        IngestSource::Github { .. } => "github",
        IngestSource::Reddit { .. } => "reddit",
        IngestSource::Youtube { .. } => "youtube",
        IngestSource::Sessions { .. } => "sessions",
    }
}

fn target_label(source: &IngestSource) -> String {
    match source {
        IngestSource::Github { repo, .. } => repo.clone(),
        IngestSource::Reddit { target } => target.clone(),
        IngestSource::Youtube { target } => target.clone(),
        IngestSource::Sessions {
            sessions_claude,
            sessions_codex,
            sessions_gemini,
            sessions_project,
        } => {
            let all = !sessions_claude && !sessions_codex && !sessions_gemini;
            let label = if all {
                "all".to_string()
            } else {
                let mut parts = vec![];
                if *sessions_claude {
                    parts.push("claude");
                }
                if *sessions_codex {
                    parts.push("codex");
                }
                if *sessions_gemini {
                    parts.push("gemini");
                }
                parts.join(",")
            };
            match sessions_project {
                Some(proj) => format!("{label}:{proj}"),
                None => label,
            }
        }
    }
}

pub async fn start_ingest_job(cfg: &Config, source: IngestSource) -> Result<Uuid, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;

    let job_config = IngestJobConfig {
        source: source.clone(),
        collection: cfg.collection.clone(),
    };
    let cfg_json = serde_json::to_value(&job_config)?;
    let source_type = source_type_label(&source);
    let target = target_label(&source);

    let id = Uuid::new_v4();
    sqlx::query(&format!(
        "INSERT INTO axon_ingest_jobs (id, status, source_type, target, config_json) \
         VALUES ($1, '{pending}', $2, $3, $4)",
        pending = JobStatus::Pending.as_str(),
    ))
    .bind(id)
    .bind(source_type)
    .bind(&target)
    .bind(cfg_json)
    .execute(&pool)
    .await?;

    if let Err(err) = enqueue_job(cfg, &cfg.ingest_queue, id).await {
        log_warn(&format!(
            "ingest enqueue failed for {id}; polling fallback will pick up: {err}"
        ));
    }

    log_info(&format!(
        "ingest job queued: id={id} source={source_type} target={target}"
    ));
    Ok(id)
}

pub async fn get_ingest_job(cfg: &Config, id: Uuid) -> Result<Option<IngestJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    Ok(sqlx::query_as::<_, IngestJob>(
        "SELECT id,status,source_type,target,created_at,updated_at,started_at,finished_at,\
         error_text,result_json,config_json FROM axon_ingest_jobs WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?)
}

pub async fn list_ingest_jobs(cfg: &Config, limit: i64) -> Result<Vec<IngestJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    Ok(sqlx::query_as::<_, IngestJob>(
        "SELECT id,status,source_type,target,created_at,updated_at,started_at,finished_at,\
         error_text,result_json,config_json FROM axon_ingest_jobs ORDER BY created_at DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(&pool)
    .await?)
}

pub async fn cancel_ingest_job(cfg: &Config, id: Uuid) -> Result<bool, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query(&format!(
        "UPDATE axon_ingest_jobs SET status='{canceled}',updated_at=NOW(),finished_at=NOW() \
         WHERE id=$1 AND status IN ('{pending}','{running}')",
        canceled = JobStatus::Canceled.as_str(),
        pending = JobStatus::Pending.as_str(),
        running = JobStatus::Running.as_str(),
    ))
    .bind(id)
    .execute(&pool)
    .await?
    .rows_affected();
    Ok(rows > 0)
}

pub async fn cleanup_ingest_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    Ok(sqlx::query(&format!(
        "DELETE FROM axon_ingest_jobs WHERE status IN ('{failed}','{canceled}') \
         OR (status = '{completed}' AND finished_at < NOW() - INTERVAL '30 days')",
        failed = JobStatus::Failed.as_str(),
        canceled = JobStatus::Canceled.as_str(),
        completed = JobStatus::Completed.as_str(),
    ))
    .execute(&pool)
    .await?
    .rows_affected())
}

pub async fn clear_ingest_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query("DELETE FROM axon_ingest_jobs")
        .execute(&pool)
        .await?
        .rows_affected();
    let _ = purge_queue_safe(cfg, &cfg.ingest_queue).await;
    Ok(rows)
}

// SEC-M-6: `cfg` is captured by value but never serialized into error_text.
// All error paths pass only `e.to_string()` or static messages to `mark_job_failed`,
// so `openai_api_key` and other secrets in `cfg` cannot leak into the database.
async fn process_ingest_job(cfg: Config, pool: PgPool, id: Uuid) {
    use crate::crates::ingest;

    let cfg_row = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT config_json FROM axon_ingest_jobs WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await;

    let job_cfg: IngestJobConfig = match cfg_row {
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

    let (heartbeat_stop_tx, heartbeat_task) =
        spawn_heartbeat_task(pool.clone(), TABLE, id, INGEST_HEARTBEAT_INTERVAL_SECS);

    let result = match &job_cfg.source {
        IngestSource::Github {
            repo,
            include_source,
        } => ingest::github::ingest_github(&cfg, repo, *include_source).await,
        IngestSource::Reddit { target } => ingest::reddit::ingest_reddit(&cfg, target).await,
        IngestSource::Youtube { target } => ingest::youtube::ingest_youtube(&cfg, target).await,
        IngestSource::Sessions {
            sessions_claude,
            sessions_codex,
            sessions_gemini,
            sessions_project,
        } => {
            let mut sessions_cfg = cfg.clone();
            sessions_cfg.sessions_claude = *sessions_claude;
            sessions_cfg.sessions_codex = *sessions_codex;
            sessions_cfg.sessions_gemini = *sessions_gemini;
            sessions_cfg.sessions_project = sessions_project.clone();
            ingest::sessions::ingest_sessions(&sessions_cfg).await
        }
    };
    let _ = heartbeat_stop_tx.send(true);
    if let Err(err) = heartbeat_task.await {
        log_warn(&format!(
            "command=ingest_worker heartbeat_task_panicked job_id={id} err={err:?}"
        ));
    }

    match result {
        Ok(chunks) => {
            match sqlx::query(&format!(
                "UPDATE axon_ingest_jobs SET status='{completed}',updated_at=NOW(),\
                 finished_at=NOW(),result_json=$2 WHERE id=$1 AND status='{running}'",
                completed = JobStatus::Completed.as_str(),
                running = JobStatus::Running.as_str(),
            ))
            .bind(id)
            .bind(serde_json::json!({"chunks_embedded": chunks}))
            .execute(&pool)
            .await
            {
                Ok(done) => {
                    if done.rows_affected() == 0 {
                        log_warn(&format!(
                            "command=ingest_worker completion_update_skipped job_id={id} reason=not_running_state"
                        ));
                    }
                }
                Err(e) => {
                    log_warn(&format!(
                        "command=ingest_worker mark_completed_failed job_id={id} err={e}"
                    ));
                }
            }
        }
        Err(e) => {
            let _ = mark_job_failed(&pool, TABLE, id, &e.to_string()).await;
        }
    }
}

pub async fn ingest_doctor(cfg: &Config) -> Result<serde_json::Value, Box<dyn Error>> {
    use crate::crates::core::health::redis_healthy;
    use crate::crates::jobs::common::open_amqp_channel;

    let (pg_ok, amqp_result, redis_ok) = tokio::join!(
        async { make_pool(cfg).await.is_ok() },
        open_amqp_channel(cfg, &cfg.ingest_queue),
        redis_healthy(&cfg.redis_url),
    );
    let amqp_ok = match amqp_result {
        Ok(ch) => {
            let _ = ch.close(0, "probe").await;
            true
        }
        Err(_) => false,
    };
    Ok(serde_json::json!({
        "postgres_ok": pg_ok,
        "amqp_ok": amqp_ok,
        "redis_ok": redis_ok,
        "queue": cfg.ingest_queue,
        "all_ok": pg_ok && amqp_ok && redis_ok
    }))
}

pub async fn run_ingest_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;

    let wc = WorkerConfig {
        table: TABLE,
        queue_name: cfg.ingest_queue.clone(),
        job_kind: "ingest",
        consumer_tag_prefix: "ingest-worker",
        lane_count: std::env::var("AXON_INGEST_LANES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2),
    };

    let process_fn: ProcessFn =
        Arc::new(|cfg, pool, id| Box::pin(process_ingest_job(cfg, pool, id)));

    run_job_worker(cfg, pool, &wc, process_fn).await
}

pub async fn recover_stale_ingest_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let stats = reclaim_stale_running_jobs(
        &pool,
        TABLE,
        "ingest",
        cfg.watchdog_stale_timeout_secs,
        cfg.watchdog_confirm_secs,
        "manual",
    )
    .await?;
    Ok(stats.reclaimed_jobs)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- source_type_label tests --

    #[test]
    fn source_type_label_github() {
        let source = IngestSource::Github {
            repo: "owner/repo".into(),
            include_source: true,
        };
        assert_eq!(source_type_label(&source), "github");
    }

    #[test]
    fn source_type_label_reddit() {
        let source = IngestSource::Reddit {
            target: "r/rust".into(),
        };
        assert_eq!(source_type_label(&source), "reddit");
    }

    #[test]
    fn source_type_label_youtube() {
        let source = IngestSource::Youtube {
            target: "https://youtube.com/watch?v=abc".into(),
        };
        assert_eq!(source_type_label(&source), "youtube");
    }

    #[test]
    fn source_type_label_sessions() {
        let source = IngestSource::Sessions {
            sessions_claude: true,
            sessions_codex: false,
            sessions_gemini: false,
            sessions_project: None,
        };
        assert_eq!(source_type_label(&source), "sessions");
    }

    // -- target_label tests --

    #[test]
    fn target_label_github_returns_repo() {
        let source = IngestSource::Github {
            repo: "anthropics/claude-code".into(),
            include_source: false,
        };
        assert_eq!(target_label(&source), "anthropics/claude-code");
    }

    #[test]
    fn target_label_sessions_all_when_no_flags() {
        let source = IngestSource::Sessions {
            sessions_claude: false,
            sessions_codex: false,
            sessions_gemini: false,
            sessions_project: None,
        };
        assert_eq!(target_label(&source), "all");
    }

    #[test]
    fn target_label_sessions_multiple_flags() {
        let source = IngestSource::Sessions {
            sessions_claude: true,
            sessions_codex: false,
            sessions_gemini: true,
            sessions_project: None,
        };
        assert_eq!(target_label(&source), "claude,gemini");
    }

    #[test]
    fn target_label_sessions_with_project() {
        let source = IngestSource::Sessions {
            sessions_claude: true,
            sessions_codex: true,
            sessions_gemini: false,
            sessions_project: Some("axon-rust".into()),
        };
        assert_eq!(target_label(&source), "claude,codex:axon-rust");
    }

    #[test]
    fn target_label_sessions_all_with_project() {
        let source = IngestSource::Sessions {
            sessions_claude: false,
            sessions_codex: false,
            sessions_gemini: false,
            sessions_project: Some("my-project".into()),
        };
        assert_eq!(target_label(&source), "all:my-project");
    }

    // -- IngestJob::status() tests --

    #[test]
    fn ingest_job_status_parses_known_variants() {
        for (raw, expected) in [
            ("pending", JobStatus::Pending),
            ("running", JobStatus::Running),
            ("completed", JobStatus::Completed),
            ("failed", JobStatus::Failed),
            ("canceled", JobStatus::Canceled),
        ] {
            let job = IngestJob {
                id: Uuid::nil(),
                status: raw.to_string(),
                source_type: "github".into(),
                target: "test".into(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                started_at: None,
                finished_at: None,
                error_text: None,
                result_json: None,
                config_json: serde_json::json!({}),
            };
            assert_eq!(job.status(), Some(expected));
        }
    }

    #[test]
    fn ingest_job_status_returns_none_for_unknown() {
        let job = IngestJob {
            id: Uuid::nil(),
            status: "bogus".to_string(),
            source_type: "github".into(),
            target: "test".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            started_at: None,
            finished_at: None,
            error_text: None,
            result_json: None,
            config_json: serde_json::json!({}),
        };
        assert_eq!(job.status(), None);
    }
}
