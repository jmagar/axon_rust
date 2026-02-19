use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::jobs::crawl_jobs_v2::runtime;
use std::error::Error;

pub async fn recover_stale_crawl_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    runtime::recover_stale_crawl_jobs(cfg).await
}
