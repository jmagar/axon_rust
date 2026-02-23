use crate::crates::core::config::Config;
use redis::AsyncCommands;
use sqlx::PgPool;
use std::collections::HashSet;
use std::error::Error;
use std::path::PathBuf;
use uuid::Uuid;

use super::super::{latest_completed_result_for_url, read_manifest_urls, CrawlJobConfig};

pub(super) struct JobExecutionContext {
    pub(super) url: String,
    pub(super) job_cfg: Config,
    pub(super) extraction_prompt: Option<String>,
    pub(super) previous_urls: HashSet<String>,
    pub(super) cache_source: Option<String>,
}

async fn fetch_job_row(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<(String, serde_json::Value)>, Box<dyn Error>> {
    let row = sqlx::query_as::<_, (String, serde_json::Value)>(
        "SELECT url, config_json FROM axon_crawl_jobs WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

async fn maybe_cancel_job_before_start(
    cfg: &Config,
    pool: &PgPool,
    id: Uuid,
) -> Result<bool, Box<dyn Error>> {
    let redis_client = redis::Client::open(cfg.redis_url.clone())?;
    let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;
    let cancel_key = format!("axon:crawl:cancel:{id}");
    let cancel_before: Option<String> = redis_conn
        .get(&cancel_key)
        .await
        .map_err(|e| format!("failed to check crawl cancellation key {cancel_key}: {e}"))?;
    if cancel_before.is_none() {
        return Ok(false);
    }

    sqlx::query("UPDATE axon_crawl_jobs SET status='canceled', updated_at=NOW(), finished_at=NOW() WHERE id=$1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(true)
}

fn build_job_config(cfg: &Config, parsed: &CrawlJobConfig, id: Uuid) -> Config {
    let mut job_cfg = cfg.clone();
    job_cfg.max_pages = parsed.max_pages;
    job_cfg.max_depth = parsed.max_depth;
    job_cfg.include_subdomains = parsed.include_subdomains;
    job_cfg.exclude_path_prefix = parsed.exclude_path_prefix.clone();
    job_cfg.respect_robots = parsed.respect_robots;
    job_cfg.min_markdown_chars = parsed.min_markdown_chars;
    job_cfg.drop_thin_markdown = parsed.drop_thin_markdown;
    job_cfg.discover_sitemaps = parsed.discover_sitemaps;
    job_cfg.embed = parsed.embed;
    job_cfg.render_mode = parsed.render_mode;
    job_cfg.collection = parsed.collection.clone();
    job_cfg.crawl_concurrency_limit = parsed.crawl_concurrency_limit;
    job_cfg.backfill_concurrency_limit = parsed.backfill_concurrency_limit;
    job_cfg.delay_ms = parsed.delay_ms;
    job_cfg.request_timeout_ms = parsed.request_timeout_ms;
    job_cfg.fetch_retries = parsed.fetch_retries;
    job_cfg.retry_backoff_ms = parsed.retry_backoff_ms;
    job_cfg.shared_queue = parsed.shared_queue;
    job_cfg.query = parsed.extraction_prompt.clone();
    job_cfg.cache = parsed.cache;
    job_cfg.cache_skip_browser = parsed.cache_skip_browser;
    job_cfg.output_dir = PathBuf::from(parsed.output_dir.clone())
        .join("jobs")
        .join(id.to_string());
    job_cfg
}

async fn load_previous_urls_for_cache(
    pool: &PgPool,
    id: Uuid,
    url: &str,
    job_cfg: &Config,
) -> Result<(HashSet<String>, Option<String>), Box<dyn Error>> {
    let mut previous_urls = HashSet::new();
    let mut cache_source: Option<String> = None;

    if !job_cfg.cache {
        return Ok((previous_urls, cache_source));
    }

    if let Some((previous_job_id, previous_result_json)) =
        latest_completed_result_for_url(pool, url, id).await?
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

    Ok((previous_urls, cache_source))
}

pub(super) async fn load_job_execution_context(
    cfg: &Config,
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<JobExecutionContext>, Box<dyn Error>> {
    let row = fetch_job_row(pool, id).await?;
    let Some((url, cfg_json)) = row else {
        return Ok(None);
    };

    if maybe_cancel_job_before_start(cfg, pool, id).await? {
        return Ok(None);
    }

    let parsed: CrawlJobConfig = serde_json::from_value(cfg_json)?;
    let extraction_prompt = parsed.extraction_prompt.clone();
    let job_cfg = build_job_config(cfg, &parsed, id);
    let (previous_urls, cache_source) =
        load_previous_urls_for_cache(pool, id, &url, &job_cfg).await?;

    Ok(Some(JobExecutionContext {
        url,
        job_cfg,
        extraction_prompt,
        previous_urls,
        cache_source,
    }))
}
