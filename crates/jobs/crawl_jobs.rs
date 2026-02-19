use crate::axon_cli::crates::core::config::{Config, RenderMode};
use crate::axon_cli::crates::core::content::{to_markdown, url_to_filename};
use crate::axon_cli::crates::core::health::redis_healthy;
use crate::axon_cli::crates::core::logging::{log_done, log_info, log_warn};
use crate::axon_cli::crates::crawl::engine::{
    append_sitemap_backfill, run_crawl_once, CrawlSummary, SitemapBackfillStats,
};
use crate::axon_cli::crates::jobs::batch_jobs::{apply_queue_injection, InjectionCandidate};
use crate::axon_cli::crates::jobs::common::{
    claim_next_pending, claim_pending_by_id, enqueue_job, make_pool, mark_job_failed,
    open_amqp_channel, JobTable,
};
use crate::axon_cli::crates::jobs::embed_jobs::start_embed_job;
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use lapin::options::{BasicAckOptions, BasicConsumeOptions};
use lapin::types::FieldTable;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use spider::tokio;
use spider::url::Url;
use sqlx::{FromRow, PgPool};
use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncWriteExt, BufWriter};
use uuid::Uuid;

const TABLE: JobTable = JobTable::Crawl;
const MID_CRAWL_INJECTION_TRIGGER_PAGES: u32 = 25;
const MID_CRAWL_INJECTION_MIN_CANDIDATES: usize = 3;
const WORKER_CONCURRENCY: usize = 2;
const STALE_RUNNING_TIMEOUT_SECS: i64 = 300;
const STALE_CONFIRMATION_SECS: i64 = 60;
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
    sitemap_concurrency_limit: Option<usize>,
    backfill_concurrency_limit: Option<usize>,
    max_sitemaps: usize,
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

#[derive(Debug, FromRow)]
struct StaleRunningJob {
    id: Uuid,
    url: String,
    started_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
    result_json: Option<serde_json::Value>,
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
        sitemap_concurrency_limit: cfg.sitemap_concurrency_limit,
        backfill_concurrency_limit: cfg.backfill_concurrency_limit,
        max_sitemaps: cfg.max_sitemaps,
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
    if !path.exists() {
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
    if !path.exists() {
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RobotsDiscoveryStats {
    robots_declared_sitemaps: usize,
    parsed_sitemap_documents: usize,
    discovered_urls: usize,
    filtered_out_of_scope_host: usize,
    filtered_out_of_scope_path: usize,
    filtered_excluded_prefix: usize,
    failed_fetches: usize,
    parse_errors: usize,
}

#[derive(Debug, Clone, Default)]
struct RobotsDiscoveryResult {
    urls: Vec<String>,
    stats: RobotsDiscoveryStats,
}

#[derive(Debug, Clone, Default)]
struct RobotsBackfillStats {
    discovered_urls: usize,
    candidates: usize,
    written: usize,
    failed: usize,
    filtered_existing: usize,
}

fn normalize_prefix(prefix: &str) -> Option<String> {
    let trimmed = prefix.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return None;
    }
    let mut value = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };
    if value.len() > 1 && value.ends_with('/') {
        value.truncate(value.len() - 1);
    }
    Some(value)
}

fn is_excluded_url_path(url: &str, prefixes: &[String]) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    let path = parsed.path();
    prefixes
        .iter()
        .filter_map(|p| normalize_prefix(p))
        .any(|p| path == p || (path.starts_with(&p) && path.as_bytes().get(p.len()) == Some(&b'/')))
}

fn canonicalize_url(url: &str) -> Option<String> {
    let mut parsed = Url::parse(url).ok()?;
    parsed.set_fragment(None);
    let path = parsed.path().to_string();
    if path.len() > 1 && path.ends_with('/') {
        parsed.set_path(path.trim_end_matches('/'));
    }
    Some(parsed.to_string())
}

fn extract_loc_values(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = xml.to_ascii_lowercase();
    let mut cursor = 0usize;
    while let Some(start) = lower[cursor..].find("<loc>") {
        let start_idx = cursor + start + "<loc>".len();
        let Some(end_rel) = lower[start_idx..].find("</loc>") else {
            break;
        };
        let end_idx = start_idx + end_rel;
        let value = xml[start_idx..end_idx].trim();
        if !value.is_empty() {
            out.push(value.replace("&amp;", "&"));
        }
        cursor = end_idx + "</loc>".len();
    }
    out
}

fn extract_robots_sitemaps(robots_txt: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in robots_txt.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("sitemap") {
            continue;
        }
        let url = value.trim();
        if !url.is_empty() {
            out.push(url.to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

async fn fetch_text_with_retry(
    client: &reqwest::Client,
    url: &str,
    retries: usize,
    backoff_ms: u64,
) -> Option<String> {
    for attempt in 0..=retries {
        let response = client.get(url).send().await;
        if let Ok(resp) = response {
            if resp.status().is_success() {
                if let Ok(text) = resp.text().await {
                    return Some(text);
                }
            }
        }
        if attempt < retries {
            let delay = backoff_ms.saturating_mul((attempt + 1) as u64);
            if delay > 0 {
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
        }
    }
    None
}

async fn discover_sitemap_urls_with_robots(
    cfg: &Config,
    start_url: &str,
) -> Result<RobotsDiscoveryResult, Box<dyn Error>> {
    let parsed = Url::parse(start_url)?;
    let scheme = parsed.scheme().to_string();
    let host = parsed.host_str().ok_or("missing host")?.to_string();
    let root_path = parsed.path().trim_end_matches('/').to_string();
    let scoped_to_root = root_path.is_empty();
    let timeout = Duration::from_millis(cfg.request_timeout_ms.unwrap_or(30_000));
    let client = reqwest::Client::builder().timeout(timeout).build()?;

    let mut queue: VecDeque<String> = VecDeque::from(vec![
        format!("{scheme}://{host}/sitemap.xml"),
        format!("{scheme}://{host}/sitemap_index.xml"),
        format!("{scheme}://{host}/sitemap-index.xml"),
    ]);
    let mut stats = RobotsDiscoveryStats::default();
    let robots_url = format!("{scheme}://{host}/robots.txt");
    if let Some(robots_txt) = fetch_text_with_retry(
        &client,
        &robots_url,
        cfg.fetch_retries,
        cfg.retry_backoff_ms,
    )
    .await
    {
        let robots_sitemaps = extract_robots_sitemaps(&robots_txt);
        stats.robots_declared_sitemaps = robots_sitemaps.len();
        for sitemap in robots_sitemaps {
            queue.push_back(sitemap);
        }
    }

    let mut seen_sitemaps = HashSet::new();
    let mut out = HashSet::new();
    let max_sitemaps = cfg.max_sitemaps.max(1);
    while let Some(candidate) = queue.pop_front() {
        if seen_sitemaps.len() >= max_sitemaps {
            break;
        }
        let Some(canonical_sitemap) = canonicalize_url(&candidate) else {
            stats.parse_errors += 1;
            continue;
        };
        if !seen_sitemaps.insert(canonical_sitemap.clone()) {
            continue;
        }
        let Some(xml) = fetch_text_with_retry(
            &client,
            &canonical_sitemap,
            cfg.fetch_retries,
            cfg.retry_backoff_ms,
        )
        .await
        else {
            stats.failed_fetches += 1;
            continue;
        };
        stats.parsed_sitemap_documents += 1;
        let is_index = xml.to_ascii_lowercase().contains("<sitemapindex");
        for loc in extract_loc_values(&xml) {
            let Ok(url) = Url::parse(&loc) else {
                stats.parse_errors += 1;
                continue;
            };
            let Some(url_host) = url.host_str() else {
                stats.parse_errors += 1;
                continue;
            };
            let host_ok = if cfg.include_subdomains {
                url_host == host || url_host.ends_with(&format!(".{host}"))
            } else {
                url_host == host
            };
            if !host_ok {
                stats.filtered_out_of_scope_host += 1;
                continue;
            }
            if !scoped_to_root {
                let p = url.path();
                let scoped_prefix = format!("{root_path}/");
                if p != root_path && !p.starts_with(&scoped_prefix) {
                    stats.filtered_out_of_scope_path += 1;
                    continue;
                }
            }
            if is_excluded_url_path(&loc, &cfg.exclude_path_prefix) {
                stats.filtered_excluded_prefix += 1;
                continue;
            }
            let Some(canonical_loc) = canonicalize_url(&loc) else {
                stats.parse_errors += 1;
                continue;
            };
            if is_index {
                queue.push_back(canonical_loc);
            } else {
                out.insert(canonical_loc);
            }
        }
    }
    let mut urls: Vec<String> = out.into_iter().collect();
    urls.sort();
    stats.discovered_urls = urls.len();
    Ok(RobotsDiscoveryResult { urls, stats })
}

async fn append_robots_backfill(
    cfg: &Config,
    start_url: &str,
    output_dir: &Path,
    seen_urls: &HashSet<String>,
    summary: &mut CrawlSummary,
) -> Result<RobotsBackfillStats, Box<dyn Error>> {
    let discovery = discover_sitemap_urls_with_robots(cfg, start_url).await?;
    let manifest_path = output_dir.join("manifest.jsonl");
    let already_written = read_manifest_urls(&manifest_path).await?;
    let candidates: Vec<String> = discovery
        .urls
        .iter()
        .filter(|url| !seen_urls.contains(*url) && !already_written.contains(*url))
        .cloned()
        .collect();
    if candidates.is_empty() {
        return Ok(RobotsBackfillStats {
            discovered_urls: discovery.urls.len(),
            filtered_existing: discovery.urls.len(),
            ..RobotsBackfillStats::default()
        });
    }

    let markdown_dir = output_dir.join("markdown");
    let timeout = Duration::from_millis(cfg.request_timeout_ms.unwrap_or(30_000));
    let client = reqwest::Client::builder().timeout(timeout).build()?;
    let mut manifest = BufWriter::new(
        tokio::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&manifest_path)
            .await?,
    );
    let mut idx = summary.markdown_files;
    let mut stats = RobotsBackfillStats {
        discovered_urls: discovery.urls.len(),
        candidates: candidates.len(),
        filtered_existing: discovery.urls.len().saturating_sub(candidates.len()),
        ..RobotsBackfillStats::default()
    };

    for url in candidates {
        let Some(html) =
            fetch_text_with_retry(&client, &url, cfg.fetch_retries, cfg.retry_backoff_ms).await
        else {
            stats.failed += 1;
            continue;
        };
        let markdown = to_markdown(&html);
        let markdown_chars = markdown.chars().count();
        if markdown_chars < cfg.min_markdown_chars {
            summary.thin_pages += 1;
        }
        if markdown_chars < cfg.min_markdown_chars && cfg.drop_thin_markdown {
            continue;
        }
        idx += 1;
        let file = markdown_dir.join(url_to_filename(&url, idx));
        tokio::fs::write(&file, markdown).await?;
        let rec = serde_json::json!({
            "url": url,
            "file_path": file.to_string_lossy(),
            "markdown_chars": markdown_chars,
            "source": "robots_sitemap_backfill"
        });
        let mut line = rec.to_string();
        line.push('\n');
        manifest.write_all(line.as_bytes()).await?;
        summary.markdown_files += 1;
        stats.written += 1;
    }
    manifest.flush().await?;
    Ok(stats)
}

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
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS axon_crawl_jobs (
            id UUID PRIMARY KEY,
            url TEXT NOT NULL,
            status TEXT NOT NULL,
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

    Ok(())
}

pub async fn doctor(cfg: &Config) -> Result<serde_json::Value, Box<dyn Error>> {
    let pg_ok = match make_pool(cfg).await {
        Ok(p) => ensure_schema(&p).await.is_ok(),
        Err(_) => false,
    };

    let amqp_result = open_amqp_channel(cfg, &cfg.crawl_queue).await;
    let amqp_ok = amqp_result.is_ok();
    let amqp_error = amqp_result.err().map(|e| e.to_string());

    let redis_ok = redis_healthy(&cfg.redis_url).await;

    Ok(serde_json::json!({
        "postgres_ok": pg_ok,
        "amqp_ok": amqp_ok,
        "amqp_error": amqp_error,
        "redis_ok": redis_ok,
        "all_ok": pg_ok && amqp_ok && redis_ok
    }))
}

pub async fn start_crawl_job(cfg: &Config, start_url: &str) -> Result<Uuid, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;

    let id = Uuid::new_v4();
    let cfg_json = serde_json::to_value(to_job_config(cfg))?;

    sqlx::query(
        r#"
        INSERT INTO axon_crawl_jobs (id, url, status, config_json)
        VALUES ($1, $2, 'pending', $3)
        "#,
    )
    .bind(id)
    .bind(start_url)
    .bind(cfg_json)
    .execute(&pool)
    .await?;

    if let Err(err) = enqueue_job(cfg, &cfg.crawl_queue, id).await {
        log_warn(&format!(
            "amqp enqueue failed for {id}; worker fallback polling will pick it up: {err}"
        ));
    }
    Ok(id)
}

pub async fn get_job(cfg: &Config, id: Uuid) -> Result<Option<CrawlJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let row = sqlx::query_as::<_, CrawlJob>(
        r#"
        SELECT id, url, status, created_at, updated_at, started_at, finished_at, error_text
            , result_json
        FROM axon_crawl_jobs
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?;
    Ok(row)
}

pub async fn list_jobs(cfg: &Config, limit: i64) -> Result<Vec<CrawlJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query_as::<_, CrawlJob>(
        r#"
        SELECT id, url, status, created_at, updated_at, started_at, finished_at, error_text
            , result_json
        FROM axon_crawl_jobs
        ORDER BY created_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(&pool)
    .await?;
    Ok(rows)
}

pub async fn cancel_job(cfg: &Config, id: Uuid) -> Result<bool, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;

    let rows = sqlx::query(
        "UPDATE axon_crawl_jobs SET status='canceled', updated_at=NOW(), finished_at=NOW() WHERE id=$1 AND status IN ('pending','running')",
    )
    .bind(id)
    .execute(&pool)
    .await?
    .rows_affected();

    let redis_client = redis::Client::open(cfg.redis_url.clone())?;
    let mut conn = redis_client.get_multiplexed_async_connection().await?;
    let key = format!("axon:crawl:cancel:{id}");
    let _: () = conn.set_ex(key, "1", 24 * 60 * 60).await?;

    Ok(rows > 0)
}

pub async fn cleanup_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query(
        "DELETE FROM axon_crawl_jobs WHERE status IN ('failed','canceled') OR (status='pending' AND created_at < NOW() - INTERVAL '1 day')",
    )
    .execute(&pool)
    .await?
    .rows_affected();
    Ok(rows)
}

pub async fn clear_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query("DELETE FROM axon_crawl_jobs")
        .execute(&pool)
        .await?
        .rows_affected();

    if let Ok(ch) = open_amqp_channel(cfg, &cfg.crawl_queue).await {
        let _ = ch
            .queue_purge(
                &cfg.crawl_queue,
                lapin::options::QueuePurgeOptions::default(),
            )
            .await;
    }

    Ok(rows)
}

async fn process_job(cfg: &Config, pool: &PgPool, id: Uuid) -> Result<(), Box<dyn Error>> {
    let row = sqlx::query_as::<_, (String, serde_json::Value)>(
        "SELECT url, config_json FROM axon_crawl_jobs WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    let Some((url, cfg_json)) = row else {
        return Ok(());
    };

    let redis_client = redis::Client::open(cfg.redis_url.clone())?;
    let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;
    let cancel_key = format!("axon:crawl:cancel:{id}");
    let cancel_before: Option<String> = redis_conn.get(&cancel_key).await.ok();
    if cancel_before.is_some() {
        sqlx::query("UPDATE axon_crawl_jobs SET status='canceled', updated_at=NOW(), finished_at=NOW() WHERE id=$1")
            .bind(id)
            .execute(pool)
            .await?;
        return Ok(());
    }

    let parsed: CrawlJobConfig = serde_json::from_value(cfg_json)?;
    let extraction_prompt = parsed.extraction_prompt.clone();
    let mut job_cfg = cfg.clone();
    job_cfg.max_pages = parsed.max_pages;
    job_cfg.max_depth = parsed.max_depth;
    job_cfg.include_subdomains = parsed.include_subdomains;
    job_cfg.exclude_path_prefix = parsed.exclude_path_prefix;
    job_cfg.respect_robots = parsed.respect_robots;
    job_cfg.min_markdown_chars = parsed.min_markdown_chars;
    job_cfg.drop_thin_markdown = parsed.drop_thin_markdown;
    job_cfg.discover_sitemaps = parsed.discover_sitemaps;
    job_cfg.embed = parsed.embed;
    job_cfg.render_mode = parsed.render_mode;
    job_cfg.collection = parsed.collection;
    job_cfg.crawl_concurrency_limit = parsed.crawl_concurrency_limit;
    job_cfg.sitemap_concurrency_limit = parsed.sitemap_concurrency_limit;
    job_cfg.backfill_concurrency_limit = parsed.backfill_concurrency_limit;
    job_cfg.max_sitemaps = parsed.max_sitemaps.max(1);
    job_cfg.delay_ms = parsed.delay_ms;
    job_cfg.request_timeout_ms = parsed.request_timeout_ms;
    job_cfg.fetch_retries = parsed.fetch_retries;
    job_cfg.retry_backoff_ms = parsed.retry_backoff_ms;
    job_cfg.shared_queue = parsed.shared_queue;
    job_cfg.query = parsed.extraction_prompt;
    job_cfg.cache = parsed.cache;
    job_cfg.cache_skip_browser = parsed.cache_skip_browser;
    job_cfg.output_dir = PathBuf::from(parsed.output_dir)
        .join("jobs")
        .join(id.to_string());

    let mut previous_urls = HashSet::new();
    let mut cache_source: Option<String> = None;
    if job_cfg.cache {
        if let Some((previous_job_id, previous_result_json)) =
            latest_completed_result_for_url(pool, &url, id).await?
        {
            let previous_output_dir = previous_result_json
                .get("output_dir")
                .and_then(|value| value.as_str())
                .map(PathBuf::from);
            if let Some(previous_output_dir) = previous_output_dir {
                let previous_manifest = previous_output_dir.join("manifest.jsonl");
                previous_urls = read_manifest_urls(&previous_manifest).await?;
                if !previous_urls.is_empty() {
                    cache_source = Some(format!(
                        "job:{} manifest:{}",
                        previous_job_id,
                        previous_manifest.to_string_lossy()
                    ));
                }
            }
        }
    }

    if job_cfg.cache && !previous_urls.is_empty() {
        let (report_path, diff_report) = write_audit_diff(
            &job_cfg.output_dir,
            &url,
            &previous_urls,
            &previous_urls,
            true,
            cache_source.clone(),
        )
        .await?;

        let result_json = serde_json::json!({
            "phase": "completed",
            "cache_hit": true,
            "cache_skip_browser": job_cfg.cache_skip_browser,
            "md_created": previous_urls.len(),
            "thin_md": 0,
            "filtered_urls": 0,
            "pages_crawled": 0,
            "pages_discovered": previous_urls.len(),
            "crawl_stream_pages": 0,
            "sitemap_discovered": 0,
            "sitemap_candidates": 0,
            "sitemap_processed": 0,
            "sitemap_fetched_ok": 0,
            "sitemap_written": 0,
            "sitemap_failed": 0,
            "sitemap_filtered": 0,
            "elapsed_ms": 0,
            "output_dir": job_cfg.output_dir.to_string_lossy(),
            "audit_diff": diff_report,
            "audit_report_path": report_path.to_string_lossy(),
        });

        sqlx::query(
            "UPDATE axon_crawl_jobs SET status='completed', updated_at=NOW(), finished_at=NOW(), error_text=NULL, result_json=$2 WHERE id=$1 AND status='running'",
        )
        .bind(id)
        .bind(result_json)
        .execute(pool)
        .await?;
        log_done(&format!("worker completed crawl job {id} (cache hit)"));
        return Ok(());
    }

    let manifest_path = job_cfg.output_dir.join("manifest.jsonl");
    let mid_injection_state = Arc::new(tokio::sync::Mutex::new(None::<serde_json::Value>));
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<CrawlSummary>();
    let progress_pool = pool.clone();
    let progress_job_id = id;
    let progress_cfg = job_cfg.clone();
    let progress_prompt = extraction_prompt.clone();
    let progress_manifest_path = manifest_path.clone();
    let progress_injection_state = Arc::clone(&mid_injection_state);
    let progress_task = tokio::spawn(async move {
        let mut injection_attempted = false;
        while let Some(progress) = progress_rx.recv().await {
            let pages_crawled = progress.pages_seen as u64;
            let filtered_urls = pages_crawled.saturating_sub(progress.markdown_files as u64);

            if !injection_attempted && progress.pages_seen >= MID_CRAWL_INJECTION_TRIGGER_PAGES {
                match read_manifest_candidates(&progress_manifest_path).await {
                    Ok(candidates) if candidates.len() >= MID_CRAWL_INJECTION_MIN_CANDIDATES => {
                        let injection = match apply_queue_injection(
                            &progress_cfg,
                            &candidates,
                            progress_prompt.as_deref(),
                            "mid-crawl",
                            true,
                        )
                        .await
                        {
                            Ok(value) => value,
                            Err(err) => serde_json::json!({
                                "phase": "mid-crawl",
                                "queue_status": "failed",
                                "error": err.to_string(),
                            }),
                        };
                        *progress_injection_state.lock().await = Some(injection);
                        injection_attempted = true;
                    }
                    Ok(_) => {}
                    Err(err) => {
                        log_warn(&format!(
                            "mid-crawl queue injection probe failed for crawl job {progress_job_id}: {err}"
                        ));
                    }
                }
            }

            let mid_queue_injection = progress_injection_state.lock().await.clone();
            let progress_json = serde_json::json!({
                "phase": "crawling",
                "md_created": progress.markdown_files,
                "thin_md": progress.thin_pages,
                "filtered_urls": filtered_urls,
                "pages_crawled": pages_crawled,
                "crawl_stream_pages": progress.pages_seen,
                "mid_queue_injection": mid_queue_injection,
            });
            let _ = sqlx::query(
                "UPDATE axon_crawl_jobs SET updated_at=NOW(), result_json=$2 WHERE id=$1 AND status='running'",
            )
            .bind(progress_job_id)
            .bind(progress_json)
            .execute(&progress_pool)
            .await;
        }
    });

    let final_prompt = extraction_prompt.clone();
    let result = async {
        let initial_mode = resolve_initial_mode(job_cfg.render_mode, job_cfg.cache_skip_browser);
        let (summary, seen_urls) = run_crawl_once(
            &job_cfg,
            &url,
            initial_mode,
            &job_cfg.output_dir,
            Some(progress_tx),
        )
        .await?;
        let mut final_summary = summary.clone();
        let mut backfill_stats = SitemapBackfillStats::default();
        let mut robots_backfill_stats = RobotsBackfillStats::default();
        let mut robots_discovery_stats = RobotsDiscoveryStats::default();

        if job_cfg.discover_sitemaps {
            backfill_stats = append_sitemap_backfill(
                &job_cfg,
                &url,
                &job_cfg.output_dir,
                &seen_urls,
                &mut final_summary,
            )
            .await?;
            robots_backfill_stats = append_robots_backfill(
                &job_cfg,
                &url,
                &job_cfg.output_dir,
                &seen_urls,
                &mut final_summary,
            )
            .await?;
            robots_discovery_stats = discover_sitemap_urls_with_robots(&job_cfg, &url)
                .await?
                .stats;
        }

        if job_cfg.embed {
            let markdown_dir = job_cfg.output_dir.join("markdown");
            let embed_job_id = start_embed_job(&job_cfg, &markdown_dir.to_string_lossy()).await?;
            log_info(&format!(
                "command=crawl enqueue_embed crawl_job_id={} embed_job_id={}",
                id, embed_job_id
            ));
        }

        let crawl_discovered = summary.pages_seen as u64;
        let sitemap_discovered = backfill_stats.sitemap_candidates as u64;
        let robots_extra = robots_backfill_stats.candidates as u64;
        let pages_discovered = crawl_discovered
            .saturating_add(sitemap_discovered)
            .saturating_add(robots_extra);
        let filtered_urls = pages_discovered.saturating_sub(final_summary.markdown_files as u64);
        let pages_crawled = summary.pages_seen as u64;
        let current_urls = read_manifest_urls(&manifest_path).await?;
        let candidates = read_manifest_candidates(&manifest_path).await?;
        let mid_queue_injection = mid_injection_state.lock().await.clone();
        let mid_enqueued = mid_queue_injection
            .as_ref()
            .and_then(|value| value.get("queue_status"))
            .and_then(|value| value.as_str())
            == Some("enqueued");
        let queue_injection = apply_queue_injection(
            &job_cfg,
            &candidates,
            final_prompt.as_deref(),
            if mid_enqueued {
                "post-crawl-review"
            } else {
                "post-crawl"
            },
            !mid_enqueued,
        )
        .await?;
        let (report_path, diff_report) = write_audit_diff(
            &job_cfg.output_dir,
            &url,
            &previous_urls,
            &current_urls,
            false,
            cache_source,
        )
        .await?;

        Ok::<serde_json::Value, Box<dyn Error>>(serde_json::json!({
            "phase": "completed",
            "cache_hit": false,
            "cache_skip_browser": job_cfg.cache_skip_browser,
            "md_created": final_summary.markdown_files,
            "thin_md": final_summary.thin_pages,
            "filtered_urls": filtered_urls,
            "pages_crawled": pages_crawled,
            "pages_discovered": pages_discovered,
            "crawl_stream_pages": summary.pages_seen,
            "sitemap_discovered": backfill_stats.sitemap_discovered,
            "sitemap_candidates": backfill_stats.sitemap_candidates,
            "sitemap_processed": backfill_stats.processed,
            "sitemap_fetched_ok": backfill_stats.fetched_ok,
            "sitemap_written": backfill_stats.written,
            "sitemap_failed": backfill_stats.failed,
            "sitemap_filtered": backfill_stats.filtered,
            "robots_sitemap_docs_parsed": robots_discovery_stats.parsed_sitemap_documents,
            "robots_declared_sitemaps": robots_discovery_stats.robots_declared_sitemaps,
            "robots_discovered_urls": robots_backfill_stats.discovered_urls,
            "robots_candidates": robots_backfill_stats.candidates,
            "robots_written": robots_backfill_stats.written,
            "robots_failed": robots_backfill_stats.failed,
            "robots_filtered_existing": robots_backfill_stats.filtered_existing,
            "elapsed_ms": final_summary.elapsed_ms,
            "output_dir": job_cfg.output_dir.to_string_lossy(),
            "audit_diff": diff_report,
            "audit_report_path": report_path.to_string_lossy(),
            "mid_queue_injection": mid_queue_injection,
            "queue_injection": queue_injection,
            "extraction_observability": queue_injection["observability"].clone(),
        }))
    }
    .await;

    if let Err(err) = progress_task.await {
        log_warn(&format!(
            "progress_task panicked while serializing progress for crawl job {id}: {err:?}"
        ));
    }

    match result {
        Ok(result_json) => {
            sqlx::query(
                "UPDATE axon_crawl_jobs SET status='completed', updated_at=NOW(), finished_at=NOW(), error_text=NULL, result_json=$2 WHERE id=$1 AND status='running'",
            )
            .bind(id)
            .bind(result_json)
            .execute(pool)
            .await?;
            log_done(&format!("worker completed crawl job {id}"));
        }
        Err(err) => {
            let msg = err.to_string();
            sqlx::query(
                "UPDATE axon_crawl_jobs SET status='failed', updated_at=NOW(), finished_at=NOW(), error_text=$2 WHERE id=$1 AND status='running'",
            )
            .bind(id)
            .bind(msg)
            .execute(pool)
            .await?;
            log_warn(&format!("worker failed crawl job {id}"));
        }
    }

    Ok(())
}

fn stale_watchdog_payload(
    mut result_json: serde_json::Value,
    observed_updated_at: DateTime<Utc>,
) -> serde_json::Value {
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
    result_json: &serde_json::Value,
    observed_updated_at: DateTime<Utc>,
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
    elapsed >= STALE_CONFIRMATION_SECS
}

async fn reclaim_stale_running_jobs(pool: &PgPool, lane: usize) -> Result<(), Box<dyn Error>> {
    let stale_jobs = sqlx::query_as::<_, StaleRunningJob>(
        r#"
        SELECT id, url, started_at, updated_at, result_json
        FROM axon_crawl_jobs
        WHERE status = 'running'
          AND updated_at < NOW() - make_interval(secs => $1::int)
        ORDER BY updated_at ASC
        LIMIT 50
        "#,
    )
    .bind(STALE_RUNNING_TIMEOUT_SECS as i32)
    .fetch_all(pool)
    .await?;

    for job in stale_jobs {
        let idle_seconds = Utc::now()
            .signed_duration_since(job.updated_at)
            .num_seconds();
        let result_json = job
            .result_json
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));

        if stale_watchdog_confirmed(&result_json, job.updated_at) {
            let pages_crawled = result_json
                .get("pages_crawled")
                .and_then(|value| value.as_u64())
                .unwrap_or(0);
            let phase = result_json
                .get("phase")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");
            let msg = format!(
                "watchdog reclaimed stale running crawl job (idle={}s phase={} pages_crawled={} lane={})",
                idle_seconds, phase, pages_crawled, lane
            );
            let rows = sqlx::query(
                "UPDATE axon_crawl_jobs SET status='failed', updated_at=NOW(), finished_at=NOW(), error_text=$2 WHERE id=$1 AND status='running'",
            )
            .bind(job.id)
            .bind(msg.clone())
            .execute(pool)
            .await?
            .rows_affected();
            if rows > 0 {
                log_warn(&format!(
                    "watchdog marked crawl job {} as failed: {}",
                    job.id, msg
                ));
            }
            continue;
        }

        let marked_json = stale_watchdog_payload(result_json, job.updated_at);
        let _ = sqlx::query(
            "UPDATE axon_crawl_jobs SET result_json=$2 WHERE id=$1 AND status='running'",
        )
        .bind(job.id)
        .bind(marked_json)
        .execute(pool)
        .await?;
        let started = job
            .started_at
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(|| "unknown".to_string());
        log_warn(&format!(
            "watchdog marked crawl job {} as stale candidate (lane={} idle={}s started_at={} url={})",
            job.id, lane, idle_seconds, started, job.url
        ));
    }

    Ok(())
}

async fn run_worker_polling_loop(cfg: &Config, pool: &PgPool) -> Result<(), Box<dyn Error>> {
    log_warn("amqp unavailable; running crawl worker in postgres polling mode");
    if WORKER_CONCURRENCY <= 1 {
        return run_worker_polling_lane(cfg, pool, 1).await;
    }
    tokio::try_join!(
        run_worker_polling_lane(cfg, pool, 1),
        run_worker_polling_lane(cfg, pool, 2)
    )?;
    Ok(())
}

async fn run_worker_polling_lane(
    cfg: &Config,
    pool: &PgPool,
    lane: usize,
) -> Result<(), Box<dyn Error>> {
    log_info(&format!(
        "crawl worker polling lane={} active queue={}",
        lane, cfg.crawl_queue
    ));
    let mut last_sweep = Instant::now();
    loop {
        if last_sweep.elapsed() >= Duration::from_secs(STALE_SWEEP_INTERVAL_SECS) {
            if let Err(err) = reclaim_stale_running_jobs(pool, lane).await {
                log_warn(&format!("watchdog sweep failed (lane={}): {}", lane, err));
            }
            last_sweep = Instant::now();
        }
        if let Some(job_id) = claim_next_pending(pool, TABLE).await? {
            if let Err(err) = process_job(cfg, pool, job_id).await {
                let error_text = err.to_string();
                mark_job_failed(pool, TABLE, job_id, &error_text).await;
                log_warn(&format!("worker failed crawl job {job_id}: {error_text}"));
            }
        } else {
            tokio::time::sleep(Duration::from_millis(800)).await;
        }
    }
}

async fn run_amqp_worker_lane(
    cfg: &Config,
    pool: &PgPool,
    lane: usize,
) -> Result<(), Box<dyn Error>> {
    let ch = open_amqp_channel(cfg, &cfg.crawl_queue).await?;
    let consumer_tag = format!("axon-rust-crawl-worker-{lane}");
    let mut consumer = ch
        .basic_consume(
            &cfg.crawl_queue,
            &consumer_tag,
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    log_info(&format!(
        "crawl worker lane={} listening on queue={} concurrency={}",
        lane, cfg.crawl_queue, WORKER_CONCURRENCY
    ));

    loop {
        let msg = match tokio::time::timeout(
            Duration::from_secs(STALE_SWEEP_INTERVAL_SECS),
            consumer.next(),
        )
        .await
        {
            Ok(Some(msg)) => msg,
            Ok(None) => break,
            Err(_) => {
                if let Err(err) = reclaim_stale_running_jobs(pool, lane).await {
                    log_warn(&format!("watchdog sweep failed (lane={}): {}", lane, err));
                }
                continue;
            }
        };
        let delivery = match msg {
            Ok(d) => d,
            Err(err) => {
                log_warn(&format!("consumer error (lane={lane}): {err}"));
                continue;
            }
        };

        let parsed = std::str::from_utf8(&delivery.data)
            .ok()
            .and_then(|s| Uuid::parse_str(s.trim()).ok());

        if let Some(job_id) = parsed {
            if claim_pending_by_id(pool, TABLE, job_id)
                .await
                .unwrap_or(false)
            {
                if let Err(err) = process_job(cfg, pool, job_id).await {
                    let error_text = err.to_string();
                    mark_job_failed(pool, TABLE, job_id, &error_text).await;
                    log_warn(&format!("worker failed crawl job {job_id}: {error_text}"));
                }
            }
        }

        if let Err(err) = delivery.ack(BasicAckOptions::default()).await {
            log_warn(&format!(
                "failed to ack crawl delivery (lane={lane}): {err}"
            ));
        }
    }

    Err(format!("crawl worker consumer stream ended unexpectedly (lane={lane})").into())
}

pub async fn run_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;

    match open_amqp_channel(cfg, &cfg.crawl_queue).await {
        Ok(_) => {}
        Err(_) => return run_worker_polling_loop(cfg, &pool).await,
    }
    if WORKER_CONCURRENCY <= 1 {
        return run_amqp_worker_lane(cfg, &pool, 1).await;
    }
    tokio::try_join!(
        run_amqp_worker_lane(cfg, &pool, 1),
        run_amqp_worker_lane(cfg, &pool, 2)
    )?;
    Ok(())
}
