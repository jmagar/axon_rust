#[allow(dead_code)]
pub(crate) const STAGE_NAME: &str = "worker";

use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::jobs::crawl_jobs;
use std::error::Error;

pub async fn run_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    crawl_jobs::run_worker(cfg).await
}
