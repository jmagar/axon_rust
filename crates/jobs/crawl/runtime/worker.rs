use crate::crates::core::config::Config;
use std::error::Error;

mod job_context;
mod loops;
mod process;
mod result_builder;

pub(super) use loops::reclaim_stale_running_jobs;

pub(super) async fn run_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    loops::run_worker(cfg).await
}
