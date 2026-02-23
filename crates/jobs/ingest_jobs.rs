use crate::crates::core::config::Config;
use crate::crates::core::logging::{log_info, log_warn};
use crate::crates::jobs::common::{
    enqueue_job, make_pool, mark_job_failed, purge_queue_safe, reclaim_stale_running_jobs, JobTable,
};
use crate::crates::jobs::worker_lane::{run_job_worker, ProcessFn, WorkerConfig};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::error::Error;
use std::sync::Arc;
use uuid::Uuid;

static SCHEMA_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

const TABLE: JobTable = JobTable::Ingest;

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

#[derive(Debug, FromRow, Serialize)]
pub struct IngestJob {
    pub id: Uuid,
    pub status: String,
    pub source_type: String,
    pub target: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_text: Option<String>,
    pub result_json: Option<serde_json::Value>,
}

async fn ensure_schema(pool: &PgPool) -> Result<(), Box<dyn Error>> {
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
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_axon_ingest_jobs_pending \
         ON axon_ingest_jobs(created_at ASC) WHERE status = 'pending'",
    )
    .execute(pool)
    .await?;

    // Add CHECK constraint to existing tables (idempotent via IF NOT EXISTS pattern).
    sqlx::query(
        r#"DO $$ BEGIN
            ALTER TABLE axon_ingest_jobs ADD CONSTRAINT axon_ingest_jobs_status_check
                CHECK (status IN ('pending', 'running', 'completed', 'failed', 'canceled'));
        EXCEPTION WHEN duplicate_object THEN NULL;
        END $$"#,
    )
    .execute(pool)
    .await?;

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
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }

    let job_config = IngestJobConfig {
        source: source.clone(),
        collection: cfg.collection.clone(),
    };
    let cfg_json = serde_json::to_value(&job_config)?;
    let source_type = source_type_label(&source);
    let target = target_label(&source);

    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO axon_ingest_jobs (id, status, source_type, target, config_json) \
         VALUES ($1, 'pending', $2, $3, $4)",
    )
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
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    Ok(sqlx::query_as::<_, IngestJob>(
        "SELECT id,status,source_type,target,created_at,updated_at,started_at,finished_at,\
         error_text,result_json FROM axon_ingest_jobs WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?)
}

pub async fn list_ingest_jobs(cfg: &Config, limit: i64) -> Result<Vec<IngestJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    Ok(sqlx::query_as::<_, IngestJob>(
        "SELECT id,status,source_type,target,created_at,updated_at,started_at,finished_at,\
         error_text,result_json FROM axon_ingest_jobs ORDER BY created_at DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(&pool)
    .await?)
}

pub async fn cancel_ingest_job(cfg: &Config, id: Uuid) -> Result<bool, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    let rows = sqlx::query(
        "UPDATE axon_ingest_jobs SET status='canceled',updated_at=NOW(),finished_at=NOW() \
         WHERE id=$1 AND status IN ('pending','running')",
    )
    .bind(id)
    .execute(&pool)
    .await?
    .rows_affected();
    Ok(rows > 0)
}

pub async fn cleanup_ingest_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    Ok(sqlx::query(
        "DELETE FROM axon_ingest_jobs WHERE status IN ('failed','canceled') \
         OR (status = 'completed' AND finished_at < NOW() - INTERVAL '30 days')",
    )
    .execute(&pool)
    .await?
    .rows_affected())
}

pub async fn clear_ingest_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    let rows = sqlx::query("DELETE FROM axon_ingest_jobs")
        .execute(&pool)
        .await?
        .rows_affected();
    let _ = purge_queue_safe(cfg, &cfg.ingest_queue).await;
    Ok(rows)
}

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
                mark_job_failed(&pool, TABLE, id, &format!("invalid config_json: {e}")).await;
                return;
            }
        },
        Ok(None) => {
            mark_job_failed(&pool, TABLE, id, "job not found in DB").await;
            return;
        }
        Err(e) => {
            mark_job_failed(&pool, TABLE, id, &format!("DB read error: {e}")).await;
            return;
        }
    };

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

    match result {
        Ok(chunks) => {
            if let Err(e) = sqlx::query(
                "UPDATE axon_ingest_jobs SET status='completed',updated_at=NOW(),\
                 finished_at=NOW(),result_json=$2 WHERE id=$1 AND status='running'",
            )
            .bind(id)
            .bind(serde_json::json!({"chunks_embedded": chunks}))
            .execute(&pool)
            .await
            {
                log_warn(&format!(
                    "command=ingest_worker mark_completed_failed job_id={id} err={e}"
                ));
            }
        }
        Err(e) => {
            mark_job_failed(&pool, TABLE, id, &e.to_string()).await;
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
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }

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
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
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
