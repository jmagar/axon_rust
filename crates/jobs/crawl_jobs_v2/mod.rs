use crate::axon_cli::crates::core::config::Config;
use std::error::Error;
use uuid::Uuid;

pub mod runtime;
pub mod manifest;
pub mod processor;
pub mod repo;
pub mod sitemap;
pub mod watchdog;
pub mod worker;

pub use runtime::CrawlJob;

pub async fn doctor(cfg: &Config) -> Result<serde_json::Value, Box<dyn Error>> {
    repo::doctor(cfg).await
}

pub async fn start_crawl_job(cfg: &Config, start_url: &str) -> Result<Uuid, Box<dyn Error>> {
    let plan = processor::build_start_plan(
        start_url,
        cfg.render_mode,
        cfg.cache_skip_browser,
        &cfg.exclude_path_prefix,
    )?;
    let mut next_cfg = cfg.clone();
    next_cfg.render_mode = plan.initial_mode;
    repo::start_crawl_job(&next_cfg, &plan.start_url).await
}

pub async fn get_job(cfg: &Config, id: Uuid) -> Result<Option<CrawlJob>, Box<dyn Error>> {
    repo::get_job(cfg, id).await
}

pub async fn list_jobs(cfg: &Config, limit: i64) -> Result<Vec<CrawlJob>, Box<dyn Error>> {
    repo::list_jobs(cfg, limit).await
}

pub async fn cancel_job(cfg: &Config, id: Uuid) -> Result<bool, Box<dyn Error>> {
    repo::cancel_job(cfg, id).await
}

pub async fn cleanup_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    repo::cleanup_jobs(cfg).await
}

pub async fn clear_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    repo::clear_jobs(cfg).await
}

pub async fn recover_stale_crawl_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    watchdog::recover_stale_crawl_jobs(cfg).await
}

pub async fn run_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    worker::run_worker(cfg).await
}
