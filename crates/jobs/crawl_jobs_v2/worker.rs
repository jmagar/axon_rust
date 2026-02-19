use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::jobs::crawl_jobs_v2::runtime;
use std::error::Error;

pub async fn run_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    runtime::run_worker(cfg).await
}
