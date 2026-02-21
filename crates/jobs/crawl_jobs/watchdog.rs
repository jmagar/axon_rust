use crate::crates::core::config::Config;
use crate::crates::jobs::crawl_jobs::runtime;
use std::error::Error;

pub async fn recover_stale_crawl_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    runtime::recover_stale_crawl_jobs(cfg).await
}
