use crate::crates::core::config::{Config, RenderMode};
use crate::crates::jobs::batch_jobs::InjectionCandidate;
use crate::crates::jobs::common::JobTable;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::collections::HashSet;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokio;
use uuid::Uuid;

static SCHEMA_INIT: OnceLock<()> = OnceLock::new();

const TABLE: JobTable = JobTable::Crawl;
const MID_CRAWL_INJECTION_TRIGGER_PAGES: u32 = 25;
const MID_CRAWL_INJECTION_MIN_CANDIDATES: usize = 3;
const WORKER_CONCURRENCY: usize = 2;
const STALE_SWEEP_INTERVAL_SECS: u64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CrawlJobConfig {
    max_pages: u32,
    max_depth: usize,
    include_subdomains: bool,
    exclude_path_prefix: Vec<String>,
    respect_robots: bool,
    min_markdown_chars: usize,
    drop_thin_markdown: bool,
    discover_sitemaps: bool,
    embed: bool,
    render_mode: RenderMode,
    collection: String,
    output_dir: String,
    crawl_concurrency_limit: Option<usize>,
    backfill_concurrency_limit: Option<usize>,
    delay_ms: u64,
    request_timeout_ms: Option<u64>,
    fetch_retries: usize,
    retry_backoff_ms: u64,
    shared_queue: bool,
    extraction_prompt: Option<String>,
    #[serde(default = "default_cache_enabled")]
    cache: bool,
    #[serde(default)]
    cache_skip_browser: bool,
}

fn default_cache_enabled() -> bool {
    true
}

#[derive(Debug, FromRow, Serialize)]
pub struct CrawlJob {
    pub id: Uuid,
    pub url: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_text: Option<String>,
    pub result_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Default)]
struct CrawlWatchdogSweepStats {
    stale_candidates: u64,
    marked_candidates: u64,
    reclaimed_jobs: u64,
}

fn to_job_config(cfg: &Config) -> CrawlJobConfig {
    CrawlJobConfig {
        max_pages: cfg.max_pages,
        max_depth: cfg.max_depth,
        include_subdomains: cfg.include_subdomains,
        exclude_path_prefix: cfg.exclude_path_prefix.clone(),
        respect_robots: cfg.respect_robots,
        min_markdown_chars: cfg.min_markdown_chars,
        drop_thin_markdown: cfg.drop_thin_markdown,
        discover_sitemaps: cfg.discover_sitemaps,
        embed: cfg.embed,
        render_mode: cfg.render_mode,
        collection: cfg.collection.clone(),
        output_dir: cfg.output_dir.to_string_lossy().to_string(),
        crawl_concurrency_limit: cfg.crawl_concurrency_limit,
        backfill_concurrency_limit: cfg.backfill_concurrency_limit,
        delay_ms: cfg.delay_ms,
        request_timeout_ms: cfg.request_timeout_ms,
        fetch_retries: cfg.fetch_retries,
        retry_backoff_ms: cfg.retry_backoff_ms,
        shared_queue: cfg.shared_queue,
        extraction_prompt: cfg.query.clone(),
        cache: cfg.cache,
        cache_skip_browser: cfg.cache_skip_browser,
    }
}

#[derive(Debug, Serialize)]
struct CrawlAuditDiff {
    start_url: String,
    previous_count: usize,
    current_count: usize,
    added_count: usize,
    removed_count: usize,
    unchanged_count: usize,
    cache_hit: bool,
    cache_source: Option<String>,
}

fn resolve_initial_mode(render_mode: RenderMode, cache_skip_browser: bool) -> RenderMode {
    if cache_skip_browser {
        return RenderMode::Http;
    }
    match render_mode {
        RenderMode::AutoSwitch => RenderMode::Http,
        mode => mode,
    }
}

async fn read_manifest_urls(path: &Path) -> Result<HashSet<String>, Box<dyn Error>> {
    if !tokio::fs::try_exists(path).await? {
        return Ok(HashSet::new());
    }
    let content = tokio::fs::read_to_string(path).await?;
    let mut out = HashSet::new();
    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(url) = json.get("url").and_then(|value| value.as_str()) else {
            continue;
        };
        out.insert(url.to_string());
    }
    Ok(out)
}

async fn read_manifest_candidates(path: &Path) -> std::io::Result<Vec<InjectionCandidate>> {
    if !tokio::fs::try_exists(path).await? {
        return Ok(Vec::new());
    }
    let content = tokio::fs::read_to_string(path).await?;
    let mut out = Vec::new();
    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(url) = json.get("url").and_then(|value| value.as_str()) else {
            continue;
        };
        let Some(markdown_chars) = json.get("markdown_chars").and_then(|value| value.as_u64())
        else {
            continue;
        };
        out.push(InjectionCandidate {
            url: url.to_string(),
            markdown_chars: markdown_chars as usize,
        });
    }
    Ok(out)
}

async fn write_audit_diff(
    output_dir: &Path,
    start_url: &str,
    previous: &HashSet<String>,
    current: &HashSet<String>,
    cache_hit: bool,
    cache_source: Option<String>,
) -> Result<(PathBuf, CrawlAuditDiff), Box<dyn Error>> {
    let unchanged_count = previous.intersection(current).count();
    let added_count = current.difference(previous).count();
    let removed_count = previous.difference(current).count();
    let report = CrawlAuditDiff {
        start_url: start_url.to_string(),
        previous_count: previous.len(),
        current_count: current.len(),
        added_count,
        removed_count,
        unchanged_count,
        cache_hit,
        cache_source,
    };

    let audit_dir = output_dir.join("audit");
    tokio::fs::create_dir_all(&audit_dir).await?;
    let report_path = audit_dir.join("diff-report.json");
    let payload = serde_json::to_string_pretty(&report)?;
    tokio::fs::write(&report_path, payload).await?;
    Ok((report_path, report))
}

mod db;
mod robots;
mod worker;
use worker::reclaim_stale_running_jobs;

async fn latest_completed_result_for_url(
    pool: &PgPool,
    url: &str,
    current_job_id: Uuid,
) -> Result<Option<(Uuid, serde_json::Value)>, Box<dyn Error>> {
    let row = sqlx::query_as::<_, (Uuid, serde_json::Value)>(
        r#"
        SELECT id, result_json
        FROM axon_crawl_jobs
        WHERE url = $1
          AND id <> $2
          AND status = 'completed'
          AND result_json IS NOT NULL
        ORDER BY finished_at DESC NULLS LAST
        LIMIT 1
        "#,
    )
    .bind(url)
    .bind(current_job_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

async fn ensure_schema(pool: &PgPool) -> Result<(), Box<dyn Error>> {
    if SCHEMA_INIT.get().is_some() {
        return Ok(());
    }

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS axon_crawl_jobs (
            id UUID PRIMARY KEY,
            url TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('pending','running','completed','failed','canceled')),
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            started_at TIMESTAMPTZ,
            finished_at TIMESTAMPTZ,
            error_text TEXT,
            result_json JSONB,
            config_json JSONB NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_axon_crawl_jobs_status ON axon_crawl_jobs(status)")
        .execute(pool)
        .await?;

    // Composite partial index for claim_next_pending: WHERE status='pending' ORDER BY created_at
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_axon_crawl_jobs_pending ON axon_crawl_jobs(created_at ASC) WHERE status = 'pending'"
    )
    .execute(pool)
    .await?;

    // Add CHECK constraint to existing tables that were created before this constraint was added.
    // This is a no-op if the constraint already exists (catches the 42710 duplicate_object error).
    let add_check = sqlx::query(
        r#"
        ALTER TABLE axon_crawl_jobs
        ADD CONSTRAINT axon_crawl_jobs_status_check
        CHECK (status IN ('pending','running','completed','failed','canceled'))
        "#,
    )
    .execute(pool)
    .await;
    match add_check {
        Ok(_) => {}
        Err(sqlx::Error::Database(ref db_err)) if db_err.code().as_deref() == Some("42710") => {
            // Constraint already exists — expected for tables created with inline CHECK.
        }
        Err(err) => return Err(err.into()),
    }

    let _ = SCHEMA_INIT.set(());
    Ok(())
}

// Public API delegates to db module.
pub use db::{
    cancel_job, cleanup_jobs, clear_jobs, doctor, get_job, list_jobs, recover_stale_crawl_jobs,
    start_crawl_job, start_crawl_jobs_batch,
};

pub async fn run_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    worker::run_worker(cfg).await
}

#[cfg(test)]
mod tests;
