use crate::crates::core::config::Config;
use crate::crates::jobs::crawl::runtime;
use std::error::Error;
use uuid::Uuid;

pub async fn doctor(cfg: &Config) -> Result<serde_json::Value, Box<dyn Error>> {
    runtime::doctor(cfg).await
}

pub async fn start_crawl_job(cfg: &Config, start_url: &str) -> Result<Uuid, Box<dyn Error>> {
    runtime::start_crawl_job(cfg, start_url).await
}

pub async fn get_job(cfg: &Config, id: Uuid) -> Result<Option<runtime::CrawlJob>, Box<dyn Error>> {
    runtime::get_job(cfg, id).await
}

pub async fn list_jobs(cfg: &Config, limit: i64) -> Result<Vec<runtime::CrawlJob>, Box<dyn Error>> {
    runtime::list_jobs(cfg, limit).await
}

pub async fn cancel_job(cfg: &Config, id: Uuid) -> Result<bool, Box<dyn Error>> {
    runtime::cancel_job(cfg, id).await
}

pub async fn cleanup_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    runtime::cleanup_jobs(cfg).await
}

pub async fn clear_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    runtime::clear_jobs(cfg).await
}
