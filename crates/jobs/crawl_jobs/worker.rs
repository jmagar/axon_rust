use crate::crates::core::config::Config;
use crate::crates::jobs::crawl_jobs::runtime;
use std::error::Error;

pub async fn run_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    runtime::run_worker(cfg).await
}
