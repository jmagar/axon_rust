use crate::crates::core::config::Config;
use crate::crates::core::logging::log_warn;
use crate::crates::jobs::common::make_pool;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use sqlx::{PgPool, Row};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

mod claude;
mod codex;
mod gemini;

pub(crate) type IngestResult<T> = Result<T, anyhow::Error>;

pub(crate) fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

pub(crate) struct SessionStateTracker {
    pool: Option<PgPool>,
}

impl SessionStateTracker {
    pub(crate) async fn new(cfg: &Config) -> Self {
        match make_pool(cfg).await {
            Ok(pool) => {
                if let Err(e) = sqlx::query(
                    r#"
                    CREATE TABLE IF NOT EXISTS axon_session_ingest_state (
                        file_path TEXT PRIMARY KEY,
                        last_modified TIMESTAMPTZ NOT NULL,
                        file_size BIGINT NOT NULL,
                        indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                    )
                    "#,
                )
                .execute(&pool)
                .await
                {
                    log_warn(&format!("failed to ensure session state table: {e}"));
                    return Self { pool: None };
                }
                Self { pool: Some(pool) }
            }
            Err(e) => {
                log_warn(&format!(
                    "database connection failed, state tracking disabled: {e}"
                ));
                Self { pool: None }
            }
        }
    }

    pub(crate) async fn should_skip(&self, path: &Path, mtime: SystemTime, size: u64) -> bool {
        let Some(pool) = &self.pool else {
            return false;
        };
        let path_str = path.to_string_lossy().to_string();

        let row = sqlx::query(
            "SELECT last_modified, file_size FROM axon_session_ingest_state WHERE file_path = $1",
        )
        .bind(path_str)
        .fetch_optional(pool)
        .await;

        match row {
            Ok(Some(r)) => {
                let db_mtime: chrono::DateTime<chrono::Utc> = r.get(0);
                let db_size: i64 = r.get(1);
                let current_mtime: chrono::DateTime<chrono::Utc> = mtime.into();
                (db_mtime - current_mtime).num_seconds().abs() < 1 && db_size == (size as i64)
            }
            _ => false,
        }
    }

    pub(crate) async fn mark_indexed(&self, path: &Path, mtime: SystemTime, size: u64) {
        let Some(pool) = &self.pool else {
            return;
        };
        let path_str = path.to_string_lossy().to_string();
        let mtime_chrono: chrono::DateTime<chrono::Utc> = mtime.into();

        let _ = sqlx::query(
            r#"
            INSERT INTO axon_session_ingest_state (file_path, last_modified, file_size, indexed_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (file_path) DO UPDATE
            SET last_modified = EXCLUDED.last_modified,
                file_size = EXCLUDED.file_size,
                indexed_at = NOW()
            "#,
        )
        .bind(path_str)
        .bind(mtime_chrono)
        .bind(size as i64)
        .execute(pool)
        .await;
    }
}

pub async fn ingest_sessions(cfg: &Config) -> Result<usize, Box<dyn Error>> {
    let state = SessionStateTracker::new(cfg).await;
    let multi = MultiProgress::new();
    let main_pb = multi.add(ProgressBar::new_spinner());
    main_pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .unwrap(),
    );
    main_pb.set_message("Discovering session files...");
    main_pb.enable_steady_tick(Duration::from_millis(100));

    let mut total_chunks = 0;
    let all_platforms = !cfg.sessions_claude && !cfg.sessions_codex && !cfg.sessions_gemini;

    if cfg.sessions_claude || all_platforms {
        total_chunks += claude::ingest_claude_sessions(cfg, &state, &multi)
            .await
            .unwrap_or(0);
    }
    if cfg.sessions_codex || all_platforms {
        total_chunks += codex::ingest_codex_sessions(cfg, &state, &multi)
            .await
            .unwrap_or(0);
    }
    if cfg.sessions_gemini || all_platforms {
        total_chunks += gemini::ingest_gemini_sessions(cfg, &state, &multi)
            .await
            .unwrap_or(0);
    }

    main_pb.finish_with_message(format!(
        "Ingestion complete: {} chunks embedded",
        total_chunks
    ));
    Ok(total_chunks)
}

pub(crate) fn resolve_collection(cfg: &Config, derived_name: &str) -> String {
    if cfg.collection != "cortex" {
        return cfg.collection.clone();
    }
    if derived_name.is_empty() {
        return "global-sessions".to_string();
    }
    format!("{}-sessions", derived_name)
}

pub(crate) fn matches_project_filter(cfg: &Config, name: &str) -> bool {
    if let Some(filter) = &cfg.sessions_project {
        name.to_lowercase().contains(&filter.to_lowercase())
    } else {
        true
    }
}
