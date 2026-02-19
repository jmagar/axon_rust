#[allow(dead_code)]
pub(crate) const STAGE_NAME: &str = "watchdog";

use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::jobs::crawl_jobs;
use std::error::Error;

pub async fn recover_stale_crawl_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    crawl_jobs::recover_stale_crawl_jobs(cfg).await
}
