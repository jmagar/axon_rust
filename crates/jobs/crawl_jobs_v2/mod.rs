use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::jobs::crawl_jobs_legacy;
use std::error::Error;
use uuid::Uuid;

pub mod config;
pub mod manifest;
pub mod processor;
pub mod repo;
pub mod sitemap;
pub mod watchdog;
pub mod worker;

pub use crawl_jobs_legacy::CrawlJob;

pub async fn doctor(cfg: &Config) -> Result<serde_json::Value, Box<dyn Error>> {
    crawl_jobs_legacy::doctor(cfg).await
}

pub async fn start_crawl_job(cfg: &Config, start_url: &str) -> Result<Uuid, Box<dyn Error>> {
    crawl_jobs_legacy::start_crawl_job(cfg, start_url).await
}

pub async fn get_job(cfg: &Config, id: Uuid) -> Result<Option<CrawlJob>, Box<dyn Error>> {
    crawl_jobs_legacy::get_job(cfg, id).await
}

pub async fn list_jobs(cfg: &Config, limit: i64) -> Result<Vec<CrawlJob>, Box<dyn Error>> {
    crawl_jobs_legacy::list_jobs(cfg, limit).await
}

pub async fn cancel_job(cfg: &Config, id: Uuid) -> Result<bool, Box<dyn Error>> {
    crawl_jobs_legacy::cancel_job(cfg, id).await
}

pub async fn cleanup_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    crawl_jobs_legacy::cleanup_jobs(cfg).await
}

pub async fn clear_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    crawl_jobs_legacy::clear_jobs(cfg).await
}

pub async fn recover_stale_crawl_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    crawl_jobs_legacy::recover_stale_crawl_jobs(cfg).await
}

pub async fn run_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    crawl_jobs_legacy::run_worker(cfg).await
}
