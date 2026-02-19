#[allow(dead_code)]
pub(crate) const STAGE_NAME: &str = "repo";

use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::jobs::crawl_jobs;
use std::error::Error;
use uuid::Uuid;

pub async fn doctor(cfg: &Config) -> Result<serde_json::Value, Box<dyn Error>> {
    crawl_jobs::doctor(cfg).await
}

pub async fn start_crawl_job(cfg: &Config, start_url: &str) -> Result<Uuid, Box<dyn Error>> {
    crawl_jobs::start_crawl_job(cfg, start_url).await
}

pub async fn get_job(
    cfg: &Config,
    id: Uuid,
) -> Result<Option<crawl_jobs::CrawlJob>, Box<dyn Error>> {
    crawl_jobs::get_job(cfg, id).await
}

pub async fn list_jobs(
    cfg: &Config,
    limit: i64,
) -> Result<Vec<crawl_jobs::CrawlJob>, Box<dyn Error>> {
    crawl_jobs::list_jobs(cfg, limit).await
}

pub async fn cancel_job(cfg: &Config, id: Uuid) -> Result<bool, Box<dyn Error>> {
    crawl_jobs::cancel_job(cfg, id).await
}

pub async fn cleanup_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    crawl_jobs::cleanup_jobs(cfg).await
}

pub async fn clear_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    crawl_jobs::clear_jobs(cfg).await
}
