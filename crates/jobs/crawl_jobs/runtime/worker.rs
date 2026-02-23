use crate::crates::core::config::Config;
use std::error::Error;

mod job_context;
mod result_builder;
mod worker_loops;
mod worker_process;

pub(super) use worker_loops::reclaim_stale_running_jobs;

pub(super) async fn run_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    worker_loops::run_worker(cfg).await
}
