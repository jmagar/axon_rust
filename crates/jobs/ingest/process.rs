use crate::crates::core::config::Config;
use crate::crates::core::logging::log_warn;
use crate::crates::jobs::common::{JobTable, mark_job_failed, spawn_heartbeat_task};
use sqlx::PgPool;
use uuid::Uuid;

use super::ops::mark_completed;
use super::types::{IngestJobConfig, IngestSource};

const TABLE: JobTable = JobTable::Ingest;
const INGEST_HEARTBEAT_INTERVAL_SECS: u64 = 30;

// SEC-M-6: `cfg` is captured by value but never serialized into error_text.
// All error paths pass only `e.to_string()` or static messages to `mark_job_failed`,
// so `openai_api_key` and other secrets in `cfg` cannot leak into the database.
pub(crate) async fn process_ingest_job(cfg: Config, pool: PgPool, id: Uuid) {
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
            mark_completed(&pool, id, chunks).await;
        }
        Err(e) => {
            let _ = mark_job_failed(&pool, TABLE, id, &e.to_string()).await;
        }
    }
}
